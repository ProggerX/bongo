use crate::{STATE_LEFT, STATE_REST, STATE_RIGHT};
use cocoa::appkit::{NSColor, NSScreen, NSWindow, NSWindowCollectionBehavior};
use cocoa::base::{NO, nil};
use cocoa::foundation::NSPoint;
use glib::translate::ToGlibPtr;
use gtk4::ApplicationWindow;
use gtk4::prelude::*;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;

mod input;
mod ui;

pub(crate) fn run(config: crate::Config) -> glib::ExitCode {
    ui::run(config)
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

unsafe extern "C" {
    fn gdk_macos_surface_get_native_window(
        surface: *mut gtk4::gdk::ffi::GdkSurface,
    ) -> cocoa::base::id;
}

pub fn configure_window(window: &ApplicationWindow, position: &str) {
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_focusable(false);

    let map_position = position.to_owned();
    window.connect_map(move |window| update_window(window, &map_position));

    let window_weak = window.downgrade();
    let position = position.to_owned();
    glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        let Some(window) = window_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if window.is_mapped() {
            update_window(&window, &position);
        }
        glib::ControlFlow::Continue
    });
}

fn update_window(window: &ApplicationWindow, position: &str) {
    let Some(surface) = window.surface() else {
        return;
    };
    unsafe {
        let native = gdk_macos_surface_get_native_window(surface.to_glib_none().0);
        if native == nil {
            return;
        }
        native.setLevel_(3);
        native.setHidesOnDeactivate_(NO);
        native.setCollectionBehavior_(
            NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
        );
        native.setOpaque_(NO);
        native.setBackgroundColor_(NSColor::clearColor(nil));
        native.setHasShadow_(NO);

        let screen = native.screen();
        if screen == nil {
            return;
        }
        let area = screen.visibleFrame();
        let frame = NSWindow::frame(native);
        let right = matches!(position, "top-right" | "bottom-right");
        let bottom = matches!(position, "bottom-left" | "bottom-right");

        let origin = NSPoint::new(
            if right {
                area.origin.x + area.size.width - frame.size.width - 16.0
            } else {
                area.origin.x + 16.0
            },
            if bottom {
                area.origin.y + 16.0
            } else {
                area.origin.y + area.size.height - frame.size.height - 16.0
            },
        );
        if frame.origin.x != origin.x || frame.origin.y != origin.y {
            native.setFrameOrigin_(origin);
        }
    }
}

pub fn start_input(state: Arc<AtomicU8>, _target_device_name: String) {
    std::thread::spawn(move || {
        let access_granted = unsafe {
            if CGPreflightListenEventAccess() {
                true
            } else {
                println!("[WARN] macOS keyboard access is not granted; requesting permission...");
                CGRequestListenEventAccess()
            }
        };

        if !access_granted {
            println!(
                "[ERROR] macOS denied keyboard access. Enable Input Monitoring for the terminal or launcher in System Settings > Privacy & Security, then restart Bongo Cat."
            );
            return;
        }

        println!("[INFO] Starting macOS keyboard event tap...");
        if let Err(error) = input::listen(state) {
            eprintln!("[ERROR] macOS keyboard listener stopped: {error}");
        }
    });
}

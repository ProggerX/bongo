use super::super::{Config, STATE_LEFT, STATE_REST, STATE_RIGHT, expand_path};
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Picture};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

fn image_texture(configured_path: Option<String>, bundled: &'static [u8]) -> gdk::Texture {
    if let Some(path) = expand_path(configured_path) {
        match gtk4::gdk_pixbuf::Pixbuf::from_file(&path) {
            Ok(pixbuf) => return gdk::Texture::for_pixbuf(&pixbuf),
            Err(error) => eprintln!("[WARN] Cannot load image {path}: {error}"),
        }
    }

    let pixbuf = gtk4::gdk_pixbuf::Pixbuf::from_read(std::io::Cursor::new(bundled))
        .expect("Bundled cat image must be valid");
    gdk::Texture::for_pixbuf(&pixbuf)
}

pub(super) fn run(conf: Config) -> glib::ExitCode {
    if let Err(error) = gtk4::init() {
        eprintln!("Failed to initialize GTK: {error}");
        return glib::ExitCode::new(1);
    }

    let app = Application::new(
        Some("com.bongocat.widget"),
        gtk4::gio::ApplicationFlags::empty(),
    );

    app.connect_activate(move |app| {
        build_ui(app, conf.clone());
    });

    app.run()
}

fn build_ui(app: &Application, conf: Config) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Bongo Cat Widget")
        .build();

    let position = conf
        .window
        .as_ref()
        .and_then(|window| window.position.clone())
        .unwrap_or_else(|| "top-left".to_string());

    crate::macos::configure_window(&window, &position);

    let img_rest = image_texture(
        conf.imgs.as_ref().and_then(|i| i.rest.clone()),
        include_bytes!("../../imgs/cat-rest.png"),
    );
    let img_left = image_texture(
        conf.imgs.as_ref().and_then(|i| i.left.clone()),
        include_bytes!("../../imgs/cat-left.png"),
    );
    let img_right = image_texture(
        conf.imgs.as_ref().and_then(|i| i.right.clone()),
        include_bytes!("../../imgs/cat-right.png"),
    );

    let width = img_rest
        .width()
        .max(img_left.width())
        .max(img_right.width()) as u32;
    let height = img_rest
        .height()
        .max(img_left.height())
        .max(img_right.height()) as u32;
    let (width, height) = conf.image_size(width, height);
    window.set_default_size(width as i32, height as i32);

    let image = Picture::for_paintable(&img_rest);
    image.set_can_shrink(true);
    image.set_keep_aspect_ratio(true);
    window.set_child(Some(&image));

    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        "
        window { background-color: transparent; border: none; box-shadow: none; }
    ",
    );

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    window.present();

    let current_state = Arc::new(AtomicU8::new(STATE_REST));

    let image_clone = image.clone();
    let state_ui = current_state.clone();
    let mut last_rendered_state = STATE_REST;

    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        let state = state_ui.load(Ordering::Relaxed);
        if state != last_rendered_state {
            match state {
                STATE_LEFT => image_clone.set_paintable(Some(&img_left)),
                STATE_RIGHT => image_clone.set_paintable(Some(&img_right)),
                _ => image_clone.set_paintable(Some(&img_rest)),
            }
            last_rendered_state = state;
        }
        glib::ControlFlow::Continue
    });

    let state_input = current_state.clone();

    let target_device_name = conf
        .input
        .as_ref()
        .and_then(|i| i.device.clone())
        .unwrap_or_else(|| "AT Translated Set 2 keyboard".to_string());

    crate::macos::start_input(state_input, target_device_name);
}

//! Observe physical key transitions without calling the main-thread-only
//! keyboard layout APIs used by rdev to translate keys into text.
use super::{STATE_LEFT, STATE_REST, STATE_RIGHT};
use std::ffi::c_void;
use std::ptr::null;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

type Ref = *const c_void;
type Event = *mut c_void;
type Callback = unsafe extern "C" fn(Ref, u32, Event, *mut c_void) -> Event;

const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const FLAGS_CHANGED: u32 = 12;
const TAP_DISABLED_TIMEOUT: u32 = 0xffff_fffe;
const TAP_DISABLED_INPUT: u32 = 0xffff_ffff;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        location: u32,
        placement: u32,
        options: u32,
        mask: u64,
        callback: Callback,
        context: *mut c_void,
    ) -> Ref;
    fn CGEventTapEnable(tap: Ref, enable: bool);
    fn CGEventGetIntegerValueField(event: Event, field: u32) -> i64;
    fn CGEventGetFlags(event: Event) -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopCommonModes: Ref;
    fn CFMachPortCreateRunLoopSource(allocator: Ref, port: Ref, order: isize) -> Ref;
    fn CFMachPortInvalidate(port: Ref);
    fn CFRunLoopGetCurrent() -> Ref;
    fn CFRunLoopAddSource(run_loop: Ref, source: Ref, mode: Ref);
    fn CFRunLoopRemoveSource(run_loop: Ref, source: Ref, mode: Ref);
    fn CFRunLoopRun();
    fn CFRelease(object: Ref);
}

struct Listener {
    state: Arc<AtomicU8>,
    strike_left: bool,
    tap: Ref,
}

impl Listener {
    fn key(&mut self, pressed: bool) {
        if pressed {
            self.state.store(
                if self.strike_left {
                    STATE_LEFT
                } else {
                    STATE_RIGHT
                },
                Ordering::Relaxed,
            );
            self.strike_left = !self.strike_left;
        } else {
            self.state.store(STATE_REST, Ordering::Relaxed);
        }
    }
}

fn modifier_pressed(keycode: i64, flags: u64) -> Option<bool> {
    let mask = match keycode {
        54 => 0x10,
        55 => 0x08,
        56 => 0x02,
        57 => 1 << 16,
        58 => 0x20,
        59 => 0x01,
        60 => 0x04,
        61 => 0x40,
        62 => 0x2000,
        63 => 1 << 23,
        _ => return None,
    };
    Some(flags & mask != 0)
}

unsafe extern "C" fn callback(_proxy: Ref, kind: u32, event: Event, context: *mut c_void) -> Event {
    let listener = unsafe { &mut *context.cast::<Listener>() };
    match kind {
        TAP_DISABLED_TIMEOUT | TAP_DISABLED_INPUT => {
            listener.state.store(STATE_REST, Ordering::Relaxed);
            unsafe { CGEventTapEnable(listener.tap, true) };
        }
        KEY_DOWN => listener.key(true),
        KEY_UP => listener.key(false),
        FLAGS_CHANGED if !event.is_null() => {
            let keycode = unsafe { CGEventGetIntegerValueField(event, 9) };
            let flags = unsafe { CGEventGetFlags(event) };
            if let Some(pressed) = modifier_pressed(keycode, flags) {
                listener.key(pressed);
            }
        }
        _ => {}
    }
    event
}

pub(super) fn listen(state: Arc<AtomicU8>) -> Result<(), &'static str> {
    let mut listener = Listener {
        state,
        strike_left: true,
        tap: null(),
    };
    unsafe {
        let mask = (1 << KEY_DOWN) | (1 << KEY_UP) | (1 << FLAGS_CHANGED);
        let tap = CGEventTapCreate(
            1,
            0,
            1,
            mask,
            callback,
            (&mut listener as *mut Listener).cast(),
        );
        if tap.is_null() {
            return Err(
                "Cannot create event tap. Check Input Monitoring permission and restart Bongo.",
            );
        }
        listener.tap = tap;
        let source = CFMachPortCreateRunLoopSource(null(), tap, 0);
        if source.is_null() {
            CFMachPortInvalidate(tap);
            CFRelease(tap);
            return Err("Cannot create keyboard event run-loop source.");
        }
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        CFRunLoopRun();
        CFMachPortInvalidate(tap);
        CFRunLoopRemoveSource(run_loop, source, kCFRunLoopCommonModes);
        CFRelease(source);
        CFRelease(tap);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr::null_mut;

    #[test]
    fn strikes_alternate_across_release_and_repeat() {
        let state = Arc::new(AtomicU8::new(STATE_REST));
        let mut listener = Listener {
            state: state.clone(),
            strike_left: true,
            tap: null(),
        };
        for (pressed, expected) in [
            (true, STATE_LEFT),
            (false, STATE_REST),
            (true, STATE_RIGHT),
            (true, STATE_LEFT),
            (false, STATE_REST),
        ] {
            listener.key(pressed);
            assert_eq!(state.load(Ordering::Relaxed), expected);
        }
    }

    #[test]
    fn modifiers_distinguish_sides_and_ignore_unrelated_flags() {
        assert_eq!(modifier_pressed(56, 0x02 | 0x04 | (1 << 17)), Some(true));
        assert_eq!(modifier_pressed(56, 0x04 | (1 << 17)), Some(false));
        assert_eq!(modifier_pressed(60, 0x04 | (1 << 17)), Some(true));
        assert_eq!(modifier_pressed(57, 1 << 16), Some(true));
        assert_eq!(modifier_pressed(63, 1 << 23), Some(true));
        assert_eq!(modifier_pressed(0, u64::MAX), None);
    }

    #[test]
    fn callback_accepts_key_events_without_layout_or_text_lookup() {
        let state = Arc::new(AtomicU8::new(STATE_REST));
        let mut listener = Listener {
            state: state.clone(),
            strike_left: true,
            tap: null(),
        };
        let context = (&mut listener as *mut Listener).cast();
        unsafe {
            assert!(callback(null(), KEY_DOWN, null_mut(), context).is_null());
            assert_eq!(state.load(Ordering::Relaxed), STATE_LEFT);
            callback(null(), KEY_UP, null_mut(), context);
            assert_eq!(state.load(Ordering::Relaxed), STATE_REST);
        }
    }
}

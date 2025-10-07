/// LD_PRELOAD library to hide overlay window from X11 client enumeration
///
/// This library intercepts X11/XCB functions to filter out the overlay window
/// from queries like XQueryTree, XGetWindowAttributes, etc.
///
/// Usage: LD_PRELOAD=./libstealth_hook.so your_application
use lazy_static::lazy_static;
use redhook::{hook, real};
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_ulong, c_void};
use std::sync::RwLock;

// X11 types
type Display = c_void;
type Window = c_ulong;
type Status = c_int;
type Atom = c_ulong;

lazy_static! {
    static ref HIDDEN_WINDOWS: RwLock<Vec<Window>> = RwLock::new(Vec::new());
    static ref STEALTH_PID: RwLock<Option<u32>> = RwLock::new(None);
}

/// Register a window ID to be hidden from enumeration
#[no_mangle]
pub extern "C" fn stealth_register_window(window: Window) {
    if let Ok(mut windows) = HIDDEN_WINDOWS.write() {
        if !windows.contains(&window) {
            windows.push(window);
            eprintln!("[STEALTH] Registered window 0x{:x} for hiding", window);
        }
    }
}

/// Unregister a window ID
#[no_mangle]
pub extern "C" fn stealth_unregister_window(window: Window) {
    if let Ok(mut windows) = HIDDEN_WINDOWS.write() {
        windows.retain(|&w| w != window);
    }
}

/// Set the stealth process PID
#[no_mangle]
pub extern "C" fn stealth_set_pid(pid: u32) {
    if let Ok(mut stealth_pid) = STEALTH_PID.write() {
        *stealth_pid = Some(pid);
        eprintln!("[STEALTH] Registered PID {} for stealth mode", pid);
    }
}

/// Check if a window should be hidden
fn is_hidden_window(window: Window) -> bool {
    HIDDEN_WINDOWS
        .read()
        .ok()
        .map(|windows| windows.contains(&window))
        .unwrap_or(false)
}

/// Check if current process should be hidden
fn is_stealth_process() -> bool {
    if let Ok(stealth_pid) = STEALTH_PID.read() {
        if let Some(pid) = *stealth_pid {
            return unsafe { libc::getpid() as u32 } == pid;
        }
    }
    false
}

// Hook XQueryTree to filter out hidden windows
hook! {
    unsafe fn XQueryTree(
        display: *mut Display,
        window: Window,
        root_return: *mut Window,
        parent_return: *mut Window,
        children_return: *mut *mut Window,
        nchildren_return: *mut c_uint,
    ) -> Status => my_XQueryTree {
        let result = real!(XQueryTree)(
            display,
            window,
            root_return,
            parent_return,
            children_return,
            nchildren_return,
        );

        if result != 0 && !children_return.is_null() && !nchildren_return.is_null() {
            let children = *children_return;
            let nchildren = *nchildren_return as usize;

            if !children.is_null() && nchildren > 0 {
                let mut filtered = Vec::new();
                let slice = std::slice::from_raw_parts(children, nchildren);

                for &child in slice {
                    if !is_hidden_window(child) {
                        filtered.push(child);
                    }
                }

                if filtered.len() < nchildren {
                    // Allocate new memory for filtered list
                    let new_children = libc::malloc(filtered.len() * std::mem::size_of::<Window>()) as *mut Window;
                    if !new_children.is_null() {
                        for (i, &child) in filtered.iter().enumerate() {
                            *new_children.add(i) = child;
                        }

                        // Free old memory and update pointers
                        libc::free(children as *mut c_void);
                        *children_return = new_children;
                        *nchildren_return = filtered.len() as c_uint;
                    }
                }
            }
        }

        result
    }
}

// Hook XGetWindowProperty to hide window properties
hook! {
    unsafe fn XGetWindowProperty(
        display: *mut Display,
        window: Window,
        property: Atom,
        long_offset: c_ulong,
        long_length: c_ulong,
        delete: c_int,
        req_type: Atom,
        actual_type_return: *mut Atom,
        actual_format_return: *mut c_int,
        nitems_return: *mut c_ulong,
        bytes_after_return: *mut c_ulong,
        prop_return: *mut *mut c_uchar,
    ) -> Status => my_XGetWindowProperty {
        if is_hidden_window(window) {
            // Return "property does not exist"
            if !actual_type_return.is_null() {
                *actual_type_return = 0; // None
            }
            if !actual_format_return.is_null() {
                *actual_format_return = 0;
            }
            if !nitems_return.is_null() {
                *nitems_return = 0;
            }
            if !bytes_after_return.is_null() {
                *bytes_after_return = 0;
            }
            if !prop_return.is_null() {
                *prop_return = std::ptr::null_mut();
            }
            return 1; // BadWindow
        }

        real!(XGetWindowProperty)(
            display,
            window,
            property,
            long_offset,
            long_length,
            delete,
            req_type,
            actual_type_return,
            actual_format_return,
            nitems_return,
            bytes_after_return,
            prop_return,
        )
    }
}

// Hook XGetWindowAttributes to hide window attributes
hook! {
    unsafe fn XGetWindowAttributes(
        display: *mut Display,
        window: Window,
        attributes_return: *mut c_void,
    ) -> Status => my_XGetWindowAttributes {
        if is_hidden_window(window) {
            return 0; // BadWindow
        }

        real!(XGetWindowAttributes)(display, window, attributes_return)
    }
}

// Hook XTranslateCoordinates to prevent coordinate mapping
hook! {
    unsafe fn XTranslateCoordinates(
        display: *mut Display,
        src_w: Window,
        dest_w: Window,
        src_x: c_int,
        src_y: c_int,
        dest_x_return: *mut c_int,
        dest_y_return: *mut c_int,
        child_return: *mut Window,
    ) -> Status => my_XTranslateCoordinates {
        let result = real!(XTranslateCoordinates)(
            display,
            src_w,
            dest_w,
            src_x,
            src_y,
            dest_x_return,
            dest_y_return,
            child_return,
        );

        // If the child is a hidden window, nullify it
        if result != 0 && !child_return.is_null() {
            let child = *child_return;
            if is_hidden_window(child) {
                *child_return = 0;
            }
        }

        result
    }
}

// Hook XQueryPointer to hide pointer child window
hook! {
    unsafe fn XQueryPointer(
        display: *mut Display,
        window: Window,
        root_return: *mut Window,
        child_return: *mut Window,
        root_x_return: *mut c_int,
        root_y_return: *mut c_int,
        win_x_return: *mut c_int,
        win_y_return: *mut c_int,
        mask_return: *mut c_uint,
    ) -> Status => my_XQueryPointer {
        let result = real!(XQueryPointer)(
            display,
            window,
            root_return,
            child_return,
            root_x_return,
            root_y_return,
            win_x_return,
            win_y_return,
            mask_return,
        );

        if result != 0 && !child_return.is_null() {
            let child = *child_return;
            if is_hidden_window(child) {
                *child_return = 0; // No child
            }
        }

        result
    }
}

// Hook XGetImage to prevent screenshot capture of overlay
hook! {
    unsafe fn XGetImage(
        display: *mut Display,
        drawable: Window,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
        plane_mask: c_ulong,
        format: c_int,
    ) -> *mut c_void => my_XGetImage {
        // If trying to capture a hidden window directly, return null
        if is_hidden_window(drawable) {
            return std::ptr::null_mut();
        }

        real!(XGetImage)(display, drawable, x, y, width, height, plane_mask, format)
    }
}

// Hook XFetchName to hide window names
hook! {
    unsafe fn XFetchName(
        display: *mut Display,
        window: Window,
        window_name_return: *mut *mut c_char,
    ) -> Status => my_XFetchName {
        if is_hidden_window(window) {
            if !window_name_return.is_null() {
                *window_name_return = std::ptr::null_mut();
            }
            return 0;
        }

        real!(XFetchName)(display, window, window_name_return)
    }
}

// Hook XGetWMName to hide WM_NAME property
hook! {
    unsafe fn XGetWMName(
        display: *mut Display,
        window: Window,
        text_prop_return: *mut c_void,
    ) -> Status => my_XGetWMName {
        if is_hidden_window(window) {
            return 0;
        }

        real!(XGetWMName)(display, window, text_prop_return)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_registration() {
        stealth_register_window(12345);
        assert!(is_hidden_window(12345));
        stealth_unregister_window(12345);
        assert!(!is_hidden_window(12345));
    }
}

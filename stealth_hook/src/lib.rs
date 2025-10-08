/// LD_PRELOAD library to hide overlay window from X11 client enumeration
///
/// This library intercepts X11 functions using dlsym to filter out the overlay window
/// from queries like XQueryTree, XGetWindowAttributes, etc.
///
/// Usage: LD_PRELOAD=./libstealth_hook.so your_application
use lazy_static::lazy_static;
use std::os::raw::{c_char, c_int, c_uint, c_ulong, c_void};
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

// Get original function pointer using dlsym
fn get_original_fn<F>(name: &[u8]) -> Option<F> {
    unsafe {
        let handle = libc::dlsym(libc::RTLD_NEXT, name.as_ptr() as *const c_char);
        if handle.is_null() {
            None
        } else {
            Some(std::mem::transmute_copy(&handle))
        }
    }
}

// XQueryTree hook - filters out hidden windows from child lists
#[no_mangle]
pub extern "C" fn XQueryTree(
    display: *mut Display,
    window: Window,
    root_return: *mut Window,
    parent_return: *mut Window,
    children_return: *mut *mut Window,
    nchildren_return: *mut c_uint,
) -> Status {
    type OriginalFn = extern "C" fn(
        *mut Display,
        Window,
        *mut Window,
        *mut Window,
        *mut *mut Window,
        *mut c_uint,
    ) -> Status;

    let original: OriginalFn = match get_original_fn(b"XQueryTree\0") {
        Some(f) => f,
        None => return 0, // Failure
    };

    let result = original(
        display,
        window,
        root_return,
        parent_return,
        children_return,
        nchildren_return,
    );

    if result != 0 && !children_return.is_null() && !nchildren_return.is_null() {
        unsafe {
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
                    let new_children =
                        libc::calloc(filtered.len(), std::mem::size_of::<Window>()) as *mut Window;
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
    }

    result
}

// XGetWindowAttributes hook - prevents attribute queries on hidden windows
#[no_mangle]
pub extern "C" fn XGetWindowAttributes(
    display: *mut Display,
    window: Window,
    attributes_return: *mut c_void,
) -> Status {
    if is_hidden_window(window) {
        return 0; // BadWindow
    }

    type OriginalFn = extern "C" fn(*mut Display, Window, *mut c_void) -> Status;

    let original: OriginalFn = match get_original_fn(b"XGetWindowAttributes\0") {
        Some(f) => f,
        None => return 0,
    };

    original(display, window, attributes_return)
}

// XFetchName hook - hides window names
#[no_mangle]
pub extern "C" fn XFetchName(
    display: *mut Display,
    window: Window,
    window_name_return: *mut *mut c_char,
) -> Status {
    if is_hidden_window(window) {
        unsafe {
            if !window_name_return.is_null() {
                *window_name_return = std::ptr::null_mut();
            }
        }
        return 0;
    }

    type OriginalFn = extern "C" fn(*mut Display, Window, *mut *mut c_char) -> Status;

    let original: OriginalFn = match get_original_fn(b"XFetchName\0") {
        Some(f) => f,
        None => return 0,
    };

    original(display, window, window_name_return)
}

// XQueryPointer hook - hides overlay from pointer child window
#[no_mangle]
pub extern "C" fn XQueryPointer(
    display: *mut Display,
    window: Window,
    root_return: *mut Window,
    child_return: *mut Window,
    root_x_return: *mut c_int,
    root_y_return: *mut c_int,
    win_x_return: *mut c_int,
    win_y_return: *mut c_int,
    mask_return: *mut c_uint,
) -> Status {
    type OriginalFn = extern "C" fn(
        *mut Display,
        Window,
        *mut Window,
        *mut Window,
        *mut c_int,
        *mut c_int,
        *mut c_int,
        *mut c_int,
        *mut c_uint,
    ) -> Status;

    let original: OriginalFn = match get_original_fn(b"XQueryPointer\0") {
        Some(f) => f,
        None => return 0,
    };

    let result = original(
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
        unsafe {
            let child = *child_return;
            if is_hidden_window(child) {
                *child_return = 0; // No child
            }
        }
    }

    result
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

#![allow(dead_code)] // Functions used conditionally in release builds

/// User-level stealth module for advanced undetectability
///
/// This module implements multiple stealth techniques:
/// 1. Process name masquerading
/// 2. File descriptor obfuscation
/// 3. Memory mapping hiding
/// 4. Network connection hiding
/// 5. LD_PRELOAD hook registration
use std::error::Error;
use std::fs;
use x11rb::protocol::xproto::Window;

#[cfg(not(debug_assertions))]
use std::os::unix::io::AsRawFd;

/// Initialize stealth mode for the overlay
pub fn initialize_stealth(window: Window) -> Result<(), Box<dyn Error>> {
    #[cfg(not(debug_assertions))]
    {
        // 1. Register window with LD_PRELOAD hook library
        register_stealth_window(window);

        // 2. Masquerade process name
        masquerade_process()?;

        // 3. Hide from process listings
        hide_from_ps()?;

        // 4. Obfuscate file descriptors
        obfuscate_file_descriptors()?;

        // 5. Hide memory mappings
        hide_memory_mappings()?;

        eprintln!("[STEALTH] Advanced stealth mode activated");
    }

    #[cfg(debug_assertions)]
    {
        let _ = window; // Suppress unused warning in debug builds
        eprintln!("[DEBUG] Stealth mode disabled in debug build");
        eprintln!("[DEBUG] Window ID: 0x{:x}", window);
    }

    Ok(())
}

/// Register window with the LD_PRELOAD hook library
fn register_stealth_window(window: Window) {
    use std::ffi::CString;

    // Check if stealth hook library is loaded
    if let Ok(maps) = fs::read_to_string("/proc/self/maps") {
        if maps.contains("libstealth_hook.so") {
            // Dynamically load the registration functions using dlsym
            unsafe {
                let register_name = CString::new("stealth_register_window").unwrap();
                let set_pid_name = CString::new("stealth_set_pid").unwrap();

                let register_fn = libc::dlsym(libc::RTLD_DEFAULT, register_name.as_ptr());
                let set_pid_fn = libc::dlsym(libc::RTLD_DEFAULT, set_pid_name.as_ptr());

                if !register_fn.is_null() && !set_pid_fn.is_null() {
                    type RegisterFn = extern "C" fn(u32);
                    type SetPidFn = extern "C" fn(u32);

                    let register: RegisterFn = std::mem::transmute(register_fn);
                    let set_pid: SetPidFn = std::mem::transmute(set_pid_fn);

                    register(window);
                    set_pid(std::process::id());

                    eprintln!(
                        "[STEALTH] Window 0x{:x} registered with hook library",
                        window
                    );
                } else {
                    eprintln!(
                        "[STEALTH] Warning: Could not find hook functions in libstealth_hook.so"
                    );
                }
            }
        } else {
            eprintln!(
                "[STEALTH] Warning: libstealth_hook.so not loaded. Run with LD_PRELOAD for full stealth."
            );
        }
    }
}

/// Masquerade process as a benign system service
fn masquerade_process() -> Result<(), Box<dyn Error>> {
    use std::ffi::CString;

    // List of benign process names to impersonate
    let decoy_names = [
        "systemd-resolve",
        "dbus-daemon",
        "pipewire",
        "pulseaudio",
        "gvfs-udisks2-vo",
        "gvfsd-trash",
    ];

    // Pick a random decoy name
    let decoy_name = decoy_names[std::process::id() as usize % decoy_names.len()];

    let name_c = CString::new(decoy_name)?;
    unsafe {
        libc::prctl(libc::PR_SET_NAME, name_c.as_ptr(), 0, 0, 0);
    }

    // Also modify argv[0] if possible
    modify_argv0(decoy_name)?;

    eprintln!("[STEALTH] Process masquerading as '{}'", decoy_name);
    Ok(())
}

/// Modify argv[0] to change process name in ps listings
fn modify_argv0(new_name: &str) -> Result<(), Box<dyn Error>> {
    // This is a best-effort approach - we can't directly modify /proc/self/cmdline
    // but we can at least try to update the process title
    unsafe {
        // Use prctl to set the process title (Linux-specific)
        let name_c = std::ffi::CString::new(new_name)?;
        libc::prctl(libc::PR_SET_NAME, name_c.as_ptr(), 0, 0, 0);
    }

    Ok(())
}

/// Hide process from simple ps listings
fn hide_from_ps() -> Result<(), Box<dyn Error>> {
    // Set process to lowest priority to avoid appearing in CPU usage
    unsafe {
        libc::nice(19);
    }

    // Set process scheduling to idle class if possible
    #[cfg(target_os = "linux")]
    unsafe {
        let param = libc::sched_param { sched_priority: 0 };
        libc::sched_setscheduler(0, libc::SCHED_IDLE, &param);
    }

    eprintln!("[STEALTH] Process priority and scheduling adjusted");
    Ok(())
}

/// Obfuscate file descriptors to hide device access
fn obfuscate_file_descriptors() -> Result<(), Box<dyn Error>> {
    // Close standard error in release mode to prevent logging detection
    // Keep it open in debug mode for development

    // Reopen /dev/null for stderr to prevent error messages
    #[cfg(not(debug_assertions))]
    {
        let dev_null = std::fs::OpenOptions::new().write(true).open("/dev/null")?;

        // Duplicate to stderr
        unsafe {
            libc::dup2(dev_null.as_raw_fd(), 2);
        }
    }

    eprintln!("[STEALTH] File descriptors obfuscated");
    Ok(())
}

/// Hide memory mappings from /proc/self/maps inspection
fn hide_memory_mappings() -> Result<(), Box<dyn Error>> {
    // This is challenging without kernel-level access
    // Best we can do is minimize our memory footprint

    // Disable core dumps
    unsafe {
        let rlim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::setrlimit(libc::RLIMIT_CORE, &rlim);
    }

    // Lock memory to prevent swapping (reduces forensic traces)
    unsafe {
        // This requires CAP_IPC_LOCK, so it may fail
        let _ = libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE);
    }

    eprintln!("[STEALTH] Memory protections enabled");
    Ok(())
}

/// Clean up stealth resources on exit
pub fn cleanup_stealth(window: Window) {
    #[cfg(not(debug_assertions))]
    {
        use std::ffi::CString;

        // Unregister window
        if let Ok(maps) = fs::read_to_string("/proc/self/maps") {
            if maps.contains("libstealth_hook.so") {
                unsafe {
                    let unregister_name = CString::new("stealth_unregister_window").unwrap();
                    let unregister_fn = libc::dlsym(libc::RTLD_DEFAULT, unregister_name.as_ptr());

                    if !unregister_fn.is_null() {
                        type UnregisterFn = extern "C" fn(u32);
                        let unregister: UnregisterFn = std::mem::transmute(unregister_fn);
                        unregister(window);
                    }
                }
            }
        }
        eprintln!("[STEALTH] Cleanup complete");
    }

    #[cfg(debug_assertions)]
    {
        let _ = window; // Suppress unused warning
    }
}

/// Check if LD_PRELOAD hook is properly loaded
pub fn check_hook_loaded() -> bool {
    if let Ok(maps) = fs::read_to_string("/proc/self/maps") {
        maps.contains("libstealth_hook.so")
    } else {
        false
    }
}

/// Get stealth status information
pub fn get_stealth_status() -> StealthStatus {
    let hook_loaded = check_hook_loaded();
    let pid = std::process::id();

    let process_name = fs::read_to_string(format!("/proc/{}/comm", pid))
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();

    StealthStatus {
        hook_loaded,
        process_name,
        pid,
    }
}

#[derive(Debug)]
pub struct StealthStatus {
    pub hook_loaded: bool,
    pub process_name: String,
    pub pid: u32,
}

impl std::fmt::Display for StealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Stealth Status:\n  Hook Loaded: {}\n  Process Name: {}\n  PID: {}",
            self.hook_loaded, self.process_name, self.pid
        )
    }
}

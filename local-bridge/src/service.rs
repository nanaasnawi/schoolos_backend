use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Get installation directory in %LOCALAPPDATA%\SchoolOS
pub fn get_install_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let dir = PathBuf::from(base).join("SchoolOS");
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Get the permanent installed executable path
pub fn get_installed_exe_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = get_install_dir()?;
    Ok(dir.join("schoolos-bridge.exe"))
}

/// Check if the currently running binary is already the installed binary
pub fn is_running_from_install_dir(current_exe: &Path) -> bool {
    if let Ok(installed) = get_installed_exe_path() {
        if let (Ok(cur_canon), Ok(inst_canon)) = (current_exe.canonicalize(), installed.canonicalize()) {
            return cur_canon == inst_canon;
        }
        return current_exe == installed;
    }
    false
}

/// Show native graphical message box on Windows
pub fn show_native_message(title: &str, message: &str, is_error: bool) {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let wide_title: Vec<u16> = OsStr::new(title).encode_wide().chain(std::iter::once(0)).collect();
        let wide_msg: Vec<u16> = OsStr::new(message).encode_wide().chain(std::iter::once(0)).collect();

        extern "system" {
            fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, utype: u32) -> i32;
        }

        const MB_OK: u32 = 0x00000000;
        const MB_ICONINFORMATION: u32 = 0x00000040;
        const MB_ICONERROR: u32 = 0x00000010;

        let flags = MB_OK | if is_error { MB_ICONERROR } else { MB_ICONINFORMATION };
        unsafe {
            MessageBoxW(0, wide_msg.as_ptr(), wide_title.as_ptr(), flags);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let prefix = if is_error { "ERROR" } else { "INFO" };
        println!("[{}] {}: {}", prefix, title, message);
    }
}

/// Register autostart in Windows Registry (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)
pub fn register_windows_autostart(exe_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        let exe_str = format!("\"{}\" --daemon", exe_path.to_string_lossy());
        let status = Command::new("reg")
            .args(&[
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "SchoolOSBridge",
                "/t",
                "REG_SZ",
                "/d",
                &exe_str,
                "/f",
            ])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .status()?;

        if !status.success() {
            return Err("Gagal mendaftarkan autostart ke Windows Registry.".into());
        }
    }
    Ok(())
}

/// Unregister autostart from Windows Registry
pub fn unregister_windows_autostart() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("reg")
            .args(&[
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "SchoolOSBridge",
                "/f",
            ])
            .creation_flags(0x08000000)
            .status();

        // Kill any existing daemon instances
        let _ = Command::new("taskkill")
            .args(&["/F", "/IM", "schoolos-bridge.exe"])
            .creation_flags(0x08000000)
            .status();
        let _ = Command::new("taskkill")
            .args(&["/F", "/IM", "schoolos-sync.exe"])
            .creation_flags(0x08000000)
            .status();
    }
    Ok(())
}

/// Spawn a detached, completely invisible background daemon process
pub fn spawn_background_daemon(exe_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        Command::new(exe_path)
            .arg("--daemon")
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new(exe_path).arg("--daemon").spawn()?;
    }

    Ok(())
}

/// Full self-install workflow: copies binary, sets registry, spawns daemon, and shows UI alert
pub fn perform_self_install(current_exe: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let target_exe = get_installed_exe_path()?;

    // 1. Terminate old running instances (excluding current process PID) to unlock file write
    #[cfg(target_os = "windows")]
    {
        let current_pid = std::process::id();
        let ps_cmd = format!(
            "Get-Process -Name 'schoolos-bridge','schoolos-sync' -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {} }} | Stop-Process -Force",
            current_pid
        );
        let _ = Command::new("powershell")
            .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_cmd])
            .creation_flags(0x08000000)
            .status();
        std::thread::sleep(std::time::Duration::from_millis(600));
    }

    // 2. Copy current executable to %LOCALAPPDATA%\SchoolOS\schoolos-bridge.exe
    if let Some(parent) = target_exe.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(current_exe, &target_exe)?;

    // 3. Register autostart in Windows Registry (HKCU Run)
    register_windows_autostart(&target_exe)?;

    // 4. Spawn the background daemon immediately
    spawn_background_daemon(&target_exe)?;

    // 5. Show success notification dialog to the operator
    show_native_message(
        "School OS Bridge — Dapodik Auto-Sync",
        "✓ School OS Bridge berhasil dipasang dan diaktifkan!\n\n\
        • Status: Aktif di latar belakang Windows (Port 5775)\n\
        • Autostart: Otomatis aktif setiap komputer dinyalakan\n\
        • Mode: Native Silent Daemon (Tanpa jendela CMD hitam)\n\n\
        Anda dapat langsung membuka dashboard School OS di browser dan klik 'Tarik Data dari Dapodik' sekarang juga.",
        false,
    );

    Ok(())
}

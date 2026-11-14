//! Windows 11 native DWM window chrome and OS profile helpers.

#[derive(Debug, Clone)]
pub struct Win11OsProfile {
    pub edition: String,
    pub display_version: String,
    pub build_number: u32,
    pub ubr: u32,
    pub hostname: String,
    pub arch: String,
    pub is_windows_11: bool,
}

#[cfg(target_os = "windows")]
pub fn apply_win11_window_chrome(
    window: &tauri::WebviewWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };

    let hwnd = window.hwnd()?;
    let dark_mode: i32 = 1;
    // DWM expects COLORREF: 0x00BBGGRR.
    // Caption matches `.dark` `--background` (#111116) seamlessly while keeping the OS default border.
    let caption_color = 0x00161111_u32;
    // Soft crisp foreground (#F2F2F5) for native Windows 11 titlebar text & caption glyphs.
    let text_color = 0x00F5F2F2_u32;

    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const _ as _,
            std::mem::size_of_val(&dark_mode) as u32,
        );

        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &caption_color as *const _ as _,
            std::mem::size_of_val(&caption_color) as u32,
        );

        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TEXT_COLOR,
            &text_color as *const _ as _,
            std::mem::size_of_val(&text_color) as u32,
        );
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn read_hklm_string(subkey: &str, value_name: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};

    let subkey_w = wide_null(subkey);
    let value_w = wide_null(value_name);
    let mut byte_len: u32 = 0;

    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut byte_len),
        )
    };
    if status.is_err() || byte_len < 2 {
        return None;
    }

    let mut buffer = vec![0u16; (byte_len as usize).div_ceil(2)];
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr() as *mut _),
            Some(&mut byte_len),
        )
    };
    if status.is_err() {
        return None;
    }

    let nul_pos = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..nul_pos]))
}

#[cfg(target_os = "windows")]
fn read_hklm_dword(subkey: &str, value_name: &str) -> Option<u32> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD};

    let subkey_w = wide_null(subkey);
    let value_w = wide_null(value_name);
    let mut value: u32 = 0;
    let mut byte_len = std::mem::size_of::<u32>() as u32;

    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut byte_len),
        )
    };

    status.is_ok().then_some(value)
}

pub fn read_win11_os_profile() -> Win11OsProfile {
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "LOCALHOST".to_string());
    let arch = std::env::consts::ARCH.to_string();

    #[cfg(target_os = "windows")]
    {
        const NT_CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

        let raw_product = read_hklm_string(NT_CURRENT_VERSION, "ProductName")
            .unwrap_or_else(|| "Windows".to_string());
        let display_version = read_hklm_string(NT_CURRENT_VERSION, "DisplayVersion")
            .unwrap_or_else(|| "Unknown".to_string());
        let build_number = read_hklm_string(NT_CURRENT_VERSION, "CurrentBuildNumber")
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let ubr = read_hklm_dword(NT_CURRENT_VERSION, "UBR").unwrap_or(0);
        let is_windows_11 = build_number >= 22000;

        // Microsoft keeps `ProductName` as "Windows 10 ..." in HKLM even on Windows 11 (Build >= 22000).
        let edition = if is_windows_11 && raw_product.starts_with("Windows 10") {
            raw_product.replacen("Windows 10", "Windows 11", 1)
        } else {
            raw_product
        };

        Win11OsProfile {
            edition,
            display_version,
            build_number,
            ubr,
            hostname,
            arch,
            is_windows_11,
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Win11OsProfile {
            edition: std::env::consts::OS.to_string(),
            display_version: "N/A".to_string(),
            build_number: 0,
            ubr: 0,
            hostname,
            arch,
            is_windows_11: false,
        }
    }
}

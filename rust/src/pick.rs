//! 目录选择器。
//!
//! 不引 rfd/gtk 之类依赖：它们的体积和编译时间都不值一个文件夹选择框，
//! 直接调 Windows 的 IFileDialog（Vista 之后的标准实现）。

use std::path::PathBuf;

#[cfg(windows)]
pub fn pick_directory() -> Option<PathBuf> {
    use std::ffi::c_void;

    // COM 的 IFileDialog 需要一整套 vtable 手写，容易出错且难以维护。
    // 改用 SHBrowseForFolder：一个函数调用，返回 PIDL 再用 SHGetPathFromIDList 取路径。
    // 它的 UI 比 IFileDialog 旧，但功能一致，且完全不用管 COM 初始化。
    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(pv: *mut c_void, flags: u32) -> i32;
        fn CoUninitialize();
    }
    #[link(name = "shell32")]
    extern "system" {
        fn SHBrowseForFolderW(lpbi: *const BrowseInfoW) -> *mut c_void;
        fn SHGetPathFromIDListW(pidl: *const c_void, pszPath: *mut u16) -> i32;
    }

    #[repr(C)]
    struct BrowseInfoW {
        hwnd_owner: *mut c_void,
        pidl_root: *mut c_void,
        display_name: *mut u16,
        title: *mut u16,
        flags: u32,
        callback: *mut c_void,
        l_param: isize,
        image: i32,
    }

    // BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE
    const FLAGS: u32 = 0x0001 | 0x0040;

    unsafe {
        // COINIT_APARTMENTTHREADED
        let hr = CoInitializeEx(std::ptr::null_mut(), 0x2);
        let should_uninit = hr >= 0;

        let title: Vec<u16> = "选择项目目录\0".encode_utf16().collect();
        let bi = BrowseInfoW {
            hwnd_owner: std::ptr::null_mut(),
            pidl_root: std::ptr::null_mut(),
            display_name: std::ptr::null_mut(),
            title: title.as_ptr() as *mut u16,
            flags: FLAGS,
            callback: std::ptr::null_mut(),
            l_param: 0,
            image: 0,
        };

        let pidl = SHBrowseForFolderW(&bi);
        let mut out = None;
        if !pidl.is_null() {
            let mut buf = [0u16; 1024];
            if SHGetPathFromIDListW(pidl, buf.as_mut_ptr()) != 0 {
                let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
                let s = String::from_utf16_lossy(&buf[..len]);
                if !s.is_empty() {
                    out = Some(PathBuf::from(s));
                }
            }
            // PIDL 由 shell 分配，需要 ILFree；这里用 CoTaskMemFree 等价回收
            #[link(name = "ole32")]
            extern "system" {
                fn CoTaskMemFree(pv: *mut c_void);
            }
            CoTaskMemFree(pidl);
        }
        if should_uninit {
            CoUninitialize();
        }
        out
    }
}

#[cfg(not(windows))]
pub fn pick_directory() -> Option<PathBuf> {
    None
}

/// 从路径取最后一段作为默认名称。
pub fn dir_basename(path: &str) -> String {
    path.trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .to_string()
}

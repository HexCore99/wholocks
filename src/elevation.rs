use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt, path::Path};

const SW_SHOWNORMAL: i32 = 1;

#[link(name = "shell32")]
unsafe extern "system" {
    fn IsUserAnAdmin() -> i32;
    fn ShellExecuteW(
        hwnd: isize,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show_command: i32,
    ) -> isize;
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

pub fn is_elevated() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

pub fn relaunch_elevated(target: &Path) -> io::Result<()> {
    let executable = std::env::current_exe()?;
    let operation = wide(OsStr::new("runas"));
    let executable = wide(executable.as_os_str());
    let parameters = wide(OsStr::new(&format!(r#""{}""#, target.display())));
    let result = unsafe {
        ShellExecuteW(
            0,
            operation.as_ptr(),
            executable.as_ptr(),
            parameters.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    if result > 32 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(result as i32))
    }
}

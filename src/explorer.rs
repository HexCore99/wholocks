use std::{
    env,
    ffi::{OsStr, c_void},
    io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const MENU_TEXT: &str = "WhoLocks";
const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
const SHCNF_IDLIST: u32 = 0;

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHChangeNotify(event_id: i32, flags: u32, item1: *const c_void, item2: *const c_void);
}

fn refresh_shell() {
    // Association changes invalidate the shell's cached menu icons.
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

const MENU_KEY: &str = r"HKCU\Software\Classes\AllFilesystemObjects\shell\WhoLocks";
const LEGACY_MENU_KEYS: [&str; 6] = [
    r"HKCU\Software\Classes\*\shell\WhoLocks",
    r"HKCU\Software\Classes\Directory\shell\WhoLocks",
    r"HKCU\Software\Classes\*\shell\WhoLocks.FindLocks",
    r"HKCU\Software\Classes\Directory\shell\WhoLocks.FindLocks",
    r"HKCU\Software\Classes\*\shell\OpenWithWhoLocks",
    r"HKCU\Software\Classes\Directory\shell\OpenWithWhoLocks",
];

fn reg<I, S>(arguments: I) -> io::Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("reg.exe").args(arguments).output()
}

fn command_error(action: &str, output: Output) -> io::Error {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let detail = if stderr.is_empty() {
        format!("reg.exe exited with {}", output.status)
    } else {
        stderr
    };
    io::Error::other(format!("could not {action}: {detail}"))
}

fn add_value(key: &str, name: Option<&str>, value: &str) -> io::Result<()> {
    let mut arguments = vec!["add", key];
    match name {
        Some(name) => arguments.extend(["/v", name]),
        None => arguments.push("/ve"),
    }
    arguments.extend(["/t", "REG_SZ", "/d", value, "/f"]);
    let output = reg(arguments)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(command_error("write the Explorer registration", output))
    }
}

fn gui_executable() -> io::Result<PathBuf> {
    let current = env::current_exe()?;
    let gui = current.with_file_name("wholocks-gui.exe");
    if gui.is_file() {
        Ok(gui)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{} was not found; build both release executables before installing",
                gui.display()
            ),
        ))
    }
}

fn register_key(key: &str, gui: &Path) -> io::Result<()> {
    let icon = format!(r#""{}",0"#, gui.display());
    let command = format!(r#""{}" "%1""#, gui.display());
    add_value(key, None, MENU_TEXT)?;
    add_value(key, Some("Icon"), &icon)?;
    add_value(&format!(r"{key}\command"), None, &command)
}

fn delete_key(key: &str) -> io::Result<()> {
    let output = reg(["delete", key, "/f"])?;
    if output.status.success() || output.status.code() == Some(1) {
        Ok(())
    } else {
        Err(command_error("remove the Explorer registration", output))
    }
}

pub fn install_context_menu() -> io::Result<PathBuf> {
    let gui = gui_executable()?;
    for key in LEGACY_MENU_KEYS {
        delete_key(key)?;
    }
    register_key(MENU_KEY, &gui)?;
    refresh_shell();
    Ok(gui)
}

pub fn uninstall_context_menu() -> io::Result<()> {
    for key in std::iter::once(MENU_KEY).chain(LEGACY_MENU_KEYS) {
        delete_key(key)?;
    }
    refresh_shell();
    Ok(())
}

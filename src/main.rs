use std::{
    collections::{HashMap, HashSet},
    env,
    error::Error,
    ffi::c_void,
    io,
    path::{Path, PathBuf},
};

type Handle = isize;
type NtStatus = i32;

const SYSTEM_EXTENDED_HANDLE_INFORMATION: u32 = 64;
const OBJECT_NAME_INFORMATION: u32 = 1;
const STATUS_INFO_LENGTH_MISMATCH: NtStatus = 0xC000_0004_u32 as NtStatus;
const STATUS_BUFFER_TOO_SMALL: NtStatus = 0xC000_0023_u32 as NtStatus;
const STATUS_BUFFER_OVERFLOW: NtStatus = 0x8000_0005_u32 as NtStatus;
const PROCESS_DUP_HANDLE: u32 = 0x0040;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const PROCESS_TERMINATE: u32 = 0x0001;
const DUPLICATE_SAME_ACCESS: u32 = 0x0000_0002;
const INVALID_HANDLE_VALUE: Handle = -1;
const FILE_TYPE_DISK: u32 = 0x0000_0001;

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

#[repr(C)]
struct SystemHandleTableEntryInfoEx {
    object: *mut c_void,
    unique_process_id: usize,
    handle_value: usize,
    granted_access: u32,
    creator_back_trace_index: u16,
    object_type_index: u16,
    handle_attributes: u32,
    reserved: u32,
}

#[derive(Debug)]
struct Lock {
    pid: u32,
    process: String,
    path: String,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQuerySystemInformation(
        system_information_class: u32,
        system_information: *mut c_void,
        system_information_length: u32,
        return_length: *mut u32,
    ) -> NtStatus;

    fn NtQueryObject(
        handle: Handle,
        object_information_class: u32,
        object_information: *mut c_void,
        object_information_length: u32,
        return_length: *mut u32,
    ) -> NtStatus;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CloseHandle(handle: Handle) -> i32;
    fn DuplicateHandle(
        source_process_handle: Handle,
        source_handle: Handle,
        target_process_handle: Handle,
        target_handle: *mut Handle,
        desired_access: u32,
        inherit_handle: i32,
        options: u32,
    ) -> i32;
    fn GetCurrentProcess() -> Handle;
    fn GetFileType(handle: Handle) -> u32;
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
    fn QueryDosDeviceW(device_name: *const u16, target_path: *mut u16, max: u32) -> u32;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: u32,
        exe_name: *mut u16,
        size: *mut u32,
    ) -> i32;
    fn TerminateProcess(process: Handle, exit_code: u32) -> i32;
}

fn nt_query_buffer(query: impl Fn(*mut c_void, u32, *mut u32) -> NtStatus) -> io::Result<Vec<u8>> {
    let mut size = 64 * 1024_usize;

    loop {
        let mut buffer = vec![0_u8; size];
        let mut needed = 0_u32;
        let status = query(buffer.as_mut_ptr().cast(), size as u32, &mut needed);

        if status >= 0 {
            return Ok(buffer);
        }

        if matches!(
            status,
            STATUS_INFO_LENGTH_MISMATCH | STATUS_BUFFER_TOO_SMALL | STATUS_BUFFER_OVERFLOW
        ) {
            size = size.max(needed as usize).saturating_mul(2);
            continue;
        }

        return Err(io::Error::from_raw_os_error(status));
    }
}

fn unicode_string(value: &UnicodeString) -> String {
    if value.buffer.is_null() || value.length == 0 {
        return String::new();
    }

    let units = unsafe { std::slice::from_raw_parts(value.buffer, value.length as usize / 2) };
    String::from_utf16_lossy(units)
}

fn object_string(handle: Handle, information_class: u32) -> Option<String> {
    let buffer = nt_query_buffer(|buffer, size, needed| unsafe {
        NtQueryObject(handle, information_class, buffer, size, needed)
    })
    .ok()?;

    let value = unsafe { &*buffer.as_ptr().cast::<UnicodeString>() };
    Some(unicode_string(value))
}

fn is_disk_handle(handle: Handle) -> bool {
    unsafe { GetFileType(handle) == FILE_TYPE_DISK }
}

fn process_name(process: Handle) -> String {
    let mut buffer = vec![0_u16; 32_768];
    let mut length = buffer.len() as u32;
    let success =
        unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) != 0 };

    if success {
        String::from_utf16_lossy(&buffer[..length as usize])
    } else {
        "<unknown process>".to_string()
    }
}

fn to_device_path(path: &Path) -> io::Result<String> {
    let path = path.canonicalize()?;
    let path = path.to_string_lossy();
    let path = path.strip_prefix(r"\\?\").unwrap_or(&path);
    let drive = path
        .get(..2)
        .filter(|drive| drive.ends_with(':'))
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "path must use a drive letter")
        })?;
    let drive_wide: Vec<u16> = drive.encode_utf16().chain(Some(0)).collect();
    let mut device = vec![0_u16; 32_768];
    let length = unsafe {
        QueryDosDeviceW(
            drive_wide.as_ptr(),
            device.as_mut_ptr(),
            device.len() as u32,
        )
    };

    if length == 0 {
        return Err(io::Error::last_os_error());
    }

    let device_length = device[..length as usize]
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(length as usize);
    let device = String::from_utf16_lossy(&device[..device_length]);
    Ok(format!("{}{}", device, &path[2..]))
}

fn path_matches(object_path: &str, target: &str, target_is_directory: bool) -> bool {
    if object_path.eq_ignore_ascii_case(target) {
        return true;
    }

    target_is_directory
        && object_path
            .get(..target.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(target))
        && object_path
            .get(target.len()..)
            .is_some_and(|suffix| suffix.starts_with('\\'))
}

fn find_locks(path: &Path) -> io::Result<Vec<Lock>> {
    let target = to_device_path(path)?;
    let target_is_directory = path.is_dir();
    let handles = nt_query_buffer(|buffer, size, needed| unsafe {
        NtQuerySystemInformation(SYSTEM_EXTENDED_HANDLE_INFORMATION, buffer, size, needed)
    })?;

    let count = unsafe { *(handles.as_ptr().cast::<usize>()) };
    let entries = unsafe {
        std::slice::from_raw_parts(
            handles.as_ptr().add(2 * std::mem::size_of::<usize>())
                as *const SystemHandleTableEntryInfoEx,
            count,
        )
    };
    let current_process = unsafe { GetCurrentProcess() };
    let mut locks = Vec::new();
    let mut processes = HashMap::new();

    for entry in entries {
        let Ok(pid) = u32::try_from(entry.unique_process_id) else {
            continue;
        };
        let process = *processes.entry(pid).or_insert_with(|| unsafe {
            OpenProcess(
                PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        });

        if process == 0 {
            continue;
        }

        let mut duplicate = 0;
        let copied = unsafe {
            DuplicateHandle(
                process,
                entry.handle_value as Handle,
                current_process,
                &mut duplicate,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            ) != 0
        };

        if copied && is_disk_handle(duplicate) {
            if let Some(object_path) = object_string(duplicate, OBJECT_NAME_INFORMATION) {
                if path_matches(&object_path, &target, target_is_directory) {
                    locks.push(Lock {
                        pid,
                        process: process_name(process),
                        path: object_path,
                    });
                }
            }
        }

        if duplicate != 0 && duplicate != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(duplicate) };
        }
    }

    for process in processes.into_values() {
        if process != 0 && process != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(process) };
        }
    }

    Ok(locks)
}

fn terminate_process(pid: u32) -> io::Result<()> {
    let process = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };

    if process == 0 {
        return Err(io::Error::last_os_error());
    }

    let terminated = unsafe { TerminateProcess(process, 1) != 0 };
    let error = (!terminated).then(io::Error::last_os_error);
    unsafe { CloseHandle(process) };

    error.map_or(Ok(()), Err)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let Some(first_argument) = arguments.next() else {
        eprintln!("Usage:");
        eprintln!(r#"  wholocks "C:\path\to\file-or-folder""#);
        eprintln!(r#"  wholocks --kill "C:\path\to\file-or-folder""#);
        return Ok(());
    };

    let kill = first_argument == "--kill";
    let path = if kill {
        let Some(path) = arguments.next() else {
            eprintln!("Missing path after --kill.");
            return Ok(());
        };
        path
    } else {
        first_argument
    };

    if arguments.next().is_some() {
        eprintln!("Expected exactly one path.");
        return Ok(());
    }

    let path = PathBuf::from(path);

    if !path.exists() {
        eprintln!("Path does not exist:");
        eprintln!("{}", path.display());
        return Ok(());
    }

    println!("Scanning Windows file and folder handles...");
    let locks = find_locks(&path)?;

    if locks.is_empty() {
        println!("No matching handles found.");
        println!("Windows can hide handles owned by protected processes.");
        return Ok(());
    }

    let pids: HashSet<u32> = locks
        .iter()
        .map(|lock| lock.pid)
        .filter(|&pid| pid != std::process::id())
        .collect();

    for lock in &locks {
        println!();
        println!("Process : {}", lock.process);
        println!("PID     : {}", lock.pid);
        println!("Handle  : {}", lock.path);
    }

    if kill {
        let mut failures = Vec::new();

        for pid in pids {
            match terminate_process(pid) {
                Ok(()) => println!("Terminated PID: {pid}"),
                Err(error) => {
                    eprintln!("Could not terminate PID {pid}: {error}");
                    failures.push(pid);
                }
            }
        }

        if !failures.is_empty() {
            return Err(io::Error::other(format!(
                "could not terminate {} process(es)",
                failures.len()
            ))
            .into());
        }
    }

    Ok(())
}

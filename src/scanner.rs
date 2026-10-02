use std::{
    collections::{HashMap, HashSet},
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
const ERROR_ACCESS_DENIED: i32 = 5;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockingProcess {
    pub pid: u32,
    pub executable: PathBuf,
    pub matched_handles: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub processes: Vec<LockingProcess>,
    pub inaccessible_process_count: usize,
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

fn object_string(handle: Handle) -> Option<String> {
    let buffer = nt_query_buffer(|buffer, size, needed| unsafe {
        NtQueryObject(handle, OBJECT_NAME_INFORMATION, buffer, size, needed)
    })
    .ok()?;
    let value = unsafe { &*buffer.as_ptr().cast::<UnicodeString>() };
    Some(unicode_string(value))
}

fn process_path(process: Handle) -> PathBuf {
    let mut buffer = vec![0_u16; 32_768];
    let mut length = buffer.len() as u32;
    let success =
        unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) != 0 };

    if success {
        PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]))
    } else {
        PathBuf::from("<unknown process>")
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

pub fn find_locks(path: &Path) -> io::Result<ScanReport> {
    let target = to_device_path(path)?;
    let target_is_directory = path.is_dir();
    let handles = nt_query_buffer(|buffer, size, needed| unsafe {
        NtQuerySystemInformation(SYSTEM_EXTENDED_HANDLE_INFORMATION, buffer, size, needed)
    })?;

    let count = unsafe { *(handles.as_ptr().cast::<usize>()) };
    let entries = unsafe {
        std::slice::from_raw_parts(
            handles
                .as_ptr()
                .add(2 * std::mem::size_of::<usize>())
                .cast::<SystemHandleTableEntryInfoEx>(),
            count,
        )
    };
    let current_process = unsafe { GetCurrentProcess() };
    let current_pid = std::process::id();
    let mut opened_processes = HashMap::new();
    let mut inaccessible_pids = HashSet::new();
    let mut matches: HashMap<u32, LockingProcess> = HashMap::new();

    for entry in entries {
        let Ok(pid) = u32::try_from(entry.unique_process_id) else {
            continue;
        };
        if pid == current_pid {
            continue;
        }

        let process = *opened_processes.entry(pid).or_insert_with(|| unsafe {
            OpenProcess(
                PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        });

        if process == 0 {
            if io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED) {
                inaccessible_pids.insert(pid);
            }
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

        if copied
            && unsafe { GetFileType(duplicate) == FILE_TYPE_DISK }
            && let Some(object_path) = object_string(duplicate)
            && path_matches(&object_path, &target, target_is_directory)
        {
            let matched = matches.entry(pid).or_insert_with(|| LockingProcess {
                pid,
                executable: process_path(process),
                matched_handles: Vec::new(),
            });
            if !matched
                .matched_handles
                .iter()
                .any(|path| path.eq_ignore_ascii_case(&object_path))
            {
                matched.matched_handles.push(object_path);
            }
        }

        if duplicate != 0 && duplicate != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(duplicate) };
        }
    }

    for process in opened_processes.into_values() {
        if process != 0 && process != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(process) };
        }
    }

    let mut processes: Vec<_> = matches.into_values().collect();
    processes.sort_by_key(|process| process.pid);
    Ok(ScanReport {
        processes,
        inaccessible_process_count: inaccessible_pids.len(),
    })
}

pub fn terminate_process(pid: u32) -> io::Result<()> {
    if pid == std::process::id() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "WhoLocks will not terminate itself",
        ));
    }

    let process = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if process == 0 {
        return Err(io::Error::last_os_error());
    }

    let terminated = unsafe { TerminateProcess(process, 1) != 0 };
    let error = (!terminated).then(io::Error::last_os_error);
    unsafe { CloseHandle(process) };
    error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::path_matches;

    #[test]
    fn file_matches_only_the_exact_path() {
        assert!(path_matches(
            r"\Device\HarddiskVolume3\work\file.txt",
            r"\Device\HarddiskVolume3\work\file.txt",
            false
        ));
        assert!(!path_matches(
            r"\Device\HarddiskVolume3\work\file.txt.bak",
            r"\Device\HarddiskVolume3\work\file.txt",
            false
        ));
    }

    #[test]
    fn directory_matches_descendants_at_a_separator_boundary() {
        let target = r"\Device\HarddiskVolume3\work";
        assert!(path_matches(
            r"\Device\HarddiskVolume3\work\src\main.rs",
            target,
            true
        ));
        assert!(!path_matches(
            r"\Device\HarddiskVolume3\worker\main.rs",
            target,
            true
        ));
    }

    #[test]
    fn matching_is_ascii_case_insensitive() {
        assert!(path_matches(
            r"\DEVICE\HARDDISKVOLUME3\WORK\FILE.TXT",
            r"\Device\HarddiskVolume3\work\file.txt",
            false
        ));
    }
}

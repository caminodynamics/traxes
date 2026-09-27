//! FILE_WRITE only: each child is opened relative to a retained directory handle.
//! No checked pathname is reopened for writing. Windows denies write/delete sharing
//! on the whole chain; Unix pins directory/file objects even if names are renamed.
//! This prevents symlink/reparse substitution, not partial writes or crash loss.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

fn denied(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

fn absolute_path(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().is_empty() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(denied("FILE_WRITE requires a nonempty path without parent traversal"));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else if path.has_root() || path.components().any(|c| matches!(c, Component::Prefix(_))) {
        Err(denied("FILE_WRITE does not support drive-relative or rooted-relative paths"))
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

struct OpenedWrite {
    file: File,
    // Keep all parents alive until after the write (including on Windows).
    _parents: Vec<File>,
}

impl OpenedWrite {
    fn write(mut self, content: &[u8]) -> io::Result<()> {
        // Open never truncates: metadata validation precedes any data change,
        // and both operations below use that same owned handle.
        self.file.set_len(0)?;
        self.file.write_all(content)
    }
}

pub(super) fn write(path: &Path, content: &[u8]) -> io::Result<()> {
    open(path)?.write(content)
}

#[cfg(windows)]
fn open(path: &Path) -> io::Result<OpenedWrite> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::MetadataExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::path::Prefix;
    use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
    use windows_sys::Wdk::Storage::FileSystem::{
        NtCreateFile, FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN,
        FILE_OPEN_IF, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
    };
    use windows_sys::Win32::Foundation::{HANDLE, OBJ_CASE_INSENSITIVE, RtlNtStatusToDosError, UNICODE_STRING};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT, FILE_GENERIC_WRITE,
        FILE_READ_ATTRIBUTES, FILE_LIST_DIRECTORY, FILE_SHARE_READ, SYNCHRONIZE,
    };
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

    fn open_component(parent: HANDLE, name: &OsStr, directory: bool) -> io::Result<File> {
        let mut wide: Vec<u16> = name.encode_wide().collect();
        if wide.contains(&0) || wide.len() > (u16::MAX as usize / 2) {
            return Err(denied("Invalid FILE_WRITE component"));
        }
        let mut name = UNICODE_STRING {
            Length: (wide.len() * 2) as u16,
            MaximumLength: (wide.len() * 2) as u16,
            Buffer: wide.as_mut_ptr(),
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: parent,
            ObjectName: &mut name,
            Attributes: OBJ_CASE_INSENSITIVE,
            SecurityDescriptor: std::ptr::null_mut(),
            SecurityQualityOfService: std::ptr::null_mut(),
        };
        let mut handle = std::ptr::null_mut();
        let mut status_block: IO_STATUS_BLOCK = unsafe { std::mem::zeroed() };
        // SAFETY: all input buffers remain live for this synchronous call; the
        // parent is owned by the caller. On success the returned handle is owned
        // exactly once by File. No FILE_OVERWRITE/TRUNCATE occurs before validation.
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                // Metadata-only opens do not participate in Windows sharing
                // checks. Directory read access is required to lock the chain.
                FILE_READ_ATTRIBUTES | SYNCHRONIZE | if directory { FILE_LIST_DIRECTORY } else { FILE_GENERIC_WRITE },
                &attributes, &mut status_block, std::ptr::null(), FILE_ATTRIBUTE_NORMAL,
                FILE_SHARE_READ, if directory { FILE_OPEN } else { FILE_OPEN_IF },
                FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT
                    | if directory { FILE_DIRECTORY_FILE } else { FILE_NON_DIRECTORY_FILE },
                std::ptr::null(), 0,
            )
        };
        if status < 0 {
            let error = io::Error::from_raw_os_error(unsafe { RtlNtStatusToDosError(status) } as i32);
            return Err(io::Error::new(error.kind(), format!("FILE_WRITE open {:?} failed (NTSTATUS {status:#x}): {error}", String::from_utf16_lossy(&wide))));
        }
        let file = unsafe { File::from_raw_handle(handle) };
        let metadata = file.metadata()?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
            return Err(denied("FILE_WRITE rejects reparse points and non-regular targets"));
        }
        Ok(file)
    }

    let absolute = absolute_path(path)?;
    let mut components = absolute.components();
    let drive = match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) => drive,
            _ => return Err(denied("FILE_WRITE supports ordinary local drive paths only")),
        },
        _ => return Err(denied("FILE_WRITE requires a drive root")),
    };
    if components.next() != Some(Component::RootDir) {
        return Err(denied("FILE_WRITE requires an absolute drive root"));
    }
    let names: Vec<_> = components.filter(|c| *c != Component::CurDir).map(|c| {
        match c {
            Component::Normal(name) => {
                let wide: Vec<u16> = name.encode_wide().collect();
                // Native relative names must not introduce streams or separators.
                if wide.iter().any(|c| [0, 58, 47, 92].contains(c))
                    || matches!(wide.last(), Some(32 | 46)) {
                    Err(denied("Unsupported FILE_WRITE component"))
                } else { Ok(name) }
            }
            _ => Err(denied("Unsupported FILE_WRITE path")),
        }
    }).collect::<io::Result<_>>()?;
    if names.is_empty() { return Err(denied("FILE_WRITE requires a filename")); }
    // The drive root is the sole namespace lookup. All subsequent lookups use
    // RootDirectory handles, never this path or a concatenated pathname again.
    let root = format!("\\??\\{}:\\", drive as char);
    let mut parents = vec![open_component(std::ptr::null_mut(), OsStr::new(&root), true)?];
    for name in &names[..names.len() - 1] {
        let child = open_component(parents.last().unwrap().as_raw_handle(), name, true)?;
        parents.push(child);
    }
    let file = open_component(parents.last().unwrap().as_raw_handle(), names[names.len() - 1], false)?;
    Ok(OpenedWrite { file, _parents: parents })
}

#[cfg(unix)]
fn open(path: &Path) -> io::Result<OpenedWrite> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::OpenOptionsExt;

    let absolute = absolute_path(path)?;
    let names: Vec<_> = absolute.components().filter_map(|c| match c {
        Component::Normal(name) => Some(name),
        _ => None,
    }).collect();
    if names.is_empty() { return Err(denied("FILE_WRITE requires a filename")); }
    let root = std::fs::OpenOptions::new().read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC).open("/")?;
    let mut parents = vec![root];
    for (index, name) in names.iter().enumerate() {
        let directory = index + 1 != names.len();
        let name = CString::new(name.as_bytes()).map_err(|_| denied("NUL in FILE_WRITE path"))?;
        let flags = libc::O_NOFOLLOW | libc::O_CLOEXEC | if directory {
            libc::O_RDONLY | libc::O_DIRECTORY
        } else {
            libc::O_WRONLY | libc::O_CREAT | libc::O_NONBLOCK
        };
        // SAFETY: live owned parent fd and NUL-terminated single component.
        // O_NOFOLLOW rejects links atomically; no truncation until fstat succeeds.
        let fd = unsafe { libc::openat(parents.last().unwrap().as_raw_fd(), name.as_ptr(), flags, 0o666 as libc::mode_t) };
        if fd < 0 { return Err(io::Error::last_os_error()); }
        let file = unsafe { File::from_raw_fd(fd) };
        let metadata = file.metadata()?;
        if directory {
            if !metadata.is_dir() { return Err(denied("FILE_WRITE parent is not a directory")); }
            parents.push(file);
        } else {
            if !metadata.is_file() { return Err(denied("FILE_WRITE target is not a regular file")); }
            return Ok(OpenedWrite { file, _parents: parents });
        }
    }
    Err(denied("FILE_WRITE requires a filename"))
}

#[cfg(not(any(windows, unix)))]
fn open(_path: &Path) -> io::Result<OpenedWrite> {
    Err(denied("Safe FILE_WRITE is unsupported on this platform"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox() -> PathBuf {
        let path = std::env::temp_dir().join(format!("traxes-atomic-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn create_and_overwrite_through_validated_handle() {
        let sandbox = sandbox();
        let target = sandbox.join("file.txt");
        write(&target, b"original longer bytes").unwrap();
        write(&target, b"short").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"short");
        std::fs::remove_dir_all(sandbox).unwrap();
    }

    #[test]
    fn unsupported_paths_fail_closed() {
        assert!(open(Path::new("")).is_err());
        assert!(open(Path::new("../escape.txt")).is_err());
        #[cfg(windows)]
        for path in [r"C:relative.txt", r"\rooted.txt", r"\\server\share\file.txt", r"\\?\C:\file.txt", r"C:\file.txt:stream"] {
            assert!(open(Path::new(path)).is_err(), "{path}");
        }
    }

    #[test]
    #[cfg(windows)]
    fn windows_parent_and_target_cannot_be_replaced_between_open_and_write() {
        let sandbox = sandbox();
        let parent = sandbox.join("parent");
        std::fs::create_dir(&parent).unwrap();
        let target = parent.join("file.txt");
        std::fs::write(&target, "before").unwrap();
        let opened = open(&target).unwrap();
        // Attempt the attacker operations at the exact old check/write boundary.
        // Both renames need DELETE access, excluded by our retained handles.
        std::thread::scope(|scope| {
            scope.spawn(|| {
                assert!(std::fs::rename(&target, parent.join("moved.txt")).is_err());
                assert!(std::fs::remove_file(&target).is_err());
                assert!(std::fs::rename(&parent, sandbox.join("moved-parent")).is_err());
            }).join().unwrap();
        });
        opened.write(b"after").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "after");
        // Prove the failed renames were due to retained handles, not ACLs.
        std::fs::rename(&parent, sandbox.join("moved-parent")).unwrap();
        std::fs::remove_dir_all(sandbox).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn windows_conflicting_mutation_handles_fail_closed() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE};
        let sandbox = sandbox();
        let parent = sandbox.join("parent");
        std::fs::create_dir(&parent).unwrap();
        let target = parent.join("file.txt");
        std::fs::write(&target, "untouched").unwrap();
        // An attacker with an already-open writable directory handle could set
        // a reparse point. Opening our chain must fail instead of trusting it.
        let mutation_handle = std::fs::OpenOptions::new().write(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(&parent).unwrap();
        assert!(write(&target, b"must not write").is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "untouched");
        drop(mutation_handle);
        // While the validated handles live, a new writable directory handle is
        // also refused, preventing reparse mutation after handle validation.
        let opened = open(&target).unwrap();
        assert!(std::fs::OpenOptions::new().write(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(&parent).is_err());
        drop(opened);
        std::fs::remove_dir_all(sandbox).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn unix_replacement_after_open_cannot_redirect_write() {
        let sandbox = sandbox();
        let parent = sandbox.join("parent");
        let outside = sandbox.join("outside");
        std::fs::create_dir(&parent).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let target = parent.join("file.txt");
        std::fs::write(&target, "before").unwrap();
        std::fs::write(outside.join("file.txt"), "untouched").unwrap();
        let opened = open(&target).unwrap();
        let moved = sandbox.join("moved");
        std::fs::rename(&parent, &moved).unwrap();
        std::os::unix::fs::symlink(&outside, &parent).unwrap();
        opened.write(b"after").unwrap();
        assert_eq!(std::fs::read_to_string(moved.join("file.txt")).unwrap(), "after");
        assert_eq!(std::fs::read_to_string(outside.join("file.txt")).unwrap(), "untouched");
        assert!(write(&target, b"must not follow").is_err());
        std::fs::remove_file(&parent).unwrap();
        std::fs::remove_dir_all(sandbox).unwrap();
    }
}

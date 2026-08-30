use std::io;
use std::path::{Path, PathBuf};

pub(crate) const RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT: usize = 32;
const RESOLVER_EXECUTION_PATH_BYTE_LIMIT: usize = 32 * 1024;

pub(crate) fn require_absolute(path: &Path, name: &str) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} is not absolute"),
        ));
    }
    Ok(())
}

pub(crate) fn require_canonical_bounded_path(path: &Path, name: &str) -> io::Result<()> {
    use std::path::Component;

    if path_encoding_length(path) > RESOLVER_EXECUTION_PATH_BYTE_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} exceeds its fixed encoding limit"),
        ));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(std::path::MAIN_SEPARATOR_STR),
            Component::Normal(component) => normalized.push(component),
            Component::CurDir | Component::ParentDir => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{name} is not lexically canonical"),
                ));
            }
        }
    }
    if normalized.as_os_str() != path.as_os_str() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} is not lexically canonical"),
        ));
    }
    Ok(())
}

pub(crate) fn require_regular_file(path: &Path, name: &str) -> io::Result<()> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot inspect {name} as a regular file: {error}"),
        )
    })?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} is not a regular file"),
        ));
    }
    Ok(())
}

fn path_encoding_length(path: &Path) -> usize {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().len()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().count().saturating_mul(2)
    }
    #[cfg(not(any(unix, windows)))]
    {
        path.as_os_str().to_string_lossy().len()
    }
}

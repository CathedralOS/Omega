/*
    Measures cheaper ways to open a source file on Windows than the one the
    standard library uses.

        cargo run --release -p omega-lexer --example open_breakdown -- [--passes N] <directory>...

    `read_breakdown` found that opening a small file is the largest part of
    reading it, about 13 microseconds, and that antivirus is not the reason.
    This tool asks how much of that is the route taken to the file, by opening
    the same files four ways:

      1. `File::open` with the full path. This is the Win32 route: the path is
         converted and normalized, then handed to the kernel, which walks it
         from the drive letter down.
      2. `NtOpenFile` with the full path. The same walk, without the Win32
         layer in front of it.
      3. `NtOpenFile` relative to one handle on the top of the tree. The
         kernel starts walking at that directory, not at the drive letter.
      4. `NtOpenFile` relative to a handle on the file's own directory. There
         is nothing left to walk; the name is looked up in one directory.

    Route 4 needs a handle on every directory. A loader that lists a tree has
    to open every directory anyway, so it can keep those handles; we open them
    outside the timing to stand for that, and report what opening them costs
    separately.

    :NativeCalls
    Routes 2 to 4 call `NtOpenFile` in ntdll directly, because the standard
    library has no way to name a starting directory. That means `unsafe` and
    hand-written declarations of three structures. They are checked the only
    way that matters here: before anything is timed, files opened by every
    route are read and compared byte for byte with `std::fs::read`.

    @Incomplete: warm file cache only, like the other tools here.
*/

#[cfg(not(windows))]
fn main() {
    eprintln!("open_breakdown measures Windows file opening and does nothing on other systems");
}

#[cfg(windows)]
#[path = "support/omega_files.rs"]
mod omega_files;

#[cfg(windows)]
fn main() {
    windows_measurement::run();
}

#[cfg(windows)]
mod windows_measurement {
    use super::omega_files;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions};
    use std::hint::black_box;
    use std::io::Read;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    const DEFAULT_PASS_COUNT: usize = 5;
    const READ_BUFFER_BYTE_COUNT: usize = 256 * 1024;
    const VERIFIED_FILE_COUNT: usize = 500;

    // :NativeCalls
    // Values from the Windows headers. `CreateFile` needs FILE_FLAG_BACKUP_SEMANTICS to open a directory.
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_READ_DATA: u32 = 0x0001;
    const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const FILE_SHARE_READ_WRITE_DELETE: u32 = 0x0007;
    const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x0000_0020;
    const FILE_NON_DIRECTORY_FILE: u32 = 0x0000_0040;
    const OBJ_CASE_INSENSITIVE: u32 = 0x0000_0040;

    /// UNICODE_STRING: a counted UTF-16 string. Both lengths are in bytes, not characters.
    #[repr(C)]
    struct UnicodeString {
        length_in_bytes: u16,
        maximum_length_in_bytes: u16,
        buffer: *const u16,
    }

    /// OBJECT_ATTRIBUTES: what to open, and which directory handle the name is relative to.
    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root_directory: RawHandle, // Null means the name is a full path.
        object_name: *const UnicodeString,
        attributes: u32,
        security_descriptor: *const c_void,
        security_quality_of_service: *const c_void,
    }

    /// IO_STATUS_BLOCK: the kernel writes the outcome here. We only need it to exist.
    #[repr(C)]
    struct IoStatusBlock {
        status_or_pointer: usize,
        information: usize,
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtOpenFile(
            file_handle: *mut RawHandle,
            desired_access: u32,
            object_attributes: *const ObjectAttributes,
            io_status_block: *mut IoStatusBlock,
            share_access: u32,
            open_options: u32,
        ) -> i32;
    }

    /// Opens a file for reading with `NtOpenFile`. `name` is a full NT path when `root_directory`
    /// is null, and a path relative to that directory otherwise.
    fn open_native(root_directory: RawHandle, name: &[u16]) -> Option<File> {
        // A UNICODE_STRING counts bytes in 16 bits, so a name of 32,768 characters or more cannot be passed.
        let name_byte_count = u16::try_from(name.len().checked_mul(2)?).ok()?;
        let object_name = UnicodeString {
            length_in_bytes: name_byte_count,
            maximum_length_in_bytes: name_byte_count,
            buffer: name.as_ptr(),
        };
        let object_attributes = ObjectAttributes {
            length: size_of::<ObjectAttributes>() as u32,
            root_directory,
            object_name: &object_name,
            attributes: OBJ_CASE_INSENSITIVE,
            security_descriptor: std::ptr::null(),
            security_quality_of_service: std::ptr::null(),
        };
        let mut io_status_block = IoStatusBlock {
            status_or_pointer: 0,
            information: 0,
        };
        let mut file_handle: RawHandle = std::ptr::null_mut();

        // SAFETY: every pointer refers to a local that outlives the call. `object_name.buffer`
        // points at `name`, which the caller keeps alive, and its length is the slice's own
        // length in bytes. `object_attributes.length` is the structure's real size. The call
        // writes only to `file_handle` and `io_status_block`, both valid and exclusively ours.
        let status = unsafe {
            NtOpenFile(
                &mut file_handle,
                FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
                &object_attributes,
                &mut io_status_block,
                FILE_SHARE_READ_WRITE_DELETE,
                FILE_SYNCHRONOUS_IO_NONALERT | FILE_NON_DIRECTORY_FILE,
            )
        };

        // An NTSTATUS below zero is a failure, and then `file_handle` was not written.
        if status < 0 || file_handle.is_null() {
            return None;
        }
        // SAFETY: the call succeeded, so `file_handle` is an open handle that nothing else owns.
        // It was opened for synchronous I/O, which is what `File`'s reads expect, and `File`
        // closes it when dropped.
        Some(unsafe { File::from_raw_handle(file_handle) })
    }

    fn open_directory(directory: &Path) -> Option<File> {
        OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(directory)
            .ok()
    }

    fn encode_utf16(text: &std::ffi::OsStr) -> Vec<u16> {
        text.encode_wide().collect()
    }

    /// One source file, with its name prepared for each of the four routes.
    struct PreparedFile {
        full_path: PathBuf,
        full_nt_name: Vec<u16>, // `\??\C:\...`, for a native open with no starting directory.
        tree_root_index: usize, // Which search directory this file was found under.
        tree_relative_name: Vec<u16>, // Path below that search directory.
        parent_index: usize,    // Which entry of the parent directory list holds this file.
        file_name: Vec<u16>,    // The last path component alone.
        byte_count: usize,
    }

    pub fn run() {
        let mut pass_count = DEFAULT_PASS_COUNT;
        let mut search_directories = Vec::new();
        let mut arguments = std::env::args_os().skip(1);
        while let Some(argument) = arguments.next() {
            if argument == "--passes" {
                let requested = arguments
                    .next()
                    .and_then(|value| value.to_str()?.parse().ok());
                let Some(requested) = requested.filter(|&count: &usize| count > 0) else {
                    eprintln!("--passes needs a positive number");
                    std::process::exit(2);
                };
                pass_count = requested;
            } else {
                // Relative opens need names without `.` or `..`, so start from an absolute path.
                match std::path::absolute(PathBuf::from(&argument)) {
                    Ok(absolute_directory) => search_directories.push(absolute_directory),
                    Err(_) => {
                        eprintln!("cannot make {} absolute", PathBuf::from(argument).display());
                        std::process::exit(2);
                    }
                }
            }
        }
        if search_directories.is_empty() {
            eprintln!("usage: open_breakdown [--passes N] <directory>...");
            std::process::exit(2);
        }

        let mut parent_directories: Vec<PathBuf> = Vec::new();
        let mut parent_indices: HashMap<PathBuf, usize> = HashMap::new();
        let mut prepared_files = Vec::new();
        for (tree_root_index, search_directory) in search_directories.iter().enumerate() {
            let mut source_paths = Vec::new();
            omega_files::collect_omega_files(search_directory, &mut source_paths);
            source_paths.sort();
            for full_path in source_paths {
                let (Some(parent), Some(file_name), Ok(tree_relative_path)) = (
                    full_path.parent(),
                    full_path.file_name(),
                    full_path.strip_prefix(search_directory),
                ) else {
                    continue;
                };
                let parent_index =
                    *parent_indices
                        .entry(parent.to_path_buf())
                        .or_insert_with(|| {
                            parent_directories.push(parent.to_path_buf());
                            parent_directories.len() - 1
                        });
                let mut full_nt_name = encode_utf16(std::ffi::OsStr::new(r"\??\"));
                full_nt_name.extend(full_path.as_os_str().encode_wide());
                prepared_files.push(PreparedFile {
                    full_nt_name,
                    tree_root_index,
                    tree_relative_name: encode_utf16(tree_relative_path.as_os_str()),
                    parent_index,
                    file_name: encode_utf16(file_name),
                    byte_count: std::fs::metadata(&full_path)
                        .map_or(0, |metadata| metadata.len() as usize),
                    full_path,
                });
            }
        }
        if prepared_files.is_empty() {
            eprintln!("no .omg source found");
            std::process::exit(1);
        }

        let tree_root_handles: Vec<File> = search_directories
            .iter()
            .filter_map(|directory| open_directory(directory))
            .collect();
        if tree_root_handles.len() != search_directories.len() {
            eprintln!("could not open every search directory");
            std::process::exit(1);
        }

        let directory_open_seconds = time_passes(pass_count, || {
            for parent_directory in &parent_directories {
                black_box(open_directory(parent_directory).is_some());
            }
        });
        let parent_handles: Vec<File> = parent_directories
            .iter()
            .filter_map(|directory| open_directory(directory))
            .collect();
        if parent_handles.len() != parent_directories.len() {
            eprintln!("could not open every parent directory");
            std::process::exit(1);
        }

        let file_count = prepared_files.len();
        println!("files:        {file_count}");
        println!(
            "directories:  {} ({:.1} files per directory)",
            parent_directories.len(),
            file_count as f64 / parent_directories.len() as f64
        );
        println!("passes:       {pass_count} (the fastest pass is shown)");

        // The four routes, as functions from a prepared file to an open handle.
        let open_standard = |prepared: &PreparedFile| File::open(&prepared.full_path).ok();
        let open_native_full =
            |prepared: &PreparedFile| open_native(std::ptr::null_mut(), &prepared.full_nt_name);
        let open_from_tree_root = |prepared: &PreparedFile| {
            open_native(
                tree_root_handles[prepared.tree_root_index].as_raw_handle(),
                &prepared.tree_relative_name,
            )
        };
        let open_from_parent = |prepared: &PreparedFile| {
            open_native(
                parent_handles[prepared.parent_index].as_raw_handle(),
                &prepared.file_name,
            )
        };

        // :NativeCalls
        // Nothing is timed until every route has been shown to deliver the right bytes.
        let mut read_buffer = vec![0u8; READ_BUFFER_BYTE_COUNT];
        let verification_stride = (file_count / VERIFIED_FILE_COUNT).max(1);
        let mut verified_file_count = 0usize;
        for prepared in prepared_files.iter().step_by(verification_stride) {
            let Ok(expected_bytes) = std::fs::read(&prepared.full_path) else {
                continue;
            };
            let routes: [(&str, Option<File>); 3] = [
                ("native, full path", open_native_full(prepared)),
                ("native, from tree root", open_from_tree_root(prepared)),
                ("native, from parent directory", open_from_parent(prepared)),
            ];
            for (route_name, opened) in routes {
                let Some(mut file) = opened else {
                    eprintln!(
                        "{route_name} failed to open {}",
                        prepared.full_path.display()
                    );
                    std::process::exit(1);
                };
                let read_byte_count =
                    read_exactly(&mut file, prepared.byte_count, &mut read_buffer);
                if read_buffer[..read_byte_count] != expected_bytes[..] {
                    eprintln!(
                        "{route_name} read different bytes from {}",
                        prepared.full_path.display()
                    );
                    std::process::exit(1);
                }
            }
            verified_file_count += 1;
        }
        println!("verified:     {verified_file_count} files read identically by every route");

        println!(
            "\nopening every directory once: {:.1} ms, {:.1} us per directory",
            directory_open_seconds * 1e3,
            directory_open_seconds * 1e6 / parent_directories.len() as f64
        );

        println!("\none thread                              open + close   open + read + close");
        println!("route                                     per file          per file   files/s");
        let report = |route_name: &str, open_seconds: f64, read_seconds: f64| {
            println!(
                "{route_name:<38} {:>9.1} us  {:>12.1} us  {:>8.0}",
                open_seconds * 1e6 / file_count as f64,
                read_seconds * 1e6 / file_count as f64,
                file_count as f64 / read_seconds
            );
        };

        macro_rules! measure_route {
            ($route_name:expr, $open:expr) => {{
                let open_seconds = time_passes(pass_count, || {
                    for prepared in &prepared_files {
                        black_box($open(prepared).is_some());
                    }
                });
                let read_seconds = time_passes(pass_count, || {
                    for prepared in &prepared_files {
                        if let Some(mut file) = $open(prepared) {
                            black_box(read_exactly(
                                &mut file,
                                prepared.byte_count,
                                &mut read_buffer,
                            ));
                        }
                    }
                });
                report($route_name, open_seconds, read_seconds);
            }};
        }
        measure_route!("1. File::open, full path", open_standard);
        measure_route!("2. NtOpenFile, full path", open_native_full);
        measure_route!("3. NtOpenFile, from the tree root", open_from_tree_root);
        measure_route!("4. NtOpenFile, from its own directory", open_from_parent);

        println!("\nopen + read + close, files per second by thread count");
        println!("threads   1. File::open   4. from its own directory");
        let hardware_thread_count =
            std::thread::available_parallelism().map_or(1, |count| count.get());
        let mut thread_count = 1;
        loop {
            let standard_seconds = time_passes(pass_count, || {
                run_on_threads(thread_count, file_count, |read_buffer, file_index| {
                    let prepared = &prepared_files[file_index];
                    if let Some(mut file) = open_standard(prepared) {
                        black_box(read_exactly(&mut file, prepared.byte_count, read_buffer));
                    }
                });
            });
            let parent_seconds = time_passes(pass_count, || {
                run_on_threads(thread_count, file_count, |read_buffer, file_index| {
                    let prepared = &prepared_files[file_index];
                    if let Some(mut file) = open_from_parent(prepared) {
                        black_box(read_exactly(&mut file, prepared.byte_count, read_buffer));
                    }
                });
            });
            println!(
                "{thread_count:>7}   {:>13.0}   {:>25.0}",
                file_count as f64 / standard_seconds,
                file_count as f64 / parent_seconds
            );
            if thread_count >= hardware_thread_count {
                break;
            }
            thread_count = (thread_count * 2).min(hardware_thread_count);
        }
    }

    /// Reads exactly `wanted_byte_count` bytes, or fewer if the file ends or the buffer is smaller.
    fn read_exactly(file: &mut File, wanted_byte_count: usize, read_buffer: &mut [u8]) -> usize {
        let wanted_byte_count = wanted_byte_count.min(read_buffer.len());
        let mut filled_byte_count = 0;
        while filled_byte_count < wanted_byte_count {
            match file.read(&mut read_buffer[filled_byte_count..wanted_byte_count]) {
                Ok(0) | Err(_) => break,
                Ok(read_byte_count) => filled_byte_count += read_byte_count,
            }
        }
        filled_byte_count
    }

    fn time_passes(pass_count: usize, mut measured_work: impl FnMut()) -> f64 {
        let mut fastest_seconds = f64::INFINITY;
        for _ in 0..pass_count {
            let started = Instant::now();
            measured_work();
            fastest_seconds = fastest_seconds.min(started.elapsed().as_secs_f64());
        }
        fastest_seconds
    }

    /// Calls `handle_item` for every index below `item_count` on `thread_count` threads, each
    /// with its own read buffer. Items are handed out one at a time from a shared counter.
    fn run_on_threads(
        thread_count: usize,
        item_count: usize,
        handle_item: impl Fn(&mut [u8], usize) + Sync,
    ) {
        if thread_count <= 1 {
            let mut read_buffer = vec![0u8; READ_BUFFER_BYTE_COUNT];
            for item_index in 0..item_count {
                handle_item(&mut read_buffer, item_index);
            }
            return;
        }

        let next_item_index = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..thread_count {
                scope.spawn(|| {
                    let mut read_buffer = vec![0u8; READ_BUFFER_BYTE_COUNT];
                    loop {
                        let item_index = next_item_index.fetch_add(1, Ordering::Relaxed);
                        if item_index >= item_count {
                            break;
                        }
                        handle_item(&mut read_buffer, item_index);
                    }
                });
            }
        });
    }
}

/*
    Breaks down what it costs to get one small source file's bytes into
    memory, and how that cost behaves as threads are added.

        cargo run --release -p omega-lexer --example read_breakdown -- [--passes N] <directory>...

    `lex_pipeline` showed that on a tree of small files the path from disk to
    tokens waits on reading, not on lexing, and that reading is limited by how
    many files can be opened per second, not by how many bytes move. This tool
    asks where the time for one file goes. Reading a file is four separate
    requests to the operating system: open it, ask how big it is, read it,
    close it. We time each one by itself, then compare whole strategies for
    reading a file, then run the best strategies on 1 to 32 threads.

    The strategies:

      - `std::fs::read`, what most Rust code does: all four requests, and a
        fresh buffer for every file.
      - Reuse one buffer: the same four requests, no allocation per file.
      - Reuse one buffer and skip the size request: read until the file says
        it is finished. Three requests for a file that fits the buffer.

    @Incomplete: every read here is served from the operating system's file
    cache, since the files were just read. This measures the cost of asking
    for a file, which is the part that does not go away on a fast drive. It
    says nothing about the first read after a restart.

    This is not part of the lexer. It sits here because the lexer's measuring
    tools are here and the step it measures has no layer of its own yet.
*/

use measure::{ToolArguments, collect_source_files};
use std::fs::File;
use std::hint::black_box;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const DEFAULT_PASS_COUNT: usize = 5;

/// Large enough that every file in an ordinary source tree is read by one request.
const REUSED_BUFFER_BYTE_COUNT: usize = 256 * 1024;

fn main() {
    let tool_arguments = ToolArguments::from_command_line("read_breakdown", DEFAULT_PASS_COUNT);
    let pass_count = tool_arguments.get_pass_count();
    let search_directories = tool_arguments.get_search_directories();

    let mut source_paths = Vec::new();

    for search_directory in search_directories {
        collect_source_files(search_directory, "omg", &mut source_paths);
    }

    source_paths.sort();

    if source_paths.is_empty() {
        eprintln!("no .omg source found");
        std::process::exit(1);
    }

    // One untimed trip so that everything measured afterwards comes from a warm cache.
    let total_byte_count: usize = source_paths
        .iter()
        .map(|source_path| std::fs::read(source_path).map_or(0, |bytes| bytes.len()))
        .sum();
    let file_count = source_paths.len();
    println!("files:  {file_count}");
    println!("bytes:  {total_byte_count}");
    println!("passes: {pass_count} (the fastest pass is shown)");

    print_step_breakdown(&source_paths, pass_count);
    print_strategy_comparison(&source_paths, pass_count, total_byte_count);
    print_thread_scaling(&source_paths, pass_count);
}

/// Times each of the four requests by itself, summed over every file on one thread.
fn print_step_breakdown(
    source_paths: &[PathBuf],
    pass_count: usize,
) {
    let mut fastest = [Duration::MAX; 4];
    let mut read_buffer = vec![0u8; REUSED_BUFFER_BYTE_COUNT];

    for _ in 0..pass_count {
        let mut step_totals = [Duration::ZERO; 4];

        for source_path in source_paths {
            let start_time = Instant::now();
            let Ok(mut file) = File::open(source_path) else {
                continue;
            };
            step_totals[0] += start_time.elapsed();

            let start_time = Instant::now();
            black_box(file.metadata().map_or(0, |metadata| metadata.len()));
            step_totals[1] += start_time.elapsed();

            let start_time = Instant::now();
            black_box(read_until_finished(&mut file, &mut read_buffer));
            step_totals[2] += start_time.elapsed();

            let start_time = Instant::now();
            drop(file);
            step_totals[3] += start_time.elapsed();
        }

        for (fastest_step, step_total) in fastest.iter_mut().zip(step_totals) {
            *fastest_step = (*fastest_step).min(step_total);
        }
    }

    let whole: Duration = fastest.iter().sum();
    println!("\none file, one thread, each request timed by itself");
    println!("request            per file    share");

    for (step_name, step_total) in ["open", "ask the size", "read", "close"].iter().zip(fastest) {
        println!(
            "{step_name:<16} {:>7.1} us   {:>5.1}%",
            step_total.as_secs_f64() * 1e6 / source_paths.len() as f64,
            step_total.as_secs_f64() * 100.0 / whole.as_secs_f64()
        );
    }

    println!("{:<16} {:>7.1} us", "all four", whole.as_secs_f64() * 1e6 / source_paths.len() as f64);
}

/// Compares whole ways of reading a file, on one thread.
fn print_strategy_comparison(
    source_paths: &[PathBuf],
    pass_count: usize,
    total_byte_count: usize,
) {
    println!("\nways to read a file, one thread");
    println!("strategy                               time     files/s     MB/s");

    let megabytes = total_byte_count as f64 / 1_000_000.0;
    let file_count = source_paths.len() as f64;
    let report = |strategy: &str, seconds: f64| {
        println!(
            "{strategy:<34} {:>7.1} ms  {:>9.0}  {:>7.0}",
            seconds * 1e3,
            file_count / seconds,
            megabytes / seconds
        );
    };

    report(
        "std::fs::read",
        time_passes(pass_count, || {
            for source_path in source_paths {
                black_box(std::fs::read(source_path).map_or(0, |bytes| bytes.len()));
            }
        }),
    );

    let mut read_buffer = vec![0u8; REUSED_BUFFER_BYTE_COUNT];
    report(
        "reuse one buffer",
        time_passes(pass_count, || {
            for source_path in source_paths {
                black_box(read_with_size_request(source_path, &mut read_buffer));
            }
        }),
    );
    report(
        "reuse one buffer, skip the size",
        time_passes(pass_count, || {
            for source_path in source_paths {
                black_box(read_without_size_request(source_path, &mut read_buffer));
            }
        }),
    );

    // A directory listing reports each file's size for free, so a loader that walks the tree
    // already knows how many bytes to ask for and needs no second read to learn the file ended.
    // The sizes are gathered here outside the timing, to stand in for that listing.
    let file_byte_counts: Vec<usize> = source_paths
        .iter()
        .map(|source_path| std::fs::metadata(source_path).map_or(0, |metadata| metadata.len() as usize))
        .collect();
    report(
        "size from the listing, one read",
        time_passes(pass_count, || {
            for (source_path, &file_byte_count) in source_paths.iter().zip(&file_byte_counts) {
                black_box(read_known_size(source_path, file_byte_count, &mut read_buffer));
            }
        }),
    );

    print_packed_file_comparison(source_paths, pass_count, &report);
}

/// Measures the alternative to opening thousands of files: reading them all as one.
///
/// We write every source into a single temporary file, time reading that, and delete it. This
/// is a measurement of what there is to gain, not a design: a real packed cache also has to
/// notice when a source file has changed, and that check is not free.
fn print_packed_file_comparison(
    source_paths: &[PathBuf],
    pass_count: usize,
    report: &impl Fn(&str, f64),
) {
    let pack_path = std::env::temp_dir().join(format!("omega-v2-read-breakdown-{}.pack", std::process::id()));
    let mut packed_bytes = Vec::new();

    for source_path in source_paths {
        if let Ok(source_bytes) = std::fs::read(source_path) {
            packed_bytes.extend_from_slice(&source_bytes);
        }
    }

    if std::fs::write(&pack_path, &packed_bytes).is_err() {
        println!("(could not write a temporary packed file; skipping that comparison)");
        return;
    }

    drop(packed_bytes);

    report(
        "all files packed into one",
        time_passes(pass_count, || {
            black_box(std::fs::read(&pack_path).map_or(0, |bytes| bytes.len()));
        }),
    );
    let mut pack_buffer = Vec::new();
    report(
        "packed, into a reused buffer",
        time_passes(pass_count, || {
            pack_buffer.clear();

            if let Ok(mut pack_file) = File::open(&pack_path) {
                let _ = pack_file.read_to_end(&mut pack_buffer);
            }

            black_box(pack_buffer.len());
        }),
    );

    let _ = std::fs::remove_file(&pack_path);
}

/// Opens the file and reads exactly `file_byte_count` bytes, with no request for the size and
/// no extra read to find the end. Returns the bytes read.
fn read_known_size(
    source_path: &Path,
    file_byte_count: usize,
    read_buffer: &mut [u8],
) -> usize {
    let Ok(mut file) = File::open(source_path) else {
        return 0;
    };
    let wanted_byte_count = file_byte_count.min(read_buffer.len());
    let mut filled_byte_count = 0;

    while filled_byte_count < wanted_byte_count {
        match file.read(&mut read_buffer[filled_byte_count..wanted_byte_count]) {
            Ok(0) | Err(_) => break,
            Ok(read_byte_count) => filled_byte_count += read_byte_count,
        }
    }

    filled_byte_count
}

/// Runs the two extreme strategies on a growing number of threads.
fn print_thread_scaling(
    source_paths: &[PathBuf],
    pass_count: usize,
) {
    let hardware_thread_count = std::thread::available_parallelism().map_or(1, |count| count.get());
    println!("\nfiles per second by thread count");
    println!("threads   std::fs::read   reuse buffer, skip size");

    let mut thread_count = 1;

    loop {
        let standard_seconds = time_passes(pass_count, || {
            run_on_threads(
                thread_count,
                source_paths.len(),
                || (),
                |(), file_index| {
                    black_box(std::fs::read(&source_paths[file_index]).map_or(0, |bytes| bytes.len()));
                },
            );
        });
        let reuse_seconds = time_passes(pass_count, || {
            run_on_threads(
                thread_count,
                source_paths.len(),
                || vec![0u8; REUSED_BUFFER_BYTE_COUNT],
                |read_buffer, file_index| {
                    black_box(read_without_size_request(&source_paths[file_index], read_buffer));
                },
            );
        });
        println!(
            "{thread_count:>7}   {:>13.0}   {:>23.0}",
            source_paths.len() as f64 / standard_seconds,
            source_paths.len() as f64 / reuse_seconds
        );

        if thread_count >= hardware_thread_count {
            break;
        }

        thread_count = (thread_count * 2).min(hardware_thread_count);
    }
}

/// Opens the file, asks its size, then reads it into `read_buffer`. Returns the bytes read.
fn read_with_size_request(
    source_path: &Path,
    read_buffer: &mut [u8],
) -> usize {
    let Ok(mut file) = File::open(source_path) else {
        return 0;
    };
    black_box(file.metadata().map_or(0, |metadata| metadata.len()));

    read_until_finished(&mut file, read_buffer)
}

/// Opens the file and reads it into `read_buffer` without asking its size first.
fn read_without_size_request(
    source_path: &Path,
    read_buffer: &mut [u8],
) -> usize {
    let Ok(mut file) = File::open(source_path) else {
        return 0;
    };

    read_until_finished(&mut file, read_buffer)
}

/// Reads until the file reports its end or `read_buffer` is full. Returns the bytes read.
///
/// A read that returns fewer bytes than asked for has not necessarily reached the end, so the
/// only reliable signal is a read that returns nothing. For a file that fits the buffer that is
/// two requests: one that returns the contents and one that returns zero.
fn read_until_finished(
    file: &mut File,
    read_buffer: &mut [u8],
) -> usize {
    let mut filled_byte_count = 0;

    while filled_byte_count < read_buffer.len() {
        match file.read(&mut read_buffer[filled_byte_count..]) {
            Ok(0) | Err(_) => break,
            Ok(read_byte_count) => filled_byte_count += read_byte_count,
        }
    }

    filled_byte_count
}

/// Runs `measured_work` once per pass and returns the fastest pass in seconds.
fn time_passes(
    pass_count: usize,
    mut measured_work: impl FnMut(),
) -> f64 {
    let mut fastest_seconds = f64::INFINITY;

    for _ in 0..pass_count {
        let start_time = Instant::now();
        measured_work();
        fastest_seconds = fastest_seconds.min(start_time.elapsed().as_secs_f64());
    }

    fastest_seconds
}

/// Calls `handle_item` once for every index below `item_count`, spread over `thread_count` threads,
/// each with its own state. Items are handed out one at a time from a shared counter.
fn run_on_threads<ThreadState>(
    thread_count: usize,
    item_count: usize,
    make_thread_state: impl Fn() -> ThreadState + Sync,
    handle_item: impl Fn(&mut ThreadState, usize) + Sync,
) {
    if thread_count <= 1 {
        let mut thread_state = make_thread_state();

        for item_index in 0..item_count {
            handle_item(&mut thread_state, item_index);
        }

        return;
    }

    let next_item_index = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..thread_count {
            scope.spawn(|| {
                let mut thread_state = make_thread_state();

                loop {
                    let item_index = next_item_index.fetch_add(1, Ordering::Relaxed);

                    if item_index >= item_count {
                        break;
                    }

                    handle_item(&mut thread_state, item_index);
                }
            });
        }
    });
}

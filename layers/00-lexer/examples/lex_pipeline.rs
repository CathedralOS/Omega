/*
    Measures the whole path from files on disk to tokens, on one core and on
    all of them, to show which step limits it.

        cargo run --release -p omega-lexer --example lex_pipeline -- [--passes N] <directory>...

    `lex_throughput` answers "how fast is the lexer on bytes already in
    memory". This answers a different question: when a compiler is pointed at
    a tree of source files, is the lexer what it waits for? So we time each
    step separately and then together:

      - finding the files;
      - reading them, which for many small files is mostly the cost of
        opening each one, not of moving bytes off the drive;
      - lexing them from memory, once with a fresh token list per file and
        once reusing one list per thread;
      - reading and lexing each file on the same thread, which is what a
        compiler would actually do.

    Every step after finding the files is run on one thread and then on every
    hardware thread. Files are handed out one at a time from a shared counter,
    so a thread that draws large files does not leave others idle.

    @Incomplete: the reads here come from the operating system's file cache,
    because the files were read moments earlier. That is the best case for
    reading. A cold read straight from the drive can only be slower, so if
    reading already limits the path here, it limits it more when cold. We do
    not measure the cold case; clearing the file cache needs administrator
    rights and disturbs the rest of the machine.
*/

#[path = "support/omega_files.rs"]
mod omega_files;

use omega_lexer::{Token, lex, lex_into};
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const DEFAULT_PASS_COUNT: usize = 5;

/// How many times the in-memory lexing steps go over the files in one pass.
///
/// On every hardware thread the whole corpus lexes in a few milliseconds, which is no longer than
/// starting the threads takes. Going over it several times makes the lexing long enough to measure;
/// the reported time is divided back down to one trip over the files.
const LEX_REPEAT_COUNT: usize = 20;

fn main() {
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
            search_directories.push(PathBuf::from(argument));
        }
    }
    if search_directories.is_empty() {
        eprintln!("usage: lex_pipeline [--passes N] <directory>...");
        std::process::exit(2);
    }

    let hardware_thread_count = std::thread::available_parallelism().map_or(1, |count| count.get());

    let mut source_paths = Vec::new();
    let find_seconds = time_passes(pass_count, || {
        source_paths.clear();
        for search_directory in &search_directories {
            omega_files::collect_omega_files(search_directory, &mut source_paths);
        }
    });
    source_paths.sort();

    // Read once up front so every later measurement sees the same warm file cache, and so the
    // lex-only measurements have the bytes in memory.
    let sources: Vec<Vec<u8>> = source_paths
        .iter()
        .map(|source_path| std::fs::read(source_path).unwrap_or_default())
        .collect();
    let total_byte_count: usize = sources.iter().map(Vec::len).sum();
    if total_byte_count == 0 {
        eprintln!("no .omg source found");
        std::process::exit(1);
    }

    println!("files:             {}", source_paths.len());
    println!("bytes:             {total_byte_count}");
    println!("hardware threads:  {hardware_thread_count}");
    println!("passes:            {pass_count} (the fastest pass is shown)");
    println!();
    println!("step                             threads       time      MB/s    files/s");

    let megabytes = total_byte_count as f64 / 1_000_000.0;
    let file_count = source_paths.len() as f64;
    let report = |step: &str, thread_count: usize, seconds: f64| {
        println!(
            "{step:<32} {thread_count:>7}  {:>8.1} ms  {:>7.0}  {:>9.0}",
            seconds * 1e3,
            megabytes / seconds,
            file_count / seconds
        );
    };
    let thread_counts = [1, hardware_thread_count];

    println!(
        "{:<32} {:>7}  {:>8.1} ms  {:>7}  {:>9.0}",
        "find the files",
        1,
        find_seconds * 1e3,
        "-",
        file_count / find_seconds
    );

    for thread_count in thread_counts {
        let seconds = time_passes(pass_count, || {
            run_on_threads(
                thread_count,
                source_paths.len(),
                || (),
                |(), file_index| {
                    black_box(
                        std::fs::read(&source_paths[file_index])
                            .map(|bytes| bytes.len())
                            .unwrap_or(0),
                    );
                },
            );
        });
        report("read (warm file cache)", thread_count, seconds);
    }

    for thread_count in thread_counts {
        let seconds = time_passes(pass_count, || {
            run_on_threads(
                thread_count,
                sources.len() * LEX_REPEAT_COUNT,
                || (),
                |(), item_index| {
                    let file_index = item_index % sources.len();
                    black_box(
                        lex(black_box(&sources[file_index]))
                            .map(|tokens| tokens.len())
                            .unwrap_or(0),
                    );
                },
            );
        });
        report(
            "lex, new token list per file",
            thread_count,
            seconds / LEX_REPEAT_COUNT as f64,
        );
    }

    for thread_count in thread_counts {
        let seconds = time_passes(pass_count, || {
            run_on_threads(
                thread_count,
                sources.len() * LEX_REPEAT_COUNT,
                Vec::<Token>::new,
                |tokens, item_index| {
                    let file_index = item_index % sources.len();
                    let _ = lex_into(black_box(&sources[file_index]), tokens);
                    black_box(tokens.len());
                },
            );
        });
        report(
            "lex, one token list per thread",
            thread_count,
            seconds / LEX_REPEAT_COUNT as f64,
        );
    }

    for thread_count in thread_counts {
        let seconds = time_passes(pass_count, || {
            run_on_threads(
                thread_count,
                source_paths.len(),
                Vec::<Token>::new,
                |tokens, file_index| {
                    if let Ok(source_bytes) = std::fs::read(&source_paths[file_index]) {
                        let _ = lex_into(&source_bytes, tokens);
                        black_box(tokens.len());
                    }
                },
            );
        });
        report("read then lex, per file", thread_count, seconds);
    }
}

/// Runs `measured_work` once per pass and returns the fastest pass in seconds.
fn time_passes(pass_count: usize, mut measured_work: impl FnMut()) -> f64 {
    let mut fastest_seconds = f64::INFINITY;
    for _ in 0..pass_count {
        let started = Instant::now();
        measured_work();
        fastest_seconds = fastest_seconds.min(started.elapsed().as_secs_f64());
    }
    fastest_seconds
}

/// Calls `handle_item` once for every index below `item_count`, spread over `thread_count` threads.
/// Each thread gets its own state from `make_thread_state` and keeps it for all its items.
///
/// Threads take the next index from a shared counter. Handing out one item at a time costs an
/// atomic increment per file, which is nothing beside opening or lexing one, and it keeps the
/// threads evenly loaded however uneven the files are.
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

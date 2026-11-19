//! Measures how fast the parser turns tokens into a tree, next to how fast the lexer made the tokens.
//!
//!     cargo run --release -p omega-parser --example parse_throughput
//!     cargo run --release -p omega-parser --example parse_throughput -- --passes 15
//!     cargo run --release -p omega-parser --example parse_throughput -- --individual
//!
//! There is no large body of V2 source yet: V1's source tree is written in V1's syntax, which this
//! grammar refuses on purpose. So the workload is this crate's own accept fixtures, laid end to
//! end and repeated until there are about 64 MB of them. Every construct in the grammar is in
//! there, in proportions nobody chose, so read the number as "the parser on dense, varied source",
//! not as a prediction for a real program.
//!
//! The last line printed is for comparing two builds of the parser:
//!
//!     RESULT parse_min_ms=154.0 parse_median_ms=158.2 floor_min_ms=41.0 lex_min_ms=136.0 fingerprint=9f3a...
//!
//! :TwoWorkloads
//! By default the bytes are one 64 MB file. With `--files` they are the same bytes as 5,000-odd
//! files of 12 KB, lexed and parsed one after another. The second is what a compiler does, and
//! the two behave differently: a 64 MB file has 870 MB of tokens, so every pass over them waits
//! on memory, while a 12 KB file's tokens stay in the cache. Tune against `--files`; keep the
//! single file as a check that nothing depends on the input being small.
//! `--individual` keeps the original file boundaries and repeats the sorted fixtures.
//!
//! Two builds that print the same fingerprint built the same tree, event for event. The floor is
//! a loop that does the least any parser over these tokens could do: skip whitespace and comments
//! and write one event per token that is left. It has no grammar in it, so the gap between the
//! floor and the parse time is what the grammar costs.

use measure::{Fingerprint, ResultLine};
use omega_lexer::{Token, lex_into};
use omega_parser::{SyntaxTree, TreeEvent, parse_file};
use std::path::Path;
use std::time::{Duration, Instant};

const TARGET_BYTE_COUNT: usize = 64 * 1024 * 1024;
const DEFAULT_PASS_COUNT: usize = 9;

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let pass_count = arguments
        .iter()
        .position(|argument| argument == "--passes")
        .and_then(|flag_position| arguments.get(flag_position + 1))
        .and_then(|pass_count_text| pass_count_text.parse().ok())
        .unwrap_or(DEFAULT_PASS_COUNT);

    // `--files`: the same bytes as many small files instead of one large one. See :TwoWorkloads.
    let is_many_files = arguments.iter().any(|argument| argument == "--files");
    // `--only TEXT`: just the fixtures whose file name contains TEXT, to see what one kind of source costs.
    let only_filter = arguments
        .iter()
        .position(|argument| argument == "--only")
        .and_then(|flag_position| arguments.get(flag_position + 1))
        .cloned();
    let fixtures = read_fixtures(only_filter.as_deref());
    let fixture_count = fixtures.len();
    let fixture_bytes: usize = fixtures.iter().map(Vec::len).sum();
    let copy_count = TARGET_BYTE_COUNT.div_ceil(fixture_bytes);
    let (sources, repetitions) = if arguments.iter().any(|argument| argument == "--individual") {
        (fixtures, copy_count)
    } else if is_many_files {
        (vec![fixtures.concat()], copy_count)
    } else {
        (vec![fixtures.concat().repeat(copy_count)], 1)
    };
    let parse_count = sources.len() * repetitions;
    let total_bytes = fixture_bytes * copy_count;
    let megabyte_count = total_bytes as f64 / (1024.0 * 1024.0);

    // `--threads`: how parsing scales when every hardware thread parses its own files.
    if arguments.iter().any(|argument| argument == "--threads") {
        run_thread_scaling(&sources, repetitions, megabyte_count);

        return;
    }

    let mut tokens = Vec::new();
    let mut lex_durations = Vec::new();
    let mut parse_durations = Vec::new();
    let mut floor_durations = Vec::new();
    let mut event_count = 0usize;
    let mut meaningful_token_count = 0usize;
    let mut tree_fingerprints = Vec::new();

    for pass in 0..pass_count {
        let mut lex_duration = Duration::ZERO;
        let mut parse_duration = Duration::ZERO;
        let mut floor_duration = Duration::ZERO;

        for (file_number, source_bytes) in sources.iter().cycle().take(parse_count).enumerate() {
            let lex_started_at = Instant::now();

            if let Err(lex_error) = lex_into(source_bytes, &mut tokens) {
                eprintln!("the fixtures do not lex: {lex_error}");
                std::process::exit(1);
            }

            lex_duration += lex_started_at.elapsed();

            let parse_started_at = Instant::now();
            let parse_result = parse_file(source_bytes, &tokens);

            parse_duration += parse_started_at.elapsed();

            match parse_result {
                Ok(syntax_tree) => {
                    // Each distinct source only needs to be counted and hashed on its first visit.
                    if pass == 0 && file_number < sources.len() {
                        event_count += syntax_tree.get_events().len() * repetitions;
                        meaningful_token_count += syntax_tree.get_meaningful_token_indices().len() * repetitions;
                        tree_fingerprints.push(get_fingerprint(&syntax_tree));
                    }
                }
                Err(parse_error) => {
                    eprintln!("the fixtures do not parse: {parse_error}");
                    std::process::exit(1);
                }
            }

            let floor_started_at = Instant::now();
            let floor_event_count = run_floor(&tokens);

            floor_duration += floor_started_at.elapsed();
            std::hint::black_box(floor_event_count);
        }

        lex_durations.push(lex_duration);
        parse_durations.push(parse_duration);
        floor_durations.push(floor_duration);
    }

    let lex_minimum = get_minimum(&lex_durations);
    let parse_minimum = get_minimum(&parse_durations);
    let combined_minimum = get_combined_minimum(&lex_durations, &parse_durations);
    let parse_median = get_median(&mut parse_durations);
    let floor_minimum = get_minimum(&floor_durations);
    let nanoseconds_per_token = parse_minimum.as_nanos() as f64 / meaningful_token_count as f64;
    let floor_nanoseconds_per_token = floor_minimum.as_nanos() as f64 / meaningful_token_count as f64;

    println!(
        "{megabyte_count:.1} MB as {parse_count} file(s): {fixture_count} fixtures x {copy_count} copies, {meaningful_token_count} meaningful tokens, {event_count} events, {pass_count} passes"
    );
    println!(
        "lex      {:>8.1} ms  {:>7.0} MB/s",
        to_milliseconds(lex_minimum),
        megabyte_count / lex_minimum.as_secs_f64()
    );
    println!(
        "parse    {:>8.1} ms  {:>7.0} MB/s  {nanoseconds_per_token:.2} ns per meaningful token (median {:.1} ms)",
        to_milliseconds(parse_minimum),
        megabyte_count / parse_minimum.as_secs_f64(),
        to_milliseconds(parse_median)
    );
    println!(
        "floor    {:>8.1} ms  {:>7.0} MB/s  {floor_nanoseconds_per_token:.2} ns per meaningful token",
        to_milliseconds(floor_minimum),
        megabyte_count / floor_minimum.as_secs_f64()
    );
    println!(
        "together {:>8.1} ms  {:>7.0} MB/s",
        to_milliseconds(combined_minimum),
        megabyte_count / combined_minimum.as_secs_f64()
    );
    println!(
        "events per meaningful token: {:.2}; tree bytes per source byte: {:.2}",
        event_count as f64 / meaningful_token_count as f64,
        (event_count * size_of::<TreeEvent>() + meaningful_token_count * size_of::<u32>()) as f64 / total_bytes as f64
    );
    // :ResultLine
    // The line `compare-builds` reads. The first number is the headline it gives its verdict on.
    let mut result_line = ResultLine::new();

    result_line.add_number("parse_min_ms", to_milliseconds(parse_minimum));
    result_line.add_number("parse_median_ms", to_milliseconds(parse_median));
    result_line.add_number("floor_min_ms", to_milliseconds(floor_minimum));
    result_line.add_number("lex_min_ms", to_milliseconds(lex_minimum));
    result_line.add_precise_number("parse_per_token_ns", nanoseconds_per_token, 2);
    result_line.add_number("together_min_ms", to_milliseconds(combined_minimum));
    let fingerprint = match tree_fingerprints.as_slice() {
        [] => 0,
        [one_tree] => *one_tree,
        hashes => {
            let mut corpus = Fingerprint::new();
            for &hash in hashes {
                corpus.mix(hash);
            }
            corpus.get_value()
        }
    };
    result_line.add_text("fingerprint", &format!("{fingerprint:016x}"));

    println!("{}", result_line.to_line());
}

/// Reads the accept fixtures in stable filename order and preserves their file boundaries.
fn read_fixtures(only_filter: Option<&str>) -> Vec<Vec<u8>> {
    let fixture_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/accept");

    let mut fixture_paths: Vec<_> = match std::fs::read_dir(&fixture_directory) {
        Ok(directory_entries) => directory_entries
            .filter_map(|directory_entry| directory_entry.ok().map(|entry| entry.path()))
            .filter(|fixture_path| {
                fixture_path
                    .extension()
                    .is_some_and(|extension| extension == "omg")
            })
            .collect(),
        Err(read_error) => {
            eprintln!("could not read {}: {read_error}", fixture_directory.display());
            std::process::exit(1);
        }
    };

    fixture_paths.retain(|fixture_path| {
        only_filter.is_none_or(|wanted_text| {
            fixture_path
                .file_name()
                .is_some_and(|file_name| file_name.to_string_lossy().contains(wanted_text))
        })
    });
    fixture_paths.sort();

    let mut fixtures = Vec::new();

    for fixture_path in &fixture_paths {
        match std::fs::read(fixture_path) {
            Ok(fixture_bytes) => fixtures.push(fixture_bytes),
            Err(read_error) => {
                eprintln!("could not read {}: {read_error}", fixture_path.display());
                std::process::exit(1);
            }
        }
    }

    if fixtures.iter().all(Vec::is_empty) {
        eprintln!("no fixtures found in {}", fixture_directory.display());
        std::process::exit(1);
    }

    fixtures
}

/*
    The sources are lexed once before timing. Each thread parses all of them for
    each repetition. If parsing scaled perfectly, N threads would do
    N times the work in the same time; the table shows what they do. The
    lexer's version of this found that a fresh list per file made the threads
    queue for the memory allocator (its experiment 4), and the parser returns
    two fresh lists per file, so this is the first place to look.
*/
fn run_thread_scaling(
    sources: &[Vec<u8>],
    repetitions: usize,
    megabyte_count: f64,
) {
    let hardware_thread_count = std::thread::available_parallelism().map_or(1, |count| count.get());
    let mut thread_counts = vec![1usize, 2, 4, 8, 16, hardware_thread_count];

    thread_counts.retain(|&thread_count| thread_count <= hardware_thread_count);
    thread_counts.dedup();

    let token_lists: Vec<_> = sources
        .iter()
        .map(|source| {
            omega_lexer::lex(source).unwrap_or_else(|error| {
                eprintln!("the fixtures do not lex: {error}");
                std::process::exit(1);
            })
        })
        .collect();
    let parse_count = sources.len() * repetitions;

    println!("{megabyte_count:.1} MB per thread as {parse_count} file(s); parsing only");
    println!("{:>8} {:>10} {:>10} {:>10}", "threads", "wall ms", "MB/s", "x 1 thread");

    let mut single_thread_rate = 0.0f64;

    for thread_count in thread_counts {
        let mut best_wall = Duration::MAX;

        for _ in 0..3 {
            let started_at = Instant::now();

            std::thread::scope(|scope| {
                for _ in 0..thread_count {
                    scope.spawn(|| {
                        for (source, tokens) in sources.iter().zip(&token_lists).cycle().take(parse_count) {
                            let parse_result = parse_file(source, tokens);

                            std::hint::black_box(parse_result.is_ok());
                        }
                    });
                }
            });

            best_wall = best_wall.min(started_at.elapsed());
        }

        let rate = megabyte_count * thread_count as f64 / best_wall.as_secs_f64();

        if thread_count == 1 {
            single_thread_rate = rate;
        }

        println!(
            "{thread_count:>8} {:>10.1} {rate:>10.0} {:>10.1}",
            to_milliseconds(best_wall),
            rate / single_thread_rate
        );
    }
}

/// The least a parser over these tokens could do: drop the trivia, number what is left, and write
/// one event for each. No grammar, no lookahead, no nodes.
fn run_floor(tokens: &[Token]) -> usize {
    let mut meaningful_token_indices: Vec<u32> = Vec::with_capacity(tokens.len() / 2 + 1);
    let mut events: Vec<TreeEvent> = Vec::with_capacity(tokens.len() * 2);

    for (token_index, token) in tokens.iter().enumerate() {
        if token.get_kind().is_trivia() {
            continue;
        }

        meaningful_token_indices.push(token_index as u32);
        events.push(TreeEvent::Token);
    }

    std::hint::black_box(&meaningful_token_indices);

    events.len()
}

/// A hash of every event and every token index. Equal fingerprints mean equal trees.
fn get_fingerprint(syntax_tree: &SyntaxTree) -> u64 {
    let mut fingerprint = Fingerprint::new();

    for tree_event in syntax_tree.get_events() {
        match tree_event {
            TreeEvent::Open(node_kind) => {
                fingerprint.mix_byte(1);
                fingerprint.mix_byte(*node_kind as u8);
            }
            TreeEvent::Close => fingerprint.mix_byte(2),
            TreeEvent::Token => fingerprint.mix_byte(3),
        }
    }

    for token_index in syntax_tree.get_meaningful_token_indices() {
        for token_index_byte in token_index.to_le_bytes() {
            fingerprint.mix_byte(token_index_byte);
        }
    }

    fingerprint.get_value()
}

fn get_minimum(durations: &[Duration]) -> Duration {
    durations.iter().copied().min().unwrap_or_default()
}

fn get_combined_minimum(
    lex_durations: &[Duration],
    parse_durations: &[Duration],
) -> Duration {
    lex_durations
        .iter()
        .zip(parse_durations)
        .map(|(lex, parse)| *lex + *parse)
        .min()
        .unwrap_or_default()
}

fn get_median(durations: &mut [Duration]) -> Duration {
    durations.sort_unstable();

    durations.get(durations.len() / 2).copied().unwrap_or_default()
}

fn to_milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_time_comes_from_one_observed_pass() {
        let lex = [1, 5].map(Duration::from_millis);
        let parse = [7, 2].map(Duration::from_millis);
        assert_eq!(get_combined_minimum(&lex, &parse), Duration::from_millis(7));
    }
}

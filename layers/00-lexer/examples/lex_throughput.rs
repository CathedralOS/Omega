/*
    Measures how fast the lexer is on a real body of Omega source, and gives
    the numbers needed to judge whether that is fast.

        cargo run --release -p omega-lexer --example lex_throughput -- [--passes N] <directory>...

    A single MB/s figure cannot be judged without knowing what bounds it and
    what it is made of, so we take five measurements over the same files, all
    held in memory first so that no disk time is included:

      1. UTF-8 validation alone. The lexer does this pass before scanning, so
         it is a floor: lexing cannot be faster than this plus something.
      2. Classifying every byte through a table. A second floor: what a
         byte-at-a-time lexer pays before it decides or stores anything.
      3. Lexing each file separately, which is the real workload. Each call
         allocates its own token list.
      4. Lexing all files joined into one buffer. Same bytes, one allocation.
         The gap between 3 and 4 is the fixed cost paid per file.
      5. The token mix: how many tokens and bytes each kind accounts for.

    Every timed measurement is repeated for the requested number of passes,
    interleaved so that drift in machine load hits all of them alike, and we
    report the fastest, median and slowest pass. The spread between passes is
    the noise; a difference smaller than it means nothing.
*/

#[path = "support/omega_files.rs"]
mod omega_files;

use omega_lexer::{TokenKind, lex};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

const DEFAULT_PASS_COUNT: usize = 9;

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
        eprintln!("usage: lex_throughput [--passes N] <directory>...");
        std::process::exit(2);
    }

    let mut source_paths = Vec::new();
    for search_directory in &search_directories {
        omega_files::collect_omega_files(search_directory, &mut source_paths);
    }
    source_paths.sort();
    let sources: Vec<Vec<u8>> = source_paths
        .iter()
        .filter_map(|source_path| std::fs::read(source_path).ok())
        .collect();

    let total_byte_count: usize = sources.iter().map(Vec::len).sum();
    if total_byte_count == 0 {
        eprintln!("no .omg source found");
        std::process::exit(1);
    }

    // Files are independent, so joining them with a line feed gives one valid source with the same tokens
    // plus one whitespace byte per file.
    let joined_source = sources.join(&b"\n"[..]);

    let mut utf8_seconds = Vec::with_capacity(pass_count);
    let mut class_run_seconds = Vec::with_capacity(pass_count);
    let mut per_file_seconds = Vec::with_capacity(pass_count);
    let mut joined_seconds = Vec::with_capacity(pass_count);
    let mut total_token_count = 0usize;
    let mut refused_file_count = 0usize;

    for _ in 0..pass_count {
        let started = Instant::now();
        for source_bytes in &sources {
            black_box(std::str::from_utf8(black_box(source_bytes)).is_ok());
        }
        utf8_seconds.push(started.elapsed().as_secs_f64());

        let started = Instant::now();
        let mut class_run_count = 0usize;
        for source_bytes in &sources {
            class_run_count += count_byte_class_runs(black_box(source_bytes));
        }
        black_box(class_run_count);
        class_run_seconds.push(started.elapsed().as_secs_f64());

        let started = Instant::now();
        let mut pass_token_count = 0usize;
        let mut pass_refused_count = 0usize;
        for source_bytes in &sources {
            match lex(black_box(source_bytes)) {
                Ok(tokens) => pass_token_count += black_box(tokens).len(),
                Err(_) => pass_refused_count += 1,
            }
        }
        per_file_seconds.push(started.elapsed().as_secs_f64());
        total_token_count = pass_token_count;
        refused_file_count = pass_refused_count;

        let started = Instant::now();
        black_box(
            lex(black_box(&joined_source))
                .map(|tokens| tokens.len())
                .unwrap_or(0),
        );
        joined_seconds.push(started.elapsed().as_secs_f64());
    }

    // :PredictableInput
    // The same 4 KB chunk lexed over and over: one line of ordinary code repeated a hundred times.
    // A branch predictor learns a pattern this regular almost perfectly, which real source never
    // lets it do. The per-token cost here against the per-token cost on real files therefore
    // shows how much of the lexer's time is branches being guessed wrong.
    let repeated_chunk = b"    counter.value = offset + width * 2;\n".repeat(100);
    let repetition_count = (total_byte_count / repeated_chunk.len()).max(1);
    let mut repeated_seconds = Vec::with_capacity(pass_count);
    let mut repeated_token_count = 0usize;
    for _ in 0..pass_count {
        let started = Instant::now();
        let mut pass_token_count = 0usize;
        for _ in 0..repetition_count {
            pass_token_count += lex(black_box(&repeated_chunk))
                .map(|tokens| black_box(tokens).len())
                .unwrap_or(0);
        }
        repeated_seconds.push(started.elapsed().as_secs_f64());
        repeated_token_count = pass_token_count;
    }

    let mut file_byte_counts: Vec<usize> = sources.iter().map(Vec::len).collect();
    file_byte_counts.sort_unstable();

    println!("files:            {}", sources.len());
    println!("bytes:            {total_byte_count}");
    println!(
        "median file:      {} bytes",
        file_byte_counts[file_byte_counts.len() / 2]
    );
    println!("tokens:           {total_token_count}");
    println!("refused files:    {refused_file_count}");
    println!("passes:           {pass_count}");
    println!();
    println!("                          fastest    median   slowest");
    print_measurement("UTF-8 validation only", &mut utf8_seconds, total_byte_count);
    print_measurement(
        "classify every byte",
        &mut class_run_seconds,
        total_byte_count,
    );
    print_measurement("lex, file by file", &mut per_file_seconds, total_byte_count);
    print_measurement(
        "lex, one joined buffer",
        &mut joined_seconds,
        joined_source.len(),
    );

    print_measurement(
        "lex, one line repeated",
        &mut repeated_seconds,
        repetition_count * repeated_chunk.len(),
    );

    let fastest_per_file_seconds = per_file_seconds.first().copied().unwrap_or(0.0);
    let fastest_repeated_seconds = repeated_seconds.first().copied().unwrap_or(0.0);
    println!();
    println!(
        "per token (file by file, fastest):  {:.1} ns",
        fastest_per_file_seconds * 1e9 / total_token_count.max(1) as f64
    );
    // :PredictableInput
    println!(
        "per token (one line repeated):      {:.1} ns",
        fastest_repeated_seconds * 1e9 / repeated_token_count.max(1) as f64
    );

    print_token_mix(&joined_source, total_byte_count);
}

/// Looks every byte up in a class table and counts how many times the class changes.
///
/// This is a floor for a byte-at-a-time lexer, not a lexer: it does the one thing no such lexer
/// can avoid, a load and a table lookup per byte, and none of the work of deciding token
/// boundaries or storing tokens. How far real lexing sits above it is the room left to gain
/// without changing approach.
fn count_byte_class_runs(source_bytes: &[u8]) -> usize {
    const BYTE_CLASSES: [u8; 256] = build_byte_classes();

    let mut previous_class = u8::MAX;
    let mut run_count = 0usize;
    for &byte in source_bytes {
        let class = BYTE_CLASSES[usize::from(byte)];
        run_count += usize::from(class != previous_class);
        previous_class = class;
    }
    run_count
}

const fn build_byte_classes() -> [u8; 256] {
    let mut classes = [5u8; 256];
    let mut byte = 0usize;
    while byte < 256 {
        classes[byte] = match byte as u8 {
            b' ' | b'\t' | b'\r' | b'\n' => 0,
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => 1,
            b'0'..=b'9' => 2,
            b'"' => 3,
            0x80..=0xFF => 4,
            _ => 5,
        };
        byte += 1;
    }
    classes
}

/// Prints one row of throughput in MB/s. Sorts `pass_seconds` fastest first as a side effect.
fn print_measurement(label: &str, pass_seconds: &mut [f64], byte_count: usize) {
    pass_seconds.sort_by(|left, right| left.total_cmp(right));
    let megabytes = byte_count as f64 / 1_000_000.0;
    let megabytes_per_second = |seconds: f64| {
        if seconds > 0.0 {
            megabytes / seconds
        } else {
            0.0
        }
    };
    let fastest = pass_seconds.first().copied().unwrap_or(0.0);
    let median = pass_seconds
        .get(pass_seconds.len() / 2)
        .copied()
        .unwrap_or(0.0);
    let slowest = pass_seconds.last().copied().unwrap_or(0.0);
    println!(
        "{label:<24} {:>7.0}   {:>7.0}   {:>7.0}   MB/s",
        megabytes_per_second(fastest),
        megabytes_per_second(median),
        megabytes_per_second(slowest)
    );
}

/// Shows which kinds of token the workload is made of, by count and by bytes covered.
fn print_token_mix(joined_source: &[u8], total_byte_count: usize) {
    let Ok(tokens) = lex(joined_source) else {
        println!("\n(token mix unavailable: the joined source did not lex)");
        return;
    };

    let mut mix: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
    for token in &tokens {
        let kind_label = match token.kind {
            TokenKind::Whitespace => "Whitespace",
            TokenKind::LineComment => "LineComment",
            TokenKind::BlockComment => "BlockComment",
            TokenKind::Word => "Word",
            TokenKind::Integer { .. } => "Integer",
            TokenKind::Float { .. } => "Float",
            TokenKind::Quoted => "Quoted",
            TokenKind::Punctuation(_) => "Punctuation",
        };
        let entry = mix.entry(kind_label).or_default();
        entry.0 += 1;
        entry.1 += token.span.get_byte_count();
    }

    println!("token fingerprint: {:016x}", fingerprint_tokens(&tokens));
    println!("\ntoken mix                 tokens   share      bytes   share");
    let mut rows: Vec<_> = mix.into_iter().collect();
    rows.sort_by_key(|&(_, (token_count, _))| std::cmp::Reverse(token_count));
    for (kind_label, (token_count, byte_count)) in rows {
        println!(
            "{kind_label:<22} {token_count:>9}   {:>4.1}%  {byte_count:>9}   {:>4.1}%",
            token_count as f64 * 100.0 / tokens.len() as f64,
            byte_count as f64 * 100.0 / total_byte_count as f64
        );
    }
}

/// A 64-bit FNV-1a hash of every token's kind and byte range.
///
/// Two builds of the lexer that print the same fingerprint for the same files produced the same
/// tokens, which is how a change meant only to be faster is shown to have changed nothing else.
fn fingerprint_tokens(tokens: &[omega_lexer::Token]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = FNV_OFFSET_BASIS;
    let mut mix = |value: u64| {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    };

    for token in tokens {
        let (kind_tag, kind_detail) = match token.kind {
            TokenKind::Whitespace => (0u64, 0u64),
            TokenKind::LineComment => (1, 0),
            TokenKind::BlockComment => (2, 0),
            TokenKind::Word => (3, 0),
            TokenKind::Integer {
                radix,
                suffix_start,
            } => (4, u64::from(radix.get_base()) << 48 | suffix_start as u64),
            TokenKind::Float { suffix_start } => (5, suffix_start as u64),
            TokenKind::Quoted => (6, 0),
            TokenKind::Punctuation(punctuation) => (7, punctuation as u64),
        };
        mix(kind_tag);
        mix(kind_detail);
        mix(token.span.start as u64);
        mix(token.span.end as u64);
    }
    hash
}

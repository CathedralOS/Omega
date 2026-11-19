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

use measure::{Fingerprint, PassTimings, ResultLine, ToolArguments, collect_source_files, megabytes_per_second};
use omega_lexer::{Token, TokenKind, lex};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

const DEFAULT_PASS_COUNT: usize = 9;

fn main() {
    let tool_arguments = ToolArguments::from_command_line("lex_throughput", DEFAULT_PASS_COUNT);
    let pass_count = tool_arguments.get_pass_count();
    let search_directories = tool_arguments.get_search_directories();

    let mut source_paths = Vec::new();

    for search_directory in search_directories {
        collect_source_files(search_directory, "omg", &mut source_paths);
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

    let mut utf8_timings = PassTimings::new(pass_count);
    let mut class_run_timings = PassTimings::new(pass_count);
    let mut per_file_timings = PassTimings::new(pass_count);
    let mut joined_timings = PassTimings::new(pass_count);
    let mut total_token_count = 0usize;
    let mut refused_file_count = 0usize;

    for _ in 0..pass_count {
        let start_time = Instant::now();

        for source_bytes in &sources {
            black_box(std::str::from_utf8(black_box(source_bytes)).is_ok());
        }

        utf8_timings.record_seconds(start_time.elapsed().as_secs_f64());

        let start_time = Instant::now();
        let mut class_run_count = 0usize;

        for source_bytes in &sources {
            class_run_count += count_byte_class_runs(black_box(source_bytes));
        }

        black_box(class_run_count);
        class_run_timings.record_seconds(start_time.elapsed().as_secs_f64());

        let start_time = Instant::now();
        let mut pass_token_count = 0usize;
        let mut pass_refused_count = 0usize;

        for source_bytes in &sources {
            match lex(black_box(source_bytes)) {
                Ok(tokens) => pass_token_count += black_box(tokens).len(),
                Err(_) => pass_refused_count += 1,
            }
        }

        per_file_timings.record_seconds(start_time.elapsed().as_secs_f64());
        total_token_count = pass_token_count;
        refused_file_count = pass_refused_count;

        let start_time = Instant::now();
        black_box(
            lex(black_box(&joined_source))
                .map(|tokens| tokens.len())
                .unwrap_or(0),
        );
        joined_timings.record_seconds(start_time.elapsed().as_secs_f64());
    }

    // :PredictableInput
    // The same 4 KB chunk lexed over and over: one line of ordinary code repeated a hundred times.
    // A branch predictor learns a pattern this regular almost perfectly, which real source never
    // lets it do. The per-token cost here against the per-token cost on real files therefore
    // shows how much of the lexer's time is branches being guessed wrong.
    let repeated_chunk = b"    counter.value = offset + width * 2;\n".repeat(100);
    let repetition_count = (total_byte_count / repeated_chunk.len()).max(1);
    let mut repeated_timings = PassTimings::new(pass_count);
    let mut repeated_token_count = 0usize;

    for _ in 0..pass_count {
        let start_time = Instant::now();
        let mut pass_token_count = 0usize;

        for _ in 0..repetition_count {
            pass_token_count += lex(black_box(&repeated_chunk))
                .map(|tokens| black_box(tokens).len())
                .unwrap_or(0);
        }

        repeated_timings.record_seconds(start_time.elapsed().as_secs_f64());
        repeated_token_count = pass_token_count;
    }

    let mut file_byte_counts: Vec<usize> = sources.iter().map(Vec::len).collect();
    file_byte_counts.sort_unstable();

    println!("files:            {}", sources.len());
    println!("bytes:            {total_byte_count}");
    println!("median file:      {} bytes", file_byte_counts[file_byte_counts.len() / 2]);
    println!("tokens:           {total_token_count}");
    println!("refused files:    {refused_file_count}");
    println!("passes:           {pass_count}");
    println!();
    println!("                          fastest    median   slowest");
    print_measurement("UTF-8 validation only", &utf8_timings, total_byte_count);
    print_measurement("classify every byte", &class_run_timings, total_byte_count);
    print_measurement("lex, file by file", &per_file_timings, total_byte_count);
    print_measurement("lex, one joined buffer", &joined_timings, joined_source.len());

    print_measurement("lex, one line repeated", &repeated_timings, repetition_count * repeated_chunk.len());

    let fastest_per_file_seconds = per_file_timings.get_fastest_seconds();
    let fastest_repeated_seconds = repeated_timings.get_fastest_seconds();
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

    let token_fingerprint = print_token_mix(&joined_source, total_byte_count);

    // :ResultLine
    // The line `compare-builds` reads. The first number is the headline it gives its verdict on:
    // lexing file by file is the real workload, and the fastest pass is the one least disturbed
    // by other work on the machine.
    let mut result_line = ResultLine::new();

    result_line.add_number(
        "per_file_fastest_mbs",
        megabytes_per_second(total_byte_count, per_file_timings.get_fastest_seconds()),
    );
    result_line.add_number(
        "per_file_median_mbs",
        megabytes_per_second(total_byte_count, per_file_timings.get_median_seconds()),
    );
    result_line.add_number(
        "joined_fastest_mbs",
        megabytes_per_second(joined_source.len(), joined_timings.get_fastest_seconds()),
    );
    result_line.add_number(
        "repeated_line_fastest_mbs",
        megabytes_per_second(repetition_count * repeated_chunk.len(), repeated_timings.get_fastest_seconds()),
    );
    result_line.add_text("fingerprint", &token_fingerprint);

    println!(
        "
{}",
        result_line.to_line()
    );
}

/// Looks every byte up in a class table and counts how many times the class changes.
///
/// This is a floor for a byte-at-a-time lexer, not a lexer: it does the one thing no such lexer
/// can avoid, a load and a table lookup per byte, and none of the work of deciding token
/// boundaries or storing tokens. How far real lexing sits above it is the room left to gain
/// without changing approach.
fn count_byte_class_runs(source_bytes: &[u8]) -> usize {
    const BYTE_CLASSES: [u8; 256] = build_byte_classes();

    let mut previous_byte_class = u8::MAX;
    let mut run_count = 0usize;

    for &source_byte in source_bytes {
        let byte_class = BYTE_CLASSES[usize::from(source_byte)];
        run_count += usize::from(byte_class != previous_byte_class);
        previous_byte_class = byte_class;
    }

    run_count
}

const fn build_byte_classes() -> [u8; 256] {
    let mut byte_classes = [5u8; 256];
    let mut byte_value = 0usize;

    while byte_value < 256 {
        byte_classes[byte_value] = match byte_value as u8 {
            b' ' | b'\t' | b'\r' | b'\n' => 0,
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => 1,
            b'0'..=b'9' => 2,
            b'"' => 3,
            0x80..=0xFF => 4,
            _ => 5,
        };
        byte_value += 1;
    }

    byte_classes
}

/// Prints one row of throughput in MB/s.
fn print_measurement(
    label: &str,
    pass_timings: &PassTimings,
    byte_count: usize,
) {
    println!(
        "{label:<24} {:>7.0}   {:>7.0}   {:>7.0}   MB/s",
        megabytes_per_second(byte_count, pass_timings.get_fastest_seconds()),
        megabytes_per_second(byte_count, pass_timings.get_median_seconds()),
        megabytes_per_second(byte_count, pass_timings.get_slowest_seconds())
    );
}

/// Shows which kinds of token the workload is made of, by count and by bytes covered.
/// Returns the fingerprint of the tokens, or a word saying there is none.
fn print_token_mix(
    joined_source: &[u8],
    total_byte_count: usize,
) -> String {
    let Ok(tokens) = lex(joined_source) else {
        println!("\n(token mix unavailable: the joined source did not lex)");

        return String::from("unavailable");
    };

    let token_fingerprint = fingerprint_tokens(&tokens).get_hex_text();

    let mut token_mix: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();

    for token in &tokens {
        let kind_label = token.get_kind().get_name();
        let kind_totals = token_mix.entry(kind_label).or_default();
        kind_totals.0 += 1;
        kind_totals.1 += token.get_span().get_byte_count();
    }

    println!("token fingerprint: {token_fingerprint}");
    println!("\ntoken mix                 tokens   share      bytes   share");
    let mut token_mix_rows: Vec<_> = token_mix.into_iter().collect();
    token_mix_rows.sort_by_key(|&(_, (token_count, _))| std::cmp::Reverse(token_count));

    for (kind_label, (token_count, byte_count)) in token_mix_rows {
        println!(
            "{kind_label:<22} {token_count:>9}   {:>4.1}%  {byte_count:>9}   {:>4.1}%",
            token_count as f64 * 100.0 / tokens.len() as f64,
            byte_count as f64 * 100.0 / total_byte_count as f64
        );
    }

    token_fingerprint
}

/// A 64-bit FNV-1a hash of every token's kind and byte range.
///
/// Two builds of the lexer that print the same fingerprint for the same files produced the same
/// tokens, which is how a change meant only to be faster is shown to have changed nothing else.
fn fingerprint_tokens(tokens: &[Token]) -> Fingerprint {
    let mut fingerprint = Fingerprint::new();

    for token in tokens {
        let (kind_tag, kind_detail) = match token.get_kind() {
            TokenKind::Whitespace => (0u64, 0u64),
            TokenKind::LineComment => (1, 0),
            TokenKind::BlockComment => (2, 0),
            TokenKind::Word => (3, 0),
            TokenKind::Integer { radix, suffix_start_offset } => (4, u64::from(radix.get_base()) << 48 | suffix_start_offset as u64),
            TokenKind::Float { suffix_start_offset } => (5, suffix_start_offset as u64),
            TokenKind::Quoted => (6, 0),
            TokenKind::Punctuation(punctuation) => (7, punctuation as u64),
        };

        fingerprint.mix(kind_tag);
        fingerprint.mix(kind_detail);
        fingerprint.mix(token.get_span().get_start_offset() as u64);
        fingerprint.mix(token.get_span().get_end_offset() as u64);
    }

    fingerprint
}

/*
    Lexes every `.omg` file under the directories given on the command line
    and reports what was refused.

    The fixtures and the spec's examples are small and were written by people
    who knew what the lexer accepts. This tool points the lexer at a large body
    of Omega source nobody wrote for it, such as Omega V1's own library, samples
    and test fixtures, to find spellings the contract missed. A refusal here is
    not automatically a bug: a tree can contain files that are meant to be
    invalid. It is a list of things to look at.

        cargo run --release -p omega-lexer --example lex_tree -- <directory>...

    Directories named `target` or starting with a dot are skipped, because a
    checkout can hold build output and other working copies of the same files.
*/

use measure::collect_source_files;
use omega_lexer::{LexErrorKind, LineIndex, lex};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

/// How many refused files to list for each kind of error.
const EXAMPLES_PER_ERROR_KIND: usize = 8;

struct Refusal {
    file_path: PathBuf,
    line: usize,
    column: usize,
    offending_text: String,
}

fn main() {
    let search_directories: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    if search_directories.is_empty() {
        eprintln!("usage: lex_tree <directory>...");
        std::process::exit(2);
    }

    let mut source_paths = Vec::new();

    for search_directory in &search_directories {
        collect_source_files(search_directory, "omg", &mut source_paths);
    }

    source_paths.sort();

    let mut accepted_file_count = 0usize;
    let mut unreadable_file_count = 0usize;
    let mut total_byte_count = 0usize;
    let mut total_token_count = 0usize;
    let mut refusals_by_kind: BTreeMap<String, Vec<Refusal>> = BTreeMap::new();
    let mut lexing_seconds = 0.0f64;

    for source_path in &source_paths {
        let Ok(source_bytes) = std::fs::read(source_path) else {
            unreadable_file_count += 1;
            continue;
        };
        total_byte_count += source_bytes.len();

        // Only the lexer is timed. Reading 19,000 small files costs far more than lexing them.
        let lex_started = Instant::now();
        let lex_result = lex(&source_bytes);
        lexing_seconds += lex_started.elapsed().as_secs_f64();

        match lex_result {
            Ok(tokens) => {
                accepted_file_count += 1;
                total_token_count += tokens.len();
            }
            Err(lex_error) => {
                let source_position = LineIndex::new(&source_bytes).get_position(lex_error.get_span().get_start_offset());
                let offending_bytes = lex_error.get_span().slice(&source_bytes).unwrap_or_default();
                refusals_by_kind
                    .entry(describe_kind(lex_error.get_kind()))
                    .or_default()
                    .push(Refusal {
                        file_path: source_path.clone(),
                        line: source_position.get_line(),
                        column: source_position.get_column(),
                        offending_text: String::from_utf8_lossy(offending_bytes)
                            .chars()
                            .take(40)
                            .collect(),
                    });
            }
        }
    }

    let refused_file_count: usize = refusals_by_kind.values().map(Vec::len).sum();
    println!("files found:    {}", source_paths.len());
    println!("accepted:       {accepted_file_count}");
    println!("refused:        {refused_file_count}");
    println!("unreadable:     {unreadable_file_count}");
    println!("bytes lexed:    {total_byte_count}");
    println!("tokens:         {total_token_count}");

    if lexing_seconds > 0.0 {
        println!(
            "lexing time:    {:.3} s ({:.0} MB/s)",
            lexing_seconds,
            total_byte_count as f64 / 1_000_000.0 / lexing_seconds
        );
    }

    for (kind, refusals) in &refusals_by_kind {
        println!("\n{kind}: {} file(s)", refusals.len());

        for refusal in refusals.iter().take(EXAMPLES_PER_ERROR_KIND) {
            println!(
                "  {}:{}:{}  {:?}",
                refusal.file_path.display(),
                refusal.line,
                refusal.column,
                refusal.offending_text
            );
        }

        if refusals.len() > EXAMPLES_PER_ERROR_KIND {
            println!("  ... and {} more", refusals.len() - EXAMPLES_PER_ERROR_KIND);
        }
    }
}

fn describe_kind(kind: LexErrorKind) -> String {
    format!("{kind:?}")
}

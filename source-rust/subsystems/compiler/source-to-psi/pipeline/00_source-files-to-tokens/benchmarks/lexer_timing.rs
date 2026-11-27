//! Time the real lexer, optionally through parsing, with retained source inputs.
//! The ordinary omega-bench runner owns pairing, resource capture and output checks.

use source_files::LoadedSource;
use source_text::SourceId;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::hint::black_box;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use syntax_trees::SyntaxTrees;
use tokens::TokenScratch;

fn main() -> Result<(), String> {
    let budget =
        worker_budget::ExecutionBudget::with_workers(1).map_err(|error| error.to_string())?;
    budget.run(measure)
}

fn measure() -> Result<(), String> {
    // Cargo supplies this marker even when the benchmark uses its own harness.
    let arguments: Vec<_> = std::env::args()
        .skip(1)
        .filter(|argument| argument != "--bench")
        .collect();
    if arguments.len() < 3 {
        return Err("usage: lexer_timing <lex|parse> <repeats> <source>...".into());
    }
    let parse = match arguments[0].as_str() {
        "lex" => false,
        "parse" => true,
        _ => return Err("mode must be lex or parse".into()),
    };
    let repeats: u64 = arguments[1]
        .parse()
        .map_err(|error| format!("repeats: {error}"))?;
    if repeats == 0 {
        return Err("repeats must be positive".into());
    }
    let sources: Vec<Arc<str>> = arguments[2..]
        .iter()
        .map(|path| {
            std::fs::read_to_string(Path::new(path))
                .map(Arc::from)
                .map_err(|error| format!("read {path}: {error}"))
        })
        .collect::<Result<_, _>>()?;
    let bytes: u64 = sources.iter().map(|source| source.len() as u64).sum();
    let work_units = bytes.checked_mul(repeats).ok_or("work size overflow")?;
    if work_units == 0 {
        return Err("sources must contain at least one byte".into());
    }

    // Hash every token's kind, range and decoded bytes outside the timed loop.
    // Candidate runs must preserve this complete observation, not only token counts.
    let mut fingerprint = DefaultHasher::new();
    let mut scratch = TokenScratch::default();
    for (source_index, source) in sources.iter().enumerate() {
        source.hash(&mut fingerprint);
        let lexed = source_compiler::test_support::lexing::lex_file(
            &mut loaded(source.clone()),
            &mut scratch,
        )
        .map_err(|error| format!("{}: {}", arguments[source_index + 2], error.message))?;
        let tokens = &lexed.tokens;
        tokens.len().hash(&mut fingerprint);
        for token in tokens.iter() {
            format!("{:?}", token.kind).hash(&mut fingerprint);
            token.span.start.hash(&mut fingerprint);
            token.span.end.hash(&mut fingerprint);
            if let Some(bytes) = tokens.literal_bytes(token) {
                bytes.as_ref().hash(&mut fingerprint);
            } else {
                tokens.spelling(token).as_bytes().hash(&mut fingerprint);
            }
        }
        lexed.tokens.recycle(&mut scratch);
    }

    let mut checksum = 0u64;
    let start = Instant::now();
    for _ in 0..black_box(repeats) {
        for source in &sources {
            let mut lexed = source_compiler::test_support::lexing::lex_file(
                &mut loaded(black_box(source.clone())),
                &mut scratch,
            )
            .map_err(|error| error.message)?;
            checksum = checksum.wrapping_add(black_box(lexed.tokens.len()) as u64);
            black_box(&lexed.tokens).as_slice();
            if parse {
                let mut syntax = SyntaxTrees::new(SourceId(1));
                let parsed =
                    source_compiler::test_support::parsing::parse_file(&mut syntax, &mut lexed)
                        .map_err(|error| error.message)?;
                checksum = checksum.wrapping_add(black_box(parsed.root_items.len()) as u64);
                black_box(&syntax);
            }
            lexed.tokens.recycle(&mut scratch);
        }
    }
    let elapsed = start.elapsed().as_nanos();
    println!(
        "{{\"schema\":\"omega-timed-work/1\",\"elapsed_ns\":{elapsed},\"work_units\":{work_units},\"checksum\":\"{}:{checksum}\"}}",
        fingerprint.finish()
    );
    Ok(())
}

fn loaded(source: Arc<str>) -> LoadedSource {
    LoadedSource {
        source_id: SourceId(1),
        source,
        ..Default::default()
    }
}

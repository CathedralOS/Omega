//! Time parsing of retained, pre-lexed token streams; omega-bench owns paired comparisons.
//! Syntax record bytes and token fingerprints are checked outside the timed loop.

use source_files::LoadedSource;
use source_text::SourceId;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::hint::black_box;
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
    let arguments: Vec<_> = std::env::args()
        .skip(1)
        .filter(|argument| argument != "--bench")
        .collect();
    if arguments.len() < 2 {
        return Err("usage: parser_timing <repeats> <source>...".into());
    }
    let repeats: u64 = arguments[0]
        .parse()
        .map_err(|error| format!("repeats: {error}"))?;
    if repeats == 0 {
        return Err("repeats must be positive".into());
    }
    let mut inputs = Vec::with_capacity(arguments.len() - 1);
    let mut fingerprint = DefaultHasher::new();
    let mut bytes = 0u64;
    for path in &arguments[1..] {
        let source: Arc<str> = std::fs::read_to_string(path)
            .map_err(|error| format!("read {path}: {error}"))?
            .into();
        bytes = bytes
            .checked_add(source.len() as u64)
            .ok_or("input size overflow")?;
        source.hash(&mut fingerprint);
        let mut loaded = LoadedSource {
            source_id: SourceId(1),
            source: source.clone(),
            ..Default::default()
        };
        let lexed = source_compiler::test_support::lexing::lex_file(
            &mut loaded,
            &mut TokenScratch::default(),
        )
        .map_err(|error| error.message)?;
        let tokens = &lexed.tokens;
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
        let mut syntax = SyntaxTrees::new(SourceId(1));
        let roots =
            source_compiler::test_support::parsing::parse_into(&mut syntax, SourceId(1), tokens)
                .map_err(|error| format!("{path}: {}", error.message))?;
        roots.items.len().hash(&mut fingerprint);
        roots.import_range.hash(&mut fingerprint);
        record_storage::encode(&syntax, record_storage::RecordLimits::default())
            .map_err(|error| error.to_string())?
            .hash(&mut fingerprint);
        inputs.push(lexed.tokens);
    }
    let work_units = bytes.checked_mul(repeats).ok_or("work size overflow")?;
    if work_units == 0 {
        return Err("sources must contain at least one byte".into());
    }
    let mut checksum = 0u64;
    let start = Instant::now();
    for _ in 0..black_box(repeats) {
        for tokens in &inputs {
            let mut syntax = SyntaxTrees::new(SourceId(1));
            let roots = source_compiler::test_support::parsing::parse_into(
                &mut syntax,
                SourceId(1),
                black_box(tokens),
            )
            .map_err(|error| error.message)?;
            checksum = checksum.wrapping_add(black_box(roots.items.len()) as u64);
            black_box(&syntax);
        }
    }
    let elapsed = start.elapsed().as_nanos();
    println!(
        "{{\"schema\":\"omega-timed-work/1\",\"elapsed_ns\":{elapsed},\"work_units\":{work_units},\"checksum\":\"{}:{checksum}\"}}",
        fingerprint.finish()
    );
    Ok(())
}

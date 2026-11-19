//! Time parsing of retained, pre-lexed batches; omega-bench owns paired comparisons.
//! Syntax snapshots and token fingerprints are checked outside the timed loop.

use source_admission::{LoadedSource, LoadedSources};
use source_text::SourceId;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

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
        let loaded: LoadedSources = [LoadedSource {
            source_id: SourceId(1),
            source: source.clone(),
            ..Default::default()
        }]
        .into_iter()
        .collect();
        let lexed = source_compiler::lexing::lex(loaded).map_err(|error| error.message)?;
        let tokens = &lexed.sources.span_or_empty(lexed.batch)[0].tokens;
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
        let parsed = source_compiler::parsing::parse(&lexed, Default::default())
            .map_err(|error| format!("{path}: {}", error.message))?;
        let roots = parsed.sources()[0].source();
        roots.root_items.len().hash(&mut fingerprint);
        roots.import_root_range.hash(&mut fingerprint);
        parsed
            .syntax(parsed.sources()[0].syntax_handle())
            .unwrap()
            .snapshot_json()
            .map_err(|error| error.to_string())?
            .hash(&mut fingerprint);
        inputs.push(lexed);
    }
    let work_units = bytes.checked_mul(repeats).ok_or("work size overflow")?;
    if work_units == 0 {
        return Err("sources must contain at least one byte".into());
    }
    let mut checksum = 0u64;
    let start = Instant::now();
    for _ in 0..black_box(repeats) {
        for lexed in &inputs {
            let parsed = source_compiler::parsing::parse(black_box(lexed), Default::default())
                .map_err(|error| error.message)?;
            checksum = checksum
                .wrapping_add(black_box(parsed.sources()[0].source().root_items.len()) as u64);
            black_box(&parsed);
        }
    }
    let elapsed = start.elapsed().as_nanos();
    println!(
        "{{\"schema\":\"omega-timed-work/1\",\"elapsed_ns\":{elapsed},\"work_units\":{work_units},\"checksum\":\"{}:{checksum}\"}}",
        fingerprint.finish()
    );
    Ok(())
}

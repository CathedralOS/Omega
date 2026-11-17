//! Time parsing of retained, pre-lexed inputs; omega-bench owns paired comparisons.
//! Syntax snapshots and token fingerprints are checked outside the timed loop.

use source::SourceId;
use source_files_to_tokens::Lexer;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;
use syntax_trees::SyntaxTrees;

fn main() -> Result<(), String> {
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
        let tokens = Lexer::new(&source)
            .tokenize()
            .map_err(|error| error.message)?
            .retain_source(source.clone());
        for token in tokens.iter() {
            format!("{:?}", token.kind).hash(&mut fingerprint);
            token.span.start.hash(&mut fingerprint);
            token.span.end.hash(&mut fingerprint);
            tokens.text(token).as_bytes().hash(&mut fingerprint);
        }
        let mut syntax = SyntaxTrees::new(SourceId(1));
        let roots = tokens_to_syntax_trees::parse(&mut syntax, SourceId(1), &tokens)
            .map_err(|error| format!("{path}: {}", error.message))?;
        roots.items.len().hash(&mut fingerprint);
        roots.import_range.hash(&mut fingerprint);
        syntax
            .snapshot_json()
            .map_err(|error| error.to_string())?
            .hash(&mut fingerprint);
        inputs.push(tokens);
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
            let roots = tokens_to_syntax_trees::parse(&mut syntax, SourceId(1), black_box(tokens))
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

//! Measure the compiler's complete loaded-source to syntax operation.
use source_files::{LoadedSource, LoadedSources};
use source_compiler::{SourceInput, CompileTimings};
use source_text::SourceId;
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
        .filter(|arg| arg != "--bench")
        .collect();
    let repeats: u64 = arguments
        .first()
        .ok_or("expected repeats and paths")?
        .parse()
        .map_err(|error| format!("{error}"))?;
    let mut bytes = 0u64;
    let sources: LoadedSources = arguments[1..]
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let source: Arc<str> = std::fs::read_to_string(path)
                .expect("benchmark source")
                .into();
            bytes += source.len() as u64;
            LoadedSource {
                source_id: SourceId(index),
                path: path.into(),
                source,
                ..Default::default()
            }
        })
        .collect();
    let mut fingerprint = std::collections::hash_map::DefaultHasher::new();
    let mut syntax = source_compiler::SourceSyntax::default();
    let parsed = SourceInput(sources.clone())
        .append(&mut syntax, &mut CompileTimings::default())
        .map_err(|error| format!("{error:?}"))?;
    syntax
        .record_bytes()
        .map_err(|error| error.to_string())?
        .hash(&mut fingerprint);
    format!("{parsed:?}").hash(&mut fingerprint);
    let start = Instant::now();
    let mut roots = 0usize;
    for _ in 0..black_box(repeats) {
        let mut syntax = source_compiler::SourceSyntax::default();
        let parsed = SourceInput(sources.clone())
            .append(&mut syntax, &mut CompileTimings::default())
            .map_err(|error| format!("{error:?}"))?;
        roots += black_box(syntax.root_item_count());
        black_box(parsed);
    }
    let elapsed = start.elapsed().as_nanos();
    let work = bytes.checked_mul(repeats).ok_or("work overflow")?;
    println!(
        "{{\"schema\":\"omega-timed-work/1\",\"elapsed_ns\":{elapsed},\"work_units\":{work},\"checksum\":\"{}:{roots}\"}}",
        fingerprint.finish()
    );
    Ok(())
}

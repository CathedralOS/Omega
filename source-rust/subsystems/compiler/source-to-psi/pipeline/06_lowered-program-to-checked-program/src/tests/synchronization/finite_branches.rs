//! Finite branch fan-out with independent atomic cells. Timings below start
//! after publication and cover the same synchronization check used at admission.
use super::super::build_purpose::{judge_build, judge_build_published};
use super::reject_deadlock;
use crate::RouteVerdict;
use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Instant;
use terminal_semantics::synchronization::{ProgressScope, private_wait_failure};

fn independent_cells(count: usize, gate: Option<u32>) -> String {
    let mut source = String::from(
        "data Cell { word:AtomicU32; }
         machine write(cell:@Cell, flag:bool) {
             transition flag { true -> set(cell), _ -> clear(cell) }
             machine set(cell:@Cell) { cell.word.store(1,Publish); }
             machine clear(cell:@Cell) { cell.word.store(0,Publish); }
         }",
    );
    if gate.is_some() {
        source.push_str(
            "machine await_clear(cell:@Cell) {
            let observed:u32=cell.word.load(Receive);
            transition observed == 1 { true -> await_clear(cell), _ -> done() }
            machine done() {}
        }",
        );
    }
    source.push_str("machine build_entry(");
    for index in 0..count {
        if index != 0 {
            source.push(',');
        }
        write!(source, "flag{index}:bool").unwrap();
    }
    source.push_str(") {");
    for index in 0..count {
        write!(
            source,
            "let mutable cell{index}:Cell; write(@cell{index},flag{index});
            let observed{index}:u32=cell{index}.word.load(Receive);"
        )
        .unwrap();
    }
    if let Some(value) = gate {
        write!(
            source,
            "let mutable gate:Cell; gate.word.store({value},Publish); await_clear(@gate);"
        )
        .unwrap();
    }
    source.push('}');
    source
}

#[test]
fn independent_branches_preserve_a_later_private_wait() {
    reject_deadlock(&independent_cells(16, Some(1)));
    for gate in [None, Some(0)] {
        assert!(matches!(
            judge_build(&independent_cells(16, gate)).unwrap(),
            RouteVerdict::Accepted(_)
        ));
    }
}

#[test]
#[ignore = "timing benchmark, run with --ignored --nocapture"]
fn finite_branch_timing() {
    for count in [8, 32, 128, 256] {
        for gate in [None, Some(0)] {
            let source = independent_cells(count, gate);
            let RouteVerdict::Accepted(accepted) =
                judge_build_published(&source, &[], terminal_codec::TerminalPublication::Portable)
                    .unwrap()
            else {
                panic!("benchmark source must pass");
            };
            let publication = accepted.publish().unwrap();
            let bytes = publication.artifact.to_bytes();
            let saved = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
            let module = saved.semantic_module();
            let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry else {
                panic!("expected checked machine entry");
            };
            let mut samples = [std::time::Duration::ZERO; 11];
            for sample in &mut samples {
                let started = Instant::now();
                let failure = private_wait_failure(
                    black_box(module),
                    entry,
                    ProgressScope::Executable,
                    |_| None,
                );
                *sample = started.elapsed();
                assert!(black_box(failure).is_none());
            }
            samples.sort_unstable();
            eprintln!(
                "branches={count} reachable_wait={} median={:?}",
                gate.is_some(),
                samples[samples.len() / 2]
            );
        }
    }
}

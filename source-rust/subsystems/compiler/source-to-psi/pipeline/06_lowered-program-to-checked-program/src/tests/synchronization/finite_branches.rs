//! Finite branch fan-out with independent atomic cells. Timings below start
//! after publication and cover the same synchronization check used at admission.
use super::super::build_purpose::{judge_build, judge_build_published};
use super::{expect_unproved_progress_at, reject_deadlock};
use crate::RouteVerdict;
use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Instant;
use terminal_semantics::synchronization::{ProgressScope, private_wait_failure};

#[derive(Clone, Copy, Debug)]
enum Branch {
    Call,
    Inline,
}

const CELL: &str = "data Cell { word:AtomicU32; }";
const WRITE: &str = "machine write(cell:@Cell, flag:bool) {
    transition flag { true -> set(cell), _ -> clear(cell) }
    machine set(cell:@Cell) { cell.word.store(1,Publish); }
    machine clear(cell:@Cell) { cell.word.store(0,Publish); }
}";
const AWAIT_CLEAR: &str = "machine await_clear(cell:@Cell) {
    let observed:u32=cell.word.load(Receive);
    transition observed == 1 { true -> await_clear(cell), _ -> done() }
    machine done() {}
}";

fn independent_cells(count: usize, gate: Option<u32>, branch: Branch) -> String {
    let mut source = String::from(CELL);
    if matches!(branch, Branch::Call) {
        source.push_str(WRITE);
    }
    let tail = gate.map(|value| {
        source.push_str(AWAIT_CLEAR);
        format!("let mutable gate:Cell; gate.word.store({value},Publish); await_clear(@gate);")
    });
    entry_with_branches(source, count, branch, "", tail.as_deref().unwrap_or(""))
}

fn entry_with_branches(
    mut source: String,
    count: usize,
    branch: Branch,
    before: &str,
    after: &str,
) -> String {
    source.push_str("machine build_entry(");
    for index in 0..count {
        if index != 0 {
            source.push(',');
        }
        write!(source, "flag{index}:bool").unwrap();
    }
    source.push_str(") {");
    source.push_str(before);
    for index in 0..count {
        write!(source, "let mutable cell{index}:Cell;").unwrap();
        match branch {
            Branch::Call => write!(source, "write(@cell{index},flag{index});"),
            Branch::Inline => write!(
                source,
                "let mark{index}:u32 = match flag{index} {{ true -> 1, false -> 0 }};
                 cell{index}.word.store(mark{index},Publish);"
            ),
        }
        .unwrap();
        write!(
            source,
            "let observed{index}:u32=cell{index}.word.load(Receive);"
        )
        .unwrap();
    }
    source.push_str(after);
    source.push('}');
    source
}

#[test]
fn independent_branches_preserve_a_later_private_wait() {
    reject_deadlock(&independent_cells(16, Some(1), Branch::Call));
    for gate in [None, Some(0)] {
        assert!(matches!(
            judge_build(&independent_cells(16, gate, Branch::Call)).unwrap(),
            RouteVerdict::Accepted(_)
        ));
    }
}

#[test]
fn inline_branches_preserve_earlier_borrows_and_separate_later_choices() {
    let program = |tail| {
        entry_with_branches(
            format!("{CELL} {AWAIT_CLEAR} data Choice<'s> {{ cell: @'s Cell; }}"),
            40,
            Branch::Inline,
            "let mutable left:Cell; let mutable right:Cell;
         left.word.store(1,Publish); right.word.store(1,Publish);
         let first:Choice = match flag0 {
             true -> Choice { cell:@left }, false -> Choice { cell:@right }
         };",
            tail,
        )
    };
    reject_deadlock(&program("await_clear(first.cell);"));
    let released = program("first.cell.word.store(0,Publish); await_clear(first.cell);");
    assert!(matches!(
        judge_build(&released).unwrap(),
        RouteVerdict::Accepted(_)
    ));

    let overlap = program(
        "let second:Choice = match flag39 {
             true -> Choice { cell:@left }, false -> Choice { cell:@right }
         }; second.cell.word.store(0,Publish); await_clear(first.cell);",
    );
    expect_unproved_progress_at(&overlap, "await_clear");
}

#[test]
#[ignore = "timing benchmark, run with --ignored --nocapture"]
fn finite_branch_timing() {
    for count in [8, 32, 128, 256] {
        for (gate, branch) in [
            (None, Branch::Call),
            (Some(0), Branch::Call),
            (None, Branch::Inline),
            (Some(0), Branch::Inline),
        ] {
            let source = independent_cells(count, gate, branch);
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
                "branches={count} shape={branch:?} reachable_wait={} median={:?}",
                gate.is_some(),
                samples[samples.len() / 2]
            );
        }
    }
}

//! The lowered-form checker can be shown a source program without running
//! the program's Build and without the old checker
//! (`CompilationSession::typed_frontier_without_build`, the harness behind
//! `examples/route_probe.rs`). Two standalone programs pin it: one the
//! checker accepts, and the same program with a machine nothing calls whose
//! addition can overflow, which it rejects because a standalone source is its
//! own validation subject.

use lowered_discharge::RouteVerdict;

// Compilation shares process-wide counters; run one at a time.
static COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Every construct here is in the checker's scalar subset: a guarded
/// transition, a direct call whose `requires` the caller establishes, and
/// exact arithmetic the `requires` keeps in range.
const ACCEPTED: &str = "\
machine scaled(value: u32, factor: u32) -> u32 requires value <= 1000 && factor <= 10 {
    value * factor + 1
}
machine settle(total: u32) {
    transition total > 9000 {
        true -> done()
        false -> done()
    }
}
machine done() { }
machine main() {
    let total: u32 = 20;
    let product: u32 = scaled(total, 3);
    settle(product)
}
";

/// `main` never calls `unchecked`, whose `value + 1` overflows for the
/// largest `u32`.
const UNCALLED_OVERFLOW: &str = "\
machine unchecked(value: u32) -> u32 {
    value + 1
}
machine main() { }
";

/// One word per judged part of `source`, staged as a standalone `main.omg`.
fn verdicts(name: &str, source: &str) -> Vec<&'static str> {
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root =
        std::env::temp_dir().join(format!("omega-route-probe-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&root).expect("staging directory");
    let main = root.join("main.omg");
    std::fs::write(&main, source).expect("staged source");
    let frontier = compiler::CompilationSession::default().typed_frontier_without_build(
        compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
        None,
    );
    let _ = std::fs::remove_dir_all(&root);
    let frontier = frontier.unwrap_or_else(|failure| {
        panic!(
            "{name}: Psi 00 through 04 must complete: {:?}",
            failure.into_diagnostics()
        )
    });
    assert_eq!(
        frontier.entries.len(),
        1,
        "{name}: `main` is the bound entry"
    );
    lowered_discharge::judge_compilation(
        &frontier.typed,
        structure_elaboration::Compilation {
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            selected: &frontier.selected,
            bindings: &frontier.bindings,
        },
    )
    .iter()
    .map(|verdict| match verdict {
        Ok(RouteVerdict::Accepted(_)) => "accepted",
        Ok(RouteVerdict::Rejected { .. }) => "rejected",
        Ok(RouteVerdict::Incomplete(_)) => "incomplete",
        Ok(RouteVerdict::FallBack(_)) => "not-supported",
        Err(_) => "defect",
    })
    .collect()
}

#[test]
fn a_standalone_program_is_judged_without_its_build() {
    assert_eq!(verdicts("accepted", ACCEPTED), ["accepted"]);
}

#[test]
fn an_uncalled_defect_of_a_standalone_program_is_rejected() {
    assert_eq!(
        verdicts("uncalled_overflow", UNCALLED_OVERFLOW),
        ["rejected"]
    );
}

//! Dispatch over an owned sum that is not `[copy]`. Read from source, as a
//! Build program: the toolchain's `Build::request_git` dispatches on the
//! `PackageSelection` it was given, and its `Root` arm has no payload to
//! name.
//!
//! The subject is the machine's to dispose of (ownership.md: "Affine |
//! Default owned data | Move at most once; discard through eligible
//! cleanup."), and an arm that never names it again leaves that to its
//! case's edge. control_flow.md: "Selected payload fields become scalar
//! parameters on that case's edge before cleanup." Only the selected case
//! holds anything (ownership.md: "An inactive case creates no obligation for
//! its absent payload."), so each arm owes exactly what its own case holds.

use super::build_purpose::judge_build;
use super::*;
use terminal_psi::{TerminalAffineCleanupAction, Terminator};
use terminal_semantics::plain_affine;

const DATA: &str = r#"
data Selection<'storage> {
    case Root;
    case Named(package: &'storage [u8]);
}
data Source<'storage> {
    case Path(location: &'storage [u8]);
    case Git(repository: &'storage [u8], selection: Selection<'storage>);
}
machine take(view: &[u8]) { }
machine pair(first: Selection, second: Selection) { }
"#;

/// `accepted`, the first thing a rejected program could not establish, or
/// the refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { unestablished, .. }) => match &unestablished[..] {
            [first, ..] => first.detail.to_owned(),
            [] => panic!("rejected for something other than a use of storage"),
        },
        other => panic!("{other:?}"),
    }
}

fn selecting(root: &str) -> String {
    format!(
        r#"{DATA}
machine build_entry(selection: Selection, view: &[u8]) {{ select(view, selection); }}
machine select(view: &[u8], selection: Selection) {{
    transition selection {{
        Selection::Root -> {root}
        Selection::Named {{ package }} -> take(package)
    }}
}}
"#
    )
}

/// An arm with nothing of the subject to name: the subject is discarded on
/// that case's edge. The other arm still reads its payload, so its edge
/// discards nothing and the subject lives until the arm returns.
#[test]
fn an_arm_that_never_names_the_subject_leaves_it_to_its_edge() {
    assert_eq!(verdict(&selecting("take(view)")), "accepted");
}

/// The toolchain's shape: one arm lends a view its case holds, the other
/// lends a view and moves the nested sum its case holds into the call. What
/// is moved out is moved once.
#[test]
fn an_arm_moves_the_owned_payload_its_case_holds() {
    let routing = |git: &str| {
        format!(
            r#"{DATA}
machine build_entry(source: Source) {{ route(source); }}
machine route(source: Source) {{
    transition source {{
        Source::Path {{ location }} -> take(location)
        Source::Git {{ repository, selection }} -> {git}
    }}
}}
machine select(view: &[u8], selection: Selection) {{
    transition selection {{
        Selection::Root -> take(view)
        Selection::Named {{ package }} -> take(package)
    }}
}}
"#
        )
    };
    assert_eq!(
        verdict(&routing("select(repository, selection)")),
        "accepted"
    );
    assert_eq!(
        verdict(&routing("pair(selection, selection)")),
        "owned storage is used after its value was transferred or consumed"
    );
}

/// The discard on a dispatch edge is the affine judgment's to hold to its
/// schedule: an edge that drops it, and an edge that disposes of storage
/// the schedule does not own, are each refused.
#[test]
fn a_dispatch_edge_disposes_of_exactly_what_the_schedule_says() {
    let Ok(RouteVerdict::Accepted(accepted)) = judge_build(&selecting("take(view)")) else {
        panic!("the program is accepted as lowered");
    };
    let module = &accepted.elaboration.module;
    let (machine, block) = module
        .machines
        .iter()
        .find_map(|machine| {
            let block = machine.blocks.iter().position(|block| {
                matches!(&block.terminator, Terminator::StructuralCase { cases, .. }
                    if cases.iter().any(|case| !case.cleanup_actions.is_empty()))
            })?;
            Some((machine, block))
        })
        .expect("a dispatch edge carries the subject's discard");
    let plan = plain_affine::verify(machine, &module.structural_types).unwrap();
    let Terminator::StructuralCase { cases, .. } = &machine.blocks[block].terminator else {
        unreachable!()
    };
    let discarding: Vec<_> = cases
        .iter()
        .map(|case| case.cleanup_actions.as_slice())
        .collect();
    let [[TerminalAffineCleanupAction::DiscardRoot(subject)], []] = discarding[..] else {
        panic!("`Root` discards the subject and `Named` keeps it: {discarding:?}");
    };
    assert!(plan.affine_roots.contains(*subject));

    let edit = |change: &dyn Fn(&mut Vec<TerminalAffineCleanupAction>)| {
        let mut machine = machine.clone();
        let Terminator::StructuralCase { cases, .. } = &mut machine.blocks[block].terminator else {
            unreachable!()
        };
        change(&mut cases[0].cleanup_actions);
        plain_affine::verify(&machine, &module.structural_types).map(|_| ())
    };
    assert!(matches!(
        edit(&|actions| actions.clear()),
        Err(plain_affine::Error::Malformed(detail))
            if detail == "normal edge affine disposal differs from its canonical schedule"
    ));
    let borrowed = machine
        .structural_places
        .iter()
        .map(|place| place.id)
        .find(|place| !plan.affine_roots.contains(*place))
        .expect("the view formal is storage the schedule does not own");
    for action in [
        TerminalAffineCleanupAction::DiscardRoot(borrowed),
        TerminalAffineCleanupAction::RetireHome(borrowed),
    ] {
        assert!(matches!(
            edit(&|actions| actions.push(action.clone())),
            Err(plain_affine::Error::Unsupported(detail))
                if detail == "disposal of storage the affine schedule does not own"
        ));
    }
}

#[test]
fn owned_copyable_storage_can_be_discarded_on_a_case_edge() {
    let program = format!(
        r#"{DATA}
data CopyHolder [copy] {{ value: u32; }}
machine build_entry(selection: Selection, spare: CopyHolder) {{
    transition selection {{
        Selection::Root -> done()
        Selection::Named {{ package }} -> take(package)
    }}
}}
machine done() {{ }}
"#
    );
    let Ok(RouteVerdict::Accepted(accepted)) = judge_build(&program) else {
        panic!("the dispatch may receive an owned copyable argument");
    };
    let mut elaboration = accepted.elaboration;
    let machine = &mut elaboration.module.machines[0];
    let spare = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.position == 1)
        .unwrap()
        .place;
    let actions = machine
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Terminator::StructuralCase { cases, .. } => cases
                .iter_mut()
                .find(|edge| !edge.cleanup_actions.is_empty())
                .map(|edge| &mut edge.cleanup_actions),
            _ => None,
        })
        .unwrap();
    actions.push(TerminalAffineCleanupAction::DiscardRoot(spare));
    let Ok(RouteVerdict::Accepted(accepted)) = judge(Ok(elaboration)) else {
        panic!("freely discardable owned storage retains its disposal authority");
    };
    accepted.publish_invocation().unwrap();
}

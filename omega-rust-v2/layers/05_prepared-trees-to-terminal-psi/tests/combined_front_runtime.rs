//! Independent integration evidence for the two package-only cases that
//! runtime-joined closes. It reads the emitted Psi directly, not through the
//! workers' Psi readers, on the exact original inputs and on one-edit controls:
//! the runtime contract without its `ensures` line, the exclusive views made
//! shared, and a caller that does not establish the debt it owes.
#[path = "support/source.rs"]
mod sources;
use omega_lowering::{BlockTerminator, CallArgument, CallRequirement, CallSite, CallTarget, LoweredPackage, OperationKind};

const GUARANTEED: &str = include_str!("fixtures/package_cases/runtime-callable-contract-guarantee/case/case.omg");
const EXCLUSIVE: &str =
    include_str!("fixtures/package_cases/exclusive-view-match-result/case/case.omg");

fn once(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "{from:?}");
    text.replace(from, to)
}

/// Every runtime call in `read`, as (contract index, requirements), from operations and terminal edges.
fn runtime_calls(package: &LoweredPackage) -> Vec<(usize, Vec<CallRequirement>)> {
    let read = package.machines.iter().find(|m| m.declaration.path == "read").expect("read lowers");
    let mut calls: Vec<_> = read
        .body
        .operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::Call { callee: CallTarget::Runtime { contract, .. }, requirements, .. } => Some((contract.index(), requirements.clone())),
            _ => None,
        })
        .collect();
    for block in &read.body.blocks {
        if let BlockTerminator::TerminalCall { callee: CallTarget::Runtime { contract, .. }, requirements, .. } = &block.terminator {
            calls.push((contract.index(), requirements.clone()));
        }
    }
    calls
}

#[test]
fn the_runtime_guarantee_case_publishes_the_guarantee_and_the_caller_owes_the_precondition() {
    let no_ensures = include_str!("fixtures/package_cases/runtime-callable-contract-guarantee/control/case.omg").to_string();
    let unestablished = once(GUARANTEED, "\nrequires offset < 256\n{", "\n{");
    let mut shapes = Vec::new();
    for text in [GUARANTEED.to_string(), no_ensures, unestablished] {
        let package = sources::lower(&[("case.omg", &text)]).unwrap();
        let [contract] = package.callable_contracts.as_slice() else { panic!("one runtime contract") };
        let [(index, requirements)] = runtime_calls(&package).try_into().unwrap_or_else(|c: Vec<_>| panic!("one runtime call, found {c:?}"));
        assert_eq!(index, 0);
        let [CallRequirement { site, clause: 0 }] = requirements.as_slice() else { panic!("the call owes the contract's one clause") };
        assert!(matches!(site, CallSite::Operation(_) | CallSite::Edge(_)));
        let caller_requires = package.machines.iter().find(|m| m.declaration.path == "read").unwrap().contract.requires.len();
        shapes.push((contract.requires.len(), contract.ensures.len(), caller_requires));
    }
    // The guarantee is published only when authored; the debt is owed whether or not
    // the caller establishes it. Lowering decides neither.
    assert_eq!(shapes, [(1, 1, 1), (1, 0, 1), (1, 1, 0)]);
}

#[test]
fn the_exclusive_match_case_moves_each_arriving_view_and_its_shared_twin_copies() {
    let arrivals = |text: &str| {
        let package = sources::lower(&[("case.omg", text)]).unwrap();
        let [pick] = package.machines.as_slice() else { panic!("one machine") };
        let mut out = Vec::new();
        for block in &pick.body.blocks {
            let edges: Vec<_> = match &block.terminator {
                BlockTerminator::Jump(edge) => vec![edge],
                BlockTerminator::Conditional { when_true, when_false, .. } => vec![when_true, when_false],
                _ => vec![],
            };
            for edge in edges {
                out.extend(edge.structural_arguments.iter().map(|a| matches!(a, CallArgument::MoveWhole(_))));
            }
        }
        out
    };
    let moved = arrivals(EXCLUSIVE);
    let copied = arrivals(include_str!("fixtures/package_cases/exclusive-view-match-result/control/case.omg"));
    assert!(!moved.is_empty() && moved.iter().all(|m| *m), "exclusive arrivals move: {moved:?}");
    assert_eq!(moved.len(), copied.len());
    assert!(copied.iter().all(|m| !*m), "shared arrivals copy: {copied:?}");
}

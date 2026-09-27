//! A top-level `transition <cond> { <guard> -> (<expr>) }` state body is the
//! machine's terminator: every arm returns a structural value, and a
//! runtime-indexed copy source (`self.nodes[position]`) mints a
//! `StructuralLeafCopy` whose path carries the `RuntimeIndex` segment plus
//! the bound obligation the terminal verifier replays.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_production::{
    TerminalProductionCustody, TerminalProductionRequest, TerminalProductionTimings,
};
use terminal_psi::{OperationKind, StructuralPathSegment};

const WITNESS: &str = r#"
    data Node [copy] { value: u64; }
    machine Node::empty() -> Node { Node { value: 0 } }
    data Nodes {
        node_count: u64;
        nodes: [Node; 4];
    }
    data Main {}
    machine Main::main() {}
    machine Nodes::get(&self, position: u64) -> Node {
        transition position < self.node_count && position < 4 {
            true -> (self.nodes[position])
            false -> (Node::empty())
            _ -> (Node::empty())
        }
    }
"#;

#[test]
fn a_transition_body_returns_a_runtime_indexed_copied_leaf() {
    let checked = crate::front_end::checked_program(WITNESS);
    let artifact =
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Nodes::get"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("the transition body plans its guarded structural return")
            .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the runtime-index copy verifies under its bound obligation");
    let copies = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::StructuralLeafCopy { path, .. } => Some(path),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 1, "one leaf copy serves the indexed arm");
    let runtime_indexes = copies[0]
        .iter()
        .filter(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
        .count();
    assert_eq!(
        runtime_indexes, 1,
        "the copy path carries the selector's runtime index"
    );
}

#[test]
fn a_transition_pair_with_wildcard_fallback_returns_the_selected_place() {
    // The two-arm spelling carries no exact `false` arm: the wildcard arm is
    // the same transition's continuation, which the guarded-exit planning
    // pairs with the guarded target directly.
    let checked = crate::front_end::checked_program(&WITNESS.replace(
        r#"transition position < self.node_count && position < 4 {
            true -> (self.nodes[position])
            false -> (Node::empty())
            _ -> (Node::empty())
        }"#,
        r#"transition position < self.node_count && position < 4 {
            true -> (self.nodes[position])
            _ -> (Node::empty())
        }"#,
    ));
    let artifact =
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Nodes::get"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("the guarded pair plans its structural return")
            .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the guarded pair verifies end to end");
}

#[test]
fn a_transition_arm_without_the_bound_is_refused_at_check() {
    // Dropping `position < 4` leaves the selector unproven against the
    // literal extent — the checker refuses the read before any plan exists.
    let source = WITNESS.replace(
        "position < self.node_count && position < 4",
        "position < self.node_count",
    );
    let diagnostics = crate::front_end::checked_program_result(&source)
        .expect_err("without a dominating bound the indexed copy is rejected");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove index `position` is within length 4")
        }),
        "the rejection names the unproven bound: {diagnostics:?}"
    );
}

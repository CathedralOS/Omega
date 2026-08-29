use omega_psi_to_abstract_operations::{
    ArtifactLoweringError, lower_artifact_sections,
    lower_artifact_sections_for_native_ranked_countdown,
};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{OperationKind, TerminalRankedGuard, Terminator};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root {}

    machine Root::countdown(token: Token, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(token, remaining - 1)
            _ -> done(token)
        }
        state done(token: Token) {}
    }
"#;

fn artifact(source: &str) -> (Vec<u8>, Vec<u8>, psi_terminal::TerminalModule) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse fixture");
    let resolved = lower_syntax_trees(&syntax).expect("resolve fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type fixture");
    let checked = lower_typed_trees(typed).expect("check fixture");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::countdown")
        .expect("lower ranked fixture");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode ranked semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode ranked proof");
    (semantic, proof, lowered.semantic_module)
}

#[test]
fn explicit_ranked_admission_retains_exact_fuel_graph_and_frontier_custody() {
    let (semantic, proof, module) = artifact(COUNTDOWN_SOURCE);
    let profile = psi_proof_admission::AdmissionProfile::default();

    assert!(matches!(
        lower_artifact_sections(&semantic, &proof, &profile),
        Err(ArtifactLoweringError::Verification(
            psi_terminal_verifier::VerificationError::Module(
                psi_terminal_verifier::ModuleError::NonExecutableRankedScc(machine)
            )
        )) if machine == module.entry
    ));

    let admitted = lower_artifact_sections_for_native_ranked_countdown(&semantic, &proof, &profile)
        .expect("the exact structural Unit u32 countdown has native custody");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    let ranked = machine.ranked_scc.as_ref().expect("ranked SCC");
    let [covered] = ranked.covered_cyclic_edges.as_slice() else {
        panic!("one covered backedge")
    };
    let graph = &admitted.countdown.graph;

    assert_eq!(admitted.plan.entry, module.entry);
    assert_eq!(
        admitted.plan.psi,
        admitted.countdown.fixed_fuel.terminal_psi()
    );
    assert_eq!(admitted.countdown.fixed_fuel.entry(), module.entry);
    assert_eq!(
        admitted.countdown.fixed_fuel.schedule(),
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity()
    );
    assert!(
        admitted
            .countdown
            .fixed_fuel
            .relevant_preconditions()
            .is_empty()
    );
    assert_eq!(
        admitted.countdown.fixed_fuel.ceiling_units(),
        25_769_803_775
    );
    assert_eq!(admitted.countdown.ranked_scc, *ranked);
    assert_eq!(graph.entry, machine.entry);
    assert_eq!(
        admitted
            .countdown
            .structural_frontiers
            .block_entry(ranked.header),
        admitted
            .countdown
            .structural_frontiers
            .edge_exit(covered.edge)
    );
    assert!(
        admitted
            .countdown
            .structural_frontiers
            .block_entry(ranked.header)
            .expect("ranked header frontier")
            .owned_places()
            .iter()
            .any(|owned| owned.place == machine.structural_parameters[0].place)
    );

    let entry = machine
        .blocks
        .iter()
        .find(|block| block.id == graph.entry)
        .expect("preheader");
    let Terminator::Jump {
        edge, arguments, ..
    } = &entry.terminator
    else {
        panic!("entry jump")
    };
    assert_eq!(*edge, graph.preheader_edge);
    assert!(arguments.contains(&graph.initial_value));

    let header = machine
        .blocks
        .iter()
        .find(|block| block.id == ranked.header)
        .expect("header");
    assert_eq!(header.operations[0].id, graph.zero_operation);
    assert_eq!(header.operations[1].id, graph.compare_operation);
    assert!(matches!(
        &header.operations[1].kind,
        OperationKind::IntegerLessThan { left, right }
            if *left == graph.zero_value && *right == ranked.rank_parameter
    ));
    let TerminalRankedGuard::UnsignedParameterPositive { condition, .. } = covered.guard;
    let Terminator::Conditional { when_false, .. } = &header.terminator else {
        panic!("header conditional")
    };
    assert_eq!(when_false.edge, graph.false_exit_edge);
    assert_eq!(when_false.target, graph.done_block);
    assert_eq!(
        header.operations[1].result.scalar().expect("condition").id,
        condition
    );

    let decrement = machine
        .blocks
        .iter()
        .find(|block| block.id == covered.source)
        .expect("decrement block");
    assert_eq!(decrement.operations[0].id, graph.one_operation);
    assert_eq!(decrement.operations[1].id, graph.subtract_operation);
    assert!(matches!(
        &decrement.operations[1].kind,
        OperationKind::ExactIntegerSubtract { right, obligation, .. }
            if *right == graph.one_value && *obligation == graph.subtract_obligation
    ));
    let done = machine
        .blocks
        .iter()
        .find(|block| block.id == graph.done_block)
        .expect("done block");
    assert!(matches!(
        done.terminator,
        Terminator::ReturnUnit { edge, .. } if edge == graph.return_edge
    ));
}

#[test]
fn explicit_ranked_admission_rejects_a_wider_rank_carrier() {
    let wider = COUNTDOWN_SOURCE.replace("remaining: u32", "remaining: u64");
    let (semantic, proof, module) = artifact(&wider);

    assert!(matches!(
        lower_artifact_sections_for_native_ranked_countdown(
            &semantic,
            &proof,
            &psi_proof_admission::AdmissionProfile::default(),
        ),
        Err(ArtifactLoweringError::Verification(
            psi_terminal_verifier::VerificationError::Module(
                psi_terminal_verifier::ModuleError::NonExecutableRankedScc(machine)
            )
        )) if machine == module.entry
    ));
}

#[test]
fn explicit_ranked_admission_rejects_an_extra_structural_token() {
    let extra_token = r#"
        data Token { value: i32; }
        data Root {}

        machine Root::countdown(first: Token, second: Token, remaining: u32)
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                true -> countdown(first, second, remaining - 1)
                _ -> done(first, second)
            }
            state done(first: Token, second: Token) {}
        }
    "#;

    let (semantic, proof, module) = artifact(extra_token);
    assert!(matches!(
        lower_artifact_sections_for_native_ranked_countdown(
            &semantic,
            &proof,
            &psi_proof_admission::AdmissionProfile::default(),
        ),
        Err(ArtifactLoweringError::Verification(
            psi_terminal_verifier::VerificationError::Module(
                psi_terminal_verifier::ModuleError::NonExecutableRankedScc(machine)
            )
        )) if machine == module.entry
    ));
}

#[test]
fn explicit_ranked_admission_requires_the_independently_checked_proof() {
    let (semantic, _, module) = artifact(COUNTDOWN_SOURCE);
    let empty_proof =
        psi_terminal_codec::encode_proof_bundle(&psi_terminal_verifier::ProofBundle::default())
            .expect("encode empty proof");

    assert!(matches!(
        lower_artifact_sections_for_native_ranked_countdown(
            &semantic,
            &empty_proof,
            &psi_proof_admission::AdmissionProfile::default(),
        ),
        Err(ArtifactLoweringError::Verification(
            psi_terminal_verifier::VerificationError::MissingEvidence(_)
        ))
    ));
    assert!(module.machines[0].ranked_scc.is_some());
}

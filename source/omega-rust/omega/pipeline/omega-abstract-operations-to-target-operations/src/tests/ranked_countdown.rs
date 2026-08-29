use super::*;
use crate::{LoweringError, lower_ranked_to_target_operations};
use omega_psi_to_abstract_operations::lower_artifact_sections_for_native_ranked_countdown;
use omega_target_operations::TargetOperation;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{TerminalAffineCleanupAction, TerminalRankedGuard};
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

fn ranked_abstract() -> omega_abstract_operations::RankedNativeAbstractOperationPlan {
    let tokens = Lexer::new(COUNTDOWN_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::countdown")
        .expect("lower terminal countdown");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("semantic");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("proof");
    lower_artifact_sections_for_native_ranked_countdown(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("admit native ranked countdown")
}

#[test]
fn ranked_countdown_target_lowering_preserves_exact_custody_and_abi() {
    let ranked = ranked_abstract();
    let graph = ranked.countdown.graph;
    let TerminalRankedGuard::UnsignedParameterPositive { edge: guard, .. } =
        ranked.countdown.ranked_scc.covered_cyclic_edges[0].guard;
    let backedge = ranked.countdown.ranked_scc.covered_cyclic_edges[0].edge;
    let expected = [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
    ];

    for (target, expected_register) in expected {
        let lowered = lower_ranked_to_target_operations(&ranked, target).expect("target lowering");
        assert_eq!(lowered.psi, ranked.plan.psi);
        assert_eq!(lowered.entry, ranked.plan.entry);
        assert_eq!(lowered.functions.len(), 1);
        assert_eq!(
            lowered.functions[0].provenance.operations,
            vec![
                graph.zero_operation,
                graph.compare_operation,
                graph.one_operation,
                graph.subtract_operation,
            ]
        );
        assert_eq!(
            lowered.functions[0].provenance.edges,
            vec![
                graph.preheader_edge,
                guard,
                graph.false_exit_edge,
                backedge,
                graph.return_edge,
            ]
        );
        let TargetOperation::RankedU32Countdown(countdown) = &lowered.functions[0].operation else {
            panic!("dedicated ranked carrier")
        };
        assert_eq!(countdown.custody, ranked.countdown);
        assert!(matches!(
            countdown.call_plan.parameters[0].locations.as_slice(),
            [omega_calling_conventions::ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 4,
            }] if *register == expected_register
        ));
        assert_eq!(countdown.call_plan.parameters.len(), 2);
        assert_eq!(countdown.structural_parameters.len(), 1);
        assert!(matches!(
            countdown.cleanup_actions.as_slice(),
            [TerminalAffineCleanupAction::DiscardRoot(place)]
                if *place == countdown.structural_parameters[0].place
        ));
        let component = &countdown.custody.ranked_scc;
        let covered = &component.covered_cyclic_edges[0];
        assert_eq!(
            countdown
                .custody
                .structural_frontiers
                .block_entry(component.header),
            countdown
                .custody
                .structural_frontiers
                .edge_exit(covered.edge)
        );
    }
}

#[test]
fn ranked_countdown_target_lowering_rejects_drifted_graph_identity() {
    let mut ranked = ranked_abstract();
    ranked.countdown.graph.false_exit_edge = ranked.countdown.graph.preheader_edge;
    assert!(matches!(
        lower_ranked_to_target_operations(&ranked, NativeTarget::linux_x64()),
        Err(LoweringError::InvalidRankedCountdown(machine)) if machine == ranked.plan.entry
    ));
}

#[test]
fn ranked_countdown_target_lowering_rejects_drifted_structural_custody() {
    let mut borrowed = ranked_abstract();
    borrowed.plan.functions[0].structural_parameters[0].access =
        psi_terminal::StructuralAccess::SharedBorrow;
    assert!(matches!(
        lower_ranked_to_target_operations(&borrowed, NativeTarget::linux_x64()),
        Err(LoweringError::InvalidRankedCountdown(machine)) if machine == borrowed.plan.entry
    ));

    let mut missing_cleanup = ranked_abstract();
    let Some(omega_abstract_operations::AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    }) = missing_cleanup.plan.functions[0].operations.last_mut()
    else {
        panic!("ranked fixture ends in its Unit return")
    };
    cleanup_actions.clear();
    assert!(matches!(
        lower_ranked_to_target_operations(&missing_cleanup, NativeTarget::linux_x64()),
        Err(LoweringError::InvalidRankedCountdown(machine))
            if machine == missing_cleanup.plan.entry
    ));
}

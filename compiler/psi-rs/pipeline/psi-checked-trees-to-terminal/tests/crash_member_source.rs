use psi_core::{Proposition, ScalarTerm, StructuralFieldId};
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{CrashPredicateTerm, CrashRouteGuard, OperationKind};
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use psi_terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalStructuralValue, interpret_terminal_artifact_with_effect_handler_measured,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Packet { should_abort: bool; }
    data Helper {}
    machine Helper::inspect(packet: Packet)
    crashes Abort
        packet.should_abort
    {}

    data Root {}
    machine Root::enter(packet: Packet)
    crashes Abort
        packet.should_abort
    {
        Helper::inspect(packet);
    }
"#;

#[test]
fn direct_boolean_member_crash_route_survives_source_call_codec_and_interpretation() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("direct Boolean member crash route lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 2);
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [root_route] = root.contract.crash_routes.as_slice() else {
        panic!("caller publishes one member-guarded route")
    };
    let [helper_route] = helper.contract.crash_routes.as_slice() else {
        panic!("callee publishes one member-guarded route")
    };
    assert!(matches!(
        root_route.alternatives.as_slice(),
        [CrashRouteGuard::Predicate(_)]
    ));
    assert!(matches!(
        helper_route.alternatives.as_slice(),
        [CrashRouteGuard::Predicate(_)]
    ));
    let call = root.blocks[0]
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::CallUnit {
                crash_continuations,
                ..
            } => Some(crash_continuations),
            _ => None,
        })
        .expect("caller emits the Unit call");
    assert_eq!(call, &root.contract.crash_routes);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs the exact member-root substitution");
    let bytes = encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );

    let packet = TerminalStructuralValue {
        opaque_identity: 7,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    assert_eq!(
        interpret_terminal_artifact_with_effect_handler_measured(
            &bytes,
            &encode_proof_bundle(&lowered.proof_bundle).expect("proof encode"),
            &AdmissionProfile::default(),
            &[],
            &[packet],
            &mut Accept,
        )
        .expect("member contracts do not reinterpret opaque aggregate runtime data")
        .into_value(),
        TerminalExecutionResult::Unit,
    );
}

#[test]
fn verifier_rejects_unknown_direct_boolean_member_identity() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let mut lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("direct Boolean member crash route lowers");
    let wrong_field = StructuralFieldId::new(u64::MAX).expect("nonzero field");
    let wrong_route = |root| {
        vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(root, wrong_field),
            ),
        ))]
    };
    let caller_root = lowered.semantic_module.machines[0].structural_parameters[0].place;
    lowered.semantic_module.machines[0].contract.crash_routes[0].alternatives =
        wrong_route(caller_root);
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut lowered.semantic_module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("root operation is the Unit call")
    };
    crash_continuations[0].alternatives = wrong_route(caller_root);
    let helper_root = lowered.semantic_module.machines[1].structural_parameters[0].place;
    lowered.semantic_module.machines[1].contract.crash_routes[0].alternatives =
        wrong_route(helper_root);

    let result = psi_terminal_verifier::validate_module(&lowered.semantic_module);
    assert!(
        matches!(
            result,
            Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
        ),
        "unexpected verification result: {result:?}"
    );
}

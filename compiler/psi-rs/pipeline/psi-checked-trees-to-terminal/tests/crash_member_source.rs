use psi_core::{Proposition, ScalarTerm, StructuralFieldId};
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    CrashPredicateTerm, CrashRouteGuard, OperationKind, StructuralFieldType, StructuralTypeShape,
};
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
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

const NESTED_SOURCE: &str = r#"
    data AbortState { should_abort: bool; }
    data Packet { state: AbortState; }
    data Helper {}
    machine Helper::inspect(packet: Packet)
    crashes Abort
        packet.state.should_abort
    {}

    data Root {}
    machine Root::enter(packet: Packet)
    crashes Abort
        packet.state.should_abort
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

#[test]
fn nested_boolean_member_path_survives_source_call_codec_verification_interpretation_and_fuel() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(NESTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("nested Boolean member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_predicate)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one nested member predicate")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            root: route_root,
            path,
        },
    ) = root_predicate.proposition()
    else {
        panic!("nested member route retains a structural Boolean path")
    };
    assert_eq!(*route_root, root.structural_parameters[0].place);
    let [outer_field, leaf_field] = path.as_slice() else {
        panic!("nested member route retains exactly two canonical field IDs")
    };
    let packet = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Packet type");
    let StructuralTypeShape::Record { fields } = &packet.shape else {
        panic!("Packet is a record")
    };
    let state = fields
        .iter()
        .find(|field| field.id == *outer_field)
        .expect("state field");
    assert_eq!(state.identity, "state");
    let StructuralFieldType::Structural(state_type) = state.field_type else {
        panic!("state field is structural")
    };
    let state = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == state_type)
        .expect("AbortState type");
    let StructuralTypeShape::Record { fields } = &state.shape else {
        panic!("AbortState is a record")
    };
    let leaf = fields
        .iter()
        .find(|field| field.id == *leaf_field)
        .expect("should_abort field");
    assert_eq!(leaf.identity, "should_abort");
    assert_eq!(
        leaf.field_type,
        StructuralFieldType::Scalar(psi_core::ScalarType::Boolean)
    );

    let [helper_route] = helper.contract.crash_routes.as_slice() else {
        panic!("callee publishes one nested member route")
    };
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(crash_continuations, &root.contract.crash_routes);
    assert_ne!(
        root.structural_parameters[0].place,
        helper.structural_parameters[0].place
    );
    assert_ne!(root.contract.crash_routes, [helper_route.clone()]);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier traverses and rebases the exact nested Boolean path");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("nested member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 9,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("nested member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 3);
}

#[test]
fn verifier_rejects_empty_truncated_and_mistyped_boolean_field_paths() {
    let tokens = Lexer::new(NESTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("nested Boolean member crash route lowers");
    let CrashRouteGuard::Predicate(predicate) =
        &lowered.semantic_module.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        panic!("nested route is a predicate")
    };
    let Proposition::Equal(_, ScalarTerm::BooleanField { path, .. }) = predicate.proposition()
    else {
        panic!("nested route is a Boolean field path")
    };
    let [outer, leaf] = path.as_slice() else {
        panic!("nested route has two fields")
    };

    for invalid_path in [
        Vec::new(),
        vec![*outer],
        vec![*leaf, *outer],
        vec![*outer, *leaf, *leaf],
    ] {
        let mut malformed = lowered.semantic_module.clone();
        let replace_path = |predicate: &mut CrashPredicateTerm| {
            let Proposition::Equal(_, ScalarTerm::BooleanField { root, .. }) =
                predicate.proposition()
            else {
                panic!("member route remains a Boolean field predicate")
            };
            let root = *root;
            *predicate = CrashPredicateTerm::new(Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field_path(root, invalid_path.clone()),
            ));
        };
        for machine in &mut malformed.machines {
            for route in &mut machine.contract.crash_routes {
                for alternative in &mut route.alternatives {
                    let CrashRouteGuard::Predicate(predicate) = alternative else {
                        continue;
                    };
                    replace_path(predicate);
                }
            }
            for operation in &mut machine.blocks[0].operations {
                let OperationKind::CallUnit {
                    crash_continuations,
                    ..
                } = &mut operation.kind
                else {
                    continue;
                };
                for route in crash_continuations {
                    for alternative in &mut route.alternatives {
                        let CrashRouteGuard::Predicate(predicate) = alternative else {
                            continue;
                        };
                        replace_path(predicate);
                    }
                }
            }
        }
        let result = psi_terminal_verifier::validate_module(&malformed);
        assert!(
            matches!(
                result,
                Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
            ),
            "unexpected validation result: {result:?}"
        );
    }
}

use super::{
    COMPOSED_MEMBER_SOURCE, INTEGER_MEMBER_SOURCE, NESTED_SOURCE, PROJECTED_SOURCE, SOURCE,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, Proposition, ScalarTerm,
    StructuralFieldId,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalStructuralValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{
    CrashPredicateTerm, CrashRouteGuard, OperationKind, StructuralFieldType, StructuralPathSegment,
    StructuralTypeShape,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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

    terminal_verifier::verify_module(
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
        interpret_terminal_artifact_measured(
            &bytes,
            &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
                .expect("proof encode"),
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &[packet],
                ..Default::default()
            },
            &mut Accept
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let mut lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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

    let result = terminal_verifier::validate_module(&lowered.semantic_module);
    assert!(
        matches!(
            result,
            Err(terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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
    let [
        CanonicalStructuralPathSegment::Field(outer_field),
        CanonicalStructuralPathSegment::Field(leaf_field),
    ] = path.as_slice()
    else {
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
        StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean)
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
    assert_ne!(
        root.contract.crash_routes.as_slice(),
        std::slice::from_ref(helper_route)
    );

    let verified = terminal_verifier::verify_module(
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
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 9,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
        &mut Accept,
    )
    .expect("nested member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 3);
}

#[test]
fn projected_structural_argument_prefix_rebases_member_crash_routes_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(PROJECTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("projected structural member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let packet_field = fields
        .iter()
        .find(|field| field.identity == "packet")
        .expect("packet field");

    let [CrashRouteGuard::Predicate(root_predicate)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one projected member predicate")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            root: route_root,
            path: caller_path,
        },
    ) = root_predicate.proposition()
    else {
        panic!("caller route retains its structural field path")
    };
    assert_eq!(*route_root, root.structural_parameters[0].place);
    assert_eq!(
        caller_path.first(),
        Some(&CanonicalStructuralPathSegment::Field(packet_field.id))
    );

    let [CrashRouteGuard::Predicate(helper_predicate)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one member predicate")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            path: helper_path, ..
        },
    ) = helper_predicate.proposition()
    else {
        panic!("callee route retains its parameter-relative field path")
    };
    assert_eq!(&caller_path[1..], helper_path);

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [StructuralPathSegment::Field("packet".into())]
    );
    assert_eq!(crash_continuations, &root.contract.crash_routes);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently composes the argument and callee field paths");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("projected member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 11,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
        &mut Accept,
    )
    .expect("projected member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 3);
}

#[test]
fn composed_boolean_member_predicate_rebases_every_path_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn paths(
        proposition: &Proposition,
    ) -> (
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
    ) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("composed member predicate is one conjunction")
        };
        let [equality, armed] = conjuncts.as_slice() else {
            panic!("conjunction retains equality then member assertion")
        };
        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::BooleanEqual { left, right }) =
            equality
        else {
            panic!("first conjunct retains Boolean equality")
        };
        let ScalarTerm::BooleanField {
            path: left_path, ..
        } = left.as_ref()
        else {
            panic!("equality left operand is a member path")
        };
        let ScalarTerm::BooleanNot { operand } = right.as_ref() else {
            panic!("equality right operand retains negation")
        };
        let ScalarTerm::BooleanField {
            path: right_path, ..
        } = operand.as_ref()
        else {
            panic!("negated operand is a member path")
        };
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::BooleanField {
                path: armed_path, ..
            },
        ) = armed
        else {
            panic!("second conjunct is the armed member assertion")
        };
        (left_path, right_path, armed_path)
    }

    let tokens = Lexer::new(COMPOSED_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("composed Boolean member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one composed member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one composed member route")
    };
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("callee composed route survives the projected call")
    };
    assert_eq!(continuation, root_route);

    let (root_left, root_right, root_armed) = paths(root_route.proposition());
    let (helper_left, helper_right, helper_armed) = paths(helper_route.proposition());
    assert_eq!(&root_left[1..], helper_left);
    assert_eq!(&root_right[1..], helper_right);
    assert_eq!(&root_armed[1..], helper_armed);
    assert_eq!(root_left[0], root_right[0]);
    assert_eq!(root_left[0], root_armed[0]);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently traverses every composed member path");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("composed member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 17,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
        &mut Accept,
    )
    .expect("composed member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let Proposition::Conjunction(conjuncts) = predicate.proposition() else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::BooleanField { path: armed, .. })) =
        conjuncts.iter().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::BooleanField { .. })
            )
        })
    else {
        unreachable!()
    };
    let armed = armed.clone();
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::BooleanEqual { left, .. })) =
        conjuncts.iter_mut().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::BooleanEqual { .. })
            )
        })
    else {
        unreachable!()
    };
    let ScalarTerm::BooleanField { path, .. } = left.as_mut() else {
        unreachable!()
    };
    *path = armed;
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected composed-member validation result: {invalid_result:?}"
    );
}

#[test]
fn integer_member_comparisons_rebase_and_validate_exact_leaf_types_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn paths(
        proposition: &Proposition,
    ) -> (
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        IntegerType,
    ) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("integer member route is one conjunction")
        };
        let [nonzero, ordered] = conjuncts.as_slice() else {
            panic!("integer route retains ordering then inequality")
        };
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = ordered
        else {
            panic!("first conjunct retains the ordered member comparison")
        };
        let ScalarTerm::IntegerField {
            path: limit_path,
            scalar_type: limit_type,
            ..
        } = left.as_ref()
        else {
            panic!("ordered left operand is the limit member")
        };
        let ScalarTerm::IntegerField {
            path: current_path,
            scalar_type: current_type,
            ..
        } = right.as_ref()
        else {
            panic!("ordered right operand is the current member")
        };
        assert_eq!(limit_type, scalar_type);
        assert_eq!(current_type, scalar_type);

        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::BooleanNot { operand }) =
            nonzero
        else {
            panic!("second conjunct retains integer inequality as negated equality")
        };
        let ScalarTerm::IntegerEqual { left, right, .. } = operand.as_ref() else {
            panic!("inequality retains one integer equality term")
        };
        let ScalarTerm::IntegerField {
            path: nonzero_path, ..
        } = left.as_ref()
        else {
            panic!("inequality left operand is the current member")
        };
        assert!(matches!(right.as_ref(), ScalarTerm::IntegerField { .. }));
        (limit_path, current_path, nonzero_path, *scalar_type)
    }

    let tokens = Lexer::new(INTEGER_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("integer member crash comparisons lower");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one integer member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one integer member route")
    };
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("integer member route survives the projected call")
    };
    assert_eq!(continuation, root_route);

    let (root_limit, root_current, root_nonzero, integer_type) = paths(root_route.proposition());
    let (helper_limit, helper_current, helper_nonzero, helper_type) =
        paths(helper_route.proposition());
    assert_eq!(
        integer_type,
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    );
    assert_eq!(integer_type, helper_type);
    assert_eq!(root_limit, helper_limit);
    assert_eq!(root_current, helper_current);
    assert_eq!(root_nonzero, helper_nonzero);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently checks every integer member path and leaf type");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("integer member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 19,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
        &mut Accept,
    )
    .expect("integer member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut mistyped = lowered.semantic_module.clone();
    let CrashRouteGuard::Predicate(predicate) =
        &mut mistyped.machines[1].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(
        _,
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        },
    )) = conjuncts.iter_mut().find(|conjunct| {
        matches!(
            conjunct,
            Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { .. })
        )
    })
    else {
        unreachable!()
    };
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    *scalar_type = u32_type;
    for operand in [left, right] {
        let ScalarTerm::IntegerField { scalar_type, .. } = operand.as_mut() else {
            unreachable!()
        };
        *scalar_type = u32_type;
    }
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = terminal_verifier::validate_module(&mistyped);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected integer-member validation result: {invalid_result:?}"
    );
}

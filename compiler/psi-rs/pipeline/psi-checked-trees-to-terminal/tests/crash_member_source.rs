use psi_core::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, Proposition, ScalarTerm,
    StructuralFieldId,
};
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    CrashPredicateTerm, CrashRouteGuard, OperationKind, StructuralFieldType, StructuralPathSegment,
    StructuralTypeShape,
};
use psi_terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
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

const PROJECTED_SOURCE: &str = r#"
    data AbortState { should_abort: bool; }
    data Packet { state: AbortState; }
    data Spare { value: u64; }
    data Envelope { packet: Packet; spare: Spare; }
    data Helper {}
    machine Helper::inspect(packet: Packet)
    crashes Abort
        packet.state.should_abort
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.packet.state.should_abort
    {
        Helper::inspect(envelope.packet);
    }
"#;

const FIXED_INDEX_SOURCE: &str = r#"
    boundary trait PortIo {}
    data Receipt [linear] { should_abort: bool; }

    boundary machine Receipt::settle(self)
    reaches PortIo
    ensures true;

    data Helper {}
    machine Helper::inspect(receipt: Receipt)
    reaches PortIo
    crashes Abort
        receipt.should_abort
    {
        Receipt::settle(receipt);
    }

    data Root {}
    machine Root::enter(receipts: [Receipt; 1])
    reaches PortIo
    crashes Abort
    {
        Helper::inspect(receipts[0]);
    }
"#;

const COMPOSED_MEMBER_SOURCE: &str = r#"
    data Flag { active: bool; }
    data Pair { left: Flag; right: Flag; armed: bool; }
    data Spare {}
    data Envelope { pair: Pair; spare: Spare; }

    data Helper {}
    machine Helper::inspect(pair: Pair)
    crashes Abort
        pair.left.active == !pair.right.active && pair.armed
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.pair.left.active == !envelope.pair.right.active && envelope.pair.armed
    {
        Helper::inspect(envelope.pair);
    }
"#;

const INTEGER_MEMBER_SOURCE: &str = r#"
    data Metrics { current: u64; limit: u64; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.limit <= metrics.current && metrics.current != metrics.limit
    {}

    data Root {}
    machine Root::enter(metrics: Metrics)
    crashes Abort
        metrics.limit <= metrics.current && metrics.current != metrics.limit
    {
        Helper::inspect(metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_SOURCE: &str = r#"
    data Metrics { current: u64; limit: u64; }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.limit <= metrics.current && metrics.current != metrics.limit
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.limit <= envelope.batch.metrics.current
            && envelope.batch.metrics.current != envelope.batch.metrics.limit
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const DISJUNCTIVE_MEMBER_SOURCE: &str = r#"
    data Flag { active: bool; }
    data Pair { left: Flag; right: Flag; decoy: Flag; }
    data Envelope { pair: Pair; spare: Pair; }
    data Helper {}
    machine Helper::inspect(pair: Pair)
    crashes Abort
        pair.left.active || !pair.right.active
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.pair.left.active || !envelope.pair.right.active
    {
        Helper::inspect(envelope.pair);
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
fn projected_structural_argument_prefix_rebases_member_crash_routes_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(PROJECTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
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

    let verified = psi_terminal_verifier::verify_module(
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
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 11,
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
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

    let verified = psi_terminal_verifier::verify_module(
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
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
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
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
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
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
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

    let verified = psi_terminal_verifier::verify_module(
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
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
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
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
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
    let invalid_result = psi_terminal_verifier::validate_module(&mistyped);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected integer-member validation result: {invalid_result:?}"
    );
}

#[test]
fn projected_argument_prefix_rebases_every_integer_member_path_end_to_end() {
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
        &[CanonicalStructuralPathSegment],
    ) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("projected integer member route is one conjunction")
        };
        let [inequality, ordered] = conjuncts.as_slice() else {
            panic!("projected integer route retains both comparisons")
        };
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual { left, right, .. },
        ) = ordered
        else {
            panic!("ordered comparison remains terminal")
        };
        let (
            ScalarTerm::IntegerField {
                path: ordered_left, ..
            },
            ScalarTerm::IntegerField {
                path: ordered_right,
                ..
            },
        ) = (left.as_ref(), right.as_ref())
        else {
            panic!("ordered operands remain integer member paths")
        };
        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::BooleanNot { operand }) =
            inequality
        else {
            panic!("inequality remains a negated equality")
        };
        let ScalarTerm::IntegerEqual { left, right, .. } = operand.as_ref() else {
            panic!("inequality retains its integer equality")
        };
        let (
            ScalarTerm::IntegerField {
                path: unequal_left, ..
            },
            ScalarTerm::IntegerField {
                path: unequal_right,
                ..
            },
        ) = (left.as_ref(), right.as_ref())
        else {
            panic!("inequality operands remain integer member paths")
        };
        (ordered_left, ordered_right, unequal_left, unequal_right)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected integer member crash route lowers");

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
    let batch = fields
        .iter()
        .find(|field| field.identity == "batch")
        .expect("batch field");
    let StructuralFieldType::Structural(batch_type) = batch.field_type else {
        panic!("batch has a structural type")
    };
    let batch_declaration = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == batch_type)
        .expect("Batch type");
    let StructuralTypeShape::Record {
        fields: batch_fields,
    } = &batch_declaration.shape
    else {
        panic!("Batch is a record")
    };
    let metrics = batch_fields
        .iter()
        .find(|field| field.identity == "metrics")
        .expect("metrics field");
    let shadow = batch_fields
        .iter()
        .find(|field| field.identity == "shadow")
        .expect("shadow field");

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [
            StructuralPathSegment::Field("batch".into()),
            StructuralPathSegment::Field("metrics".into())
        ]
    );
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
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains one guarded crash continuation")
    };
    assert_eq!(continuation, root_route);
    let (root_ordered_left, root_ordered_right, root_unequal_left, root_unequal_right) =
        paths(root_route.proposition());
    let (helper_ordered_left, helper_ordered_right, helper_unequal_left, helper_unequal_right) =
        paths(helper_route.proposition());
    for (caller_path, callee_path) in [
        root_ordered_left,
        root_ordered_right,
        root_unequal_left,
        root_unequal_right,
    ]
    .into_iter()
    .zip([
        helper_ordered_left,
        helper_ordered_right,
        helper_unequal_left,
        helper_unequal_right,
    ]) {
        assert_eq!(
            &caller_path[..2],
            [
                CanonicalStructuralPathSegment::Field(batch.id),
                CanonicalStructuralPathSegment::Field(metrics.id),
            ]
        );
        assert_eq!(&caller_path[2..], callee_path);
    }

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently composes every projected integer member path");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("projected integer route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 23,
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
    .expect("projected integer contract remains verified metadata at interpretation");
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
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. })) =
        conjuncts.iter_mut().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { .. })
            )
        })
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField { path, .. } = left.as_mut() else {
        unreachable!()
    };
    path[1] = CanonicalStructuralPathSegment::Field(shadow.id);
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected projected integer validation result: {invalid_result:?}"
    );
}

#[test]
fn proposition_disjunction_rebases_and_verifies_each_member_path_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn field_paths(proposition: &Proposition) -> Vec<&[CanonicalStructuralPathSegment]> {
        fn collect_term<'a>(
            term: &'a ScalarTerm,
            paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
        ) {
            match term {
                ScalarTerm::BooleanField { path, .. } => paths.push(path),
                ScalarTerm::BooleanNot { operand } => collect_term(operand, paths),
                _ => {}
            }
        }
        fn collect<'a>(
            proposition: &'a Proposition,
            paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
        ) {
            match proposition {
                Proposition::Equal(left, right) => {
                    collect_term(left, paths);
                    collect_term(right, paths);
                }
                Proposition::Disjunction(disjuncts) => {
                    for disjunct in disjuncts {
                        collect(disjunct, paths);
                    }
                }
                _ => {}
            }
        }
        let mut paths = Vec::new();
        collect(proposition, &mut paths);
        paths
    }

    let tokens = Lexer::new(DISJUNCTIVE_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("disjunctive projected member crash route lowers");

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
    let pair = fields
        .iter()
        .find(|field| field.identity == "pair")
        .expect("pair field");
    let StructuralFieldType::Structural(pair_type) = pair.field_type else {
        panic!("pair has a structural type")
    };
    let pair_declaration = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == pair_type)
        .expect("Pair type");
    let StructuralTypeShape::Record {
        fields: pair_fields,
    } = &pair_declaration.shape
    else {
        panic!("Pair is a record")
    };
    let decoy = pair_fields
        .iter()
        .find(|field| field.identity == "decoy")
        .expect("decoy field");

    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one disjunctive route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one disjunctive route")
    };
    let Proposition::Disjunction(root_disjuncts) = root_route.proposition() else {
        panic!("caller retains terminal proposition disjunction")
    };
    assert_eq!(root_disjuncts.len(), 2);
    let Proposition::Disjunction(helper_disjuncts) = helper_route.proposition() else {
        panic!("callee retains terminal proposition disjunction")
    };
    assert_eq!(helper_disjuncts.len(), 2);

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [StructuralPathSegment::Field("pair".into())]
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains one disjunctive continuation")
    };
    assert_eq!(continuation, root_route);
    let root_paths = field_paths(root_route.proposition());
    let helper_paths = field_paths(helper_route.proposition());
    assert_eq!(root_paths.len(), 2);
    assert_eq!(helper_paths.len(), 2);
    for root_path in root_paths {
        assert_eq!(
            root_path.first(),
            Some(&CanonicalStructuralPathSegment::Field(pair.id))
        );
        assert!(helper_paths.contains(&&root_path[1..]));
    }

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently reconstructs the disjunctive continuation");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("disjunctive route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 29,
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
    .expect("disjunctive member contract remains verified metadata at interpretation");
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
    let mut proposition = predicate.proposition().clone();
    let Proposition::Disjunction(disjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::BooleanField { path, .. })) =
        disjuncts.iter_mut().find(|disjunct| {
            matches!(
                disjunct,
                Proposition::Equal(_, ScalarTerm::BooleanField { .. })
            )
        })
    else {
        unreachable!()
    };
    path[1] = CanonicalStructuralPathSegment::Field(decoy.id);
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected disjunctive validation result: {invalid_result:?}"
    );
}

#[test]
fn fixed_index_argument_prefix_is_canonical_and_rebases_member_crash_routes_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(FIXED_INDEX_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("fixed-index structural member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
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
        [StructuralPathSegment::FixedIndex(0)]
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("callee member route survives the fixed-index call")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            root: continuation_root,
            path: continuation_path,
        },
    ) = continuation.proposition()
    else {
        panic!("continuation is a canonical structural Boolean path")
    };
    assert_eq!(*continuation_root, root.structural_parameters[0].place);
    let [
        CanonicalStructuralPathSegment::FixedIndex(0),
        CanonicalStructuralPathSegment::Field(leaf),
    ] = continuation_path.as_slice()
    else {
        panic!("fixed index precedes the callee-relative Boolean field")
    };
    let [CrashRouteGuard::Predicate(helper_predicate)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one Boolean member route")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            path: helper_path, ..
        },
    ) = helper_predicate.proposition()
    else {
        panic!("callee route retains its member")
    };
    assert_eq!(helper_path, &[CanonicalStructuralPathSegment::Field(*leaf)]);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently composes the fixed index and callee member");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("fixed-index member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 4);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 13,
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
    .expect("fixed-index member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut out_of_bounds = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut out_of_bounds.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let Proposition::Equal(_, ScalarTerm::BooleanField { root, path }) = predicate.proposition()
    else {
        unreachable!()
    };
    let root = *root;
    let mut path = path.clone();
    path[0] = CanonicalStructuralPathSegment::FixedIndex(1);
    *predicate = CrashPredicateTerm::new(Proposition::Equal(
        ScalarTerm::boolean(true),
        ScalarTerm::boolean_field_path(root, path),
    ));
    let invalid_result = psi_terminal_verifier::validate_module(&out_of_bounds);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected fixed-index validation result: {invalid_result:?}"
    );
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

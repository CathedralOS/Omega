use super::{
    DISJUNCTIVE_MEMBER_SOURCE, NEGATIVE_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE,
    POLICY_NEGATIVE_ONE_LITERAL_DIVISION_SOURCE, PROJECTED_INTEGER_MEMBER_ARITHMETIC_SOURCE,
    PROJECTED_INTEGER_MEMBER_BITWISE_SOURCE, PROJECTED_INTEGER_MEMBER_DIVISION_SOURCE,
    PROJECTED_INTEGER_MEMBER_EXACT_SHIFT_SOURCE, PROJECTED_INTEGER_MEMBER_MULTIPLICATION_SOURCE,
    PROJECTED_INTEGER_MEMBER_POLICY_ARITHMETIC_SOURCE,
    PROJECTED_INTEGER_MEMBER_POLICY_DIVISION_SOURCE, PROJECTED_INTEGER_MEMBER_SOURCE,
    PROJECTED_INTEGER_MEMBER_SUBTRACTION_SOURCE, PROJECTED_INTEGER_MEMBER_WRAPPING_SHIFT_SOURCE,
    PROJECTED_RUNTIME_DIVISOR_CALL_SOURCE, RUNTIME_DIVISOR_CALL_SOURCE,
    RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE, UNPROVEN_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE,
};
use crate::crash_member_source::bounded_inputs;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::{AdmissionProfile, EvidenceRoute, ProofRule};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, Proposition, ScalarTerm,
};
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

    let checked = crate::front_end::checked_program(PROJECTED_INTEGER_MEMBER_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
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

    let verified = terminal_verifier::verify_module(
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
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
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
    let invalid_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected projected integer validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_addition_rebases_every_operand_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn addition_fields(
        proposition: &Proposition,
    ) -> (&ScalarTerm, &ScalarTerm, &ScalarTerm, IntegerType) {
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = proposition
        else {
            panic!("member arithmetic route retains its ordered comparison")
        };
        let ScalarTerm::ExactIntegerAdd {
            scalar_type: addition_type,
            left: add_left,
            right: add_right,
        } = left.as_ref()
        else {
            panic!("comparison left operand retains exact addition")
        };
        assert_eq!(addition_type, scalar_type);
        (add_left, add_right, right, *scalar_type)
    }

    let checked = crate::front_end::checked_program(PROJECTED_INTEGER_MEMBER_ARITHMETIC_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("projected exact member addition lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one arithmetic member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one arithmetic member route")
    };

    let (root_current, root_delta, root_limit, root_type) =
        addition_fields(root_route.proposition());
    let (helper_current, helper_delta, helper_limit, helper_type) =
        addition_fields(helper_route.proposition());
    assert_eq!(
        root_type,
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    );
    assert_eq!(root_type, helper_type);
    for term in [root_current, root_delta, root_limit] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller arithmetic operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(*scalar_type, root_type);
    }
    for term in [helper_current, helper_delta, helper_limit] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("callee arithmetic operand is a typed member path")
        };
        assert_eq!(*field_root, helper.structural_parameters[0].place);
        assert_eq!(path.len(), 1);
        assert_eq!(*scalar_type, helper_type);
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the arithmetic member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes every exact-add member operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("arithmetic member route has an acyclic fixed-fuel certificate");
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
        opaque_identity: 41,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let used_fuel = bounded_inputs::execute(
        &lowered.semantic_module,
        &semantics,
        &proof,
        argument,
        [("current", 20), ("delta", 5), ("limit", 30)],
        &mut Accept,
    );
    assert_eq!(used_fuel, fixed.ceiling_units());

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
    let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. }) = &mut proposition
    else {
        unreachable!()
    };
    let ScalarTerm::ExactIntegerAdd { left, right, .. } = left.as_mut() else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: left_path, ..
    } = left.as_mut()
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: right_path, ..
    } = right.as_ref()
    else {
        unreachable!()
    };
    *left_path = right_path.clone();
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected arithmetic-member validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_subtraction_rebases_every_operand_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn subtraction_fields(
        proposition: &Proposition,
    ) -> (&ScalarTerm, &ScalarTerm, &ScalarTerm, IntegerType) {
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = proposition
        else {
            panic!("member subtraction route retains its ordered comparison")
        };
        let ScalarTerm::ExactIntegerSubtract {
            scalar_type: subtraction_type,
            left: minuend,
            right: subtrahend,
        } = right.as_ref()
        else {
            panic!("comparison right operand retains exact subtraction")
        };
        assert_eq!(subtraction_type, scalar_type);
        (left, minuend, subtrahend, *scalar_type)
    }

    let checked = crate::front_end::checked_program(PROJECTED_INTEGER_MEMBER_SUBTRACTION_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("projected exact member subtraction lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one subtraction member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one subtraction member route")
    };

    let (root_floor, root_current, root_delta, root_type) =
        subtraction_fields(root_route.proposition());
    let (helper_floor, helper_current, helper_delta, helper_type) =
        subtraction_fields(helper_route.proposition());
    assert_eq!(
        root_type,
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    );
    assert_eq!(root_type, helper_type);
    for term in [root_floor, root_current, root_delta] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller subtraction operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(*scalar_type, root_type);
    }
    for term in [helper_floor, helper_current, helper_delta] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("callee subtraction operand is a typed member path")
        };
        assert_eq!(*field_root, helper.structural_parameters[0].place);
        assert_eq!(path.len(), 1);
        assert_eq!(*scalar_type, helper_type);
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the subtraction member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes every exact-subtract member operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("subtraction member route has an acyclic fixed-fuel certificate");
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
        opaque_identity: 43,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let used_fuel = bounded_inputs::execute(
        &lowered.semantic_module,
        &semantics,
        &proof,
        argument,
        [("current", 150), ("delta", 25), ("floor", 100)],
        &mut Accept,
    );
    assert_eq!(used_fuel, fixed.ceiling_units());

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
    let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { right, .. }) = &mut proposition
    else {
        unreachable!()
    };
    let ScalarTerm::ExactIntegerSubtract { left, right, .. } = right.as_mut() else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: left_path, ..
    } = left.as_mut()
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: right_path, ..
    } = right.as_ref()
    else {
        unreachable!()
    };
    *left_path = right_path.clone();
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected subtraction-member validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_multiplication_rebases_every_operand_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn multiplication_fields(proposition: &Proposition) -> [&ScalarTerm; 3] {
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = proposition
        else {
            panic!("member multiplication route retains its ordered comparison")
        };
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type: multiplication_type,
            left: multiplicand,
            right: multiplier,
        } = left.as_ref()
        else {
            panic!("comparison left operand retains exact multiplication")
        };
        assert_eq!(multiplication_type, scalar_type);
        [multiplicand, multiplier, right]
    }

    let checked = crate::front_end::checked_program(PROJECTED_INTEGER_MEMBER_MULTIPLICATION_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("projected exact member multiplication lowers");

    let root = &lowered.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one multiplication member route")
    };
    for term in multiplication_fields(root_route.proposition()) {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller multiplication operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(
            *scalar_type,
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        );
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the multiplication member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes every exact-multiply member operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("multiplication member route has an acyclic fixed-fuel certificate");
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
        opaque_identity: 47,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let used_fuel = bounded_inputs::execute(
        &lowered.semantic_module,
        &semantics,
        &proof,
        argument,
        [("current", 3), ("factor", 4), ("limit", 20)],
        &mut Accept,
    );
    assert_eq!(used_fuel, fixed.ceiling_units());

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
    let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. }) = &mut proposition
    else {
        unreachable!()
    };
    let ScalarTerm::ExactIntegerMultiply { left, right, .. } = left.as_mut() else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: left_path, ..
    } = left.as_mut()
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: right_path, ..
    } = right.as_ref()
    else {
        unreachable!()
    };
    *left_path = right_path.clone();
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected multiplication-member validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_division_and_remainder_rebase_safe_literals_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn arithmetic_fields(proposition: &Proposition) -> [&ScalarTerm; 3] {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("division and remainder route is one conjunction")
        };
        let mut division = None;
        let mut remainder = None;
        let mut parity = None;
        for conjunct in conjuncts {
            let Proposition::Equal(ScalarTerm::Boolean(true), predicate) = conjunct else {
                continue;
            };
            match predicate {
                ScalarTerm::IntegerLessOrEqual { left, right, .. } => {
                    let ScalarTerm::ExactIntegerDivide {
                        left: dividend,
                        right: divisor,
                        ..
                    } = left.as_ref()
                    else {
                        continue;
                    };
                    assert!(matches!(
                        divisor.as_ref(),
                        ScalarTerm::Integer {
                            value: semantic_vocabulary::IntegerValue::Unsigned(2),
                            ..
                        }
                    ));
                    division = Some(dividend.as_ref());
                    assert!(matches!(right.as_ref(), ScalarTerm::IntegerField { .. }));
                }
                ScalarTerm::IntegerEqual { left, right, .. } => {
                    let ScalarTerm::ExactIntegerRemainder {
                        left: dividend,
                        right: divisor,
                        ..
                    } = left.as_ref()
                    else {
                        continue;
                    };
                    assert!(matches!(
                        divisor.as_ref(),
                        ScalarTerm::Integer {
                            value: semantic_vocabulary::IntegerValue::Unsigned(2),
                            ..
                        }
                    ));
                    remainder = Some(dividend.as_ref());
                    parity = Some(right.as_ref());
                }
                _ => {}
            }
        }
        [
            division.expect("division member"),
            remainder.expect("remainder member"),
            parity.expect("remainder comparison member"),
        ]
    }

    let checked = crate::front_end::checked_program(PROJECTED_INTEGER_MEMBER_DIVISION_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("projected exact member division and remainder lower");

    let root = &lowered.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one division/remainder member route")
    };
    for term in arithmetic_fields(root_route.proposition()) {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller division operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(
            *scalar_type,
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        );
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the division/remainder member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes division/remainder member operands");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("division/remainder member route has an acyclic fixed-fuel certificate");
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
        opaque_identity: 53,
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
    .expect("member division remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut unsafe_divisor = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Helper::inspect"),
    )
    .expect("standalone helper division lowers")
    .semantic_module;
    let CrashRouteGuard::Predicate(predicate) =
        &mut unsafe_divisor.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(ScalarTerm::ExactIntegerDivide {
        scalar_type, right, ..
    }) = conjuncts.iter_mut().find_map(|conjunct| {
        let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. }) = conjunct else {
            return None;
        };
        Some(left.as_mut())
    })
    else {
        unreachable!()
    };
    **right =
        ScalarTerm::integer(*scalar_type, semantic_vocabulary::IntegerValue::Unsigned(0)).unwrap();
    *predicate = CrashPredicateTerm::new(proposition);
    let unsafe_result = terminal_verifier::validate_module(&unsafe_divisor);
    assert!(
        matches!(
            unsafe_result,
            Err(terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
        ),
        "unexpected unsafe-divisor validation result: {unsafe_result:?}"
    );

    let checked = crate::front_end::checked_program(RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE);
    let runtime_divisor = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("a positive runtime-divisor requirement is explicit terminal safety evidence");
    let runtime_machine = &runtime_divisor.semantic_module.machines[0];
    assert_eq!(runtime_machine.contract.requires.len(), 1);
    assert!(matches!(
        &runtime_machine.contract.requires[0],
        Proposition::LessOrEqual(
            ScalarTerm::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(1),
                ..
            },
            ScalarTerm::IntegerField { .. }
        )
    ));
    terminal_verifier::verify_module(
        &runtime_divisor.semantic_module,
        &runtime_divisor.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently accepts the runtime-divisor requirement");
    let encoded = encode_module(&runtime_divisor.semantic_module)
        .expect("runtime-divisor semantic module encodes");
    assert_eq!(
        decode_module(&encoded),
        Ok(runtime_divisor.semantic_module.clone()),
        "the exact runtime safety requirement survives canonical encoding"
    );

    let mut missing_requirement = runtime_divisor.semantic_module.clone();
    missing_requirement.machines[0].contract.requires.clear();
    assert!(matches!(
        terminal_verifier::validate_module(&missing_requirement),
        Err(terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
    ));

    let mut redirected_requirement = runtime_divisor.semantic_module.clone();
    let StructuralTypeShape::Record { fields } = &redirected_requirement.structural_types[0].shape
    else {
        unreachable!()
    };
    let limit = fields[2].id;
    let Proposition::LessOrEqual(_, ScalarTerm::IntegerField { path, .. }) =
        &mut redirected_requirement.machines[0].contract.requires[0]
    else {
        unreachable!()
    };
    *path = vec![CanonicalStructuralPathSegment::Field(limit)];
    assert!(matches!(
        terminal_verifier::validate_module(&redirected_requirement),
        Err(terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
    ));

    let diagnostics =
        crate::front_end::checked_program_result(UNPROVEN_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE)
            .expect_err("an unproven runtime divisor must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("divisor must be proven nonzero")),
        "unexpected unproven-runtime-divisor diagnostics: {diagnostics:?}"
    );
}

#[test]
fn bitwise_member_terms_rebase_across_projected_calls_and_codecs() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn inspect_term<'a>(
        term: &'a ScalarTerm,
        bitwise_counts: &mut [usize; 4],
        paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
    ) {
        match term {
            ScalarTerm::IntegerField { path, .. } => paths.push(path),
            ScalarTerm::BooleanNot { operand } => inspect_term(operand, bitwise_counts, paths),
            ScalarTerm::IntegerBitwiseNot { operand, .. } => {
                bitwise_counts[3] += 1;
                inspect_term(operand, bitwise_counts, paths);
            }
            ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
                match term {
                    ScalarTerm::IntegerBitwiseAnd { .. } => bitwise_counts[0] += 1,
                    ScalarTerm::IntegerBitwiseOr { .. } => bitwise_counts[1] += 1,
                    ScalarTerm::IntegerBitwiseXor { .. } => bitwise_counts[2] += 1,
                    _ => unreachable!(),
                }
                inspect_term(left, bitwise_counts, paths);
                inspect_term(right, bitwise_counts, paths);
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. } => {
                inspect_term(left, bitwise_counts, paths);
                inspect_term(right, bitwise_counts, paths);
            }
            _ => {}
        }
    }

    fn inspect_proposition(
        proposition: &Proposition,
    ) -> ([usize; 4], Vec<&[CanonicalStructuralPathSegment]>) {
        fn inspect<'a>(
            proposition: &'a Proposition,
            bitwise_counts: &mut [usize; 4],
            paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
        ) {
            match proposition {
                Proposition::Equal(left, right)
                | Proposition::LessThan(left, right)
                | Proposition::LessOrEqual(left, right) => {
                    inspect_term(left, bitwise_counts, paths);
                    inspect_term(right, bitwise_counts, paths);
                }
                Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                    for proposition in propositions {
                        inspect(proposition, bitwise_counts, paths);
                    }
                }
                _ => {}
            }
        }

        let mut bitwise_counts = [0; 4];
        let mut paths = Vec::new();
        inspect(proposition, &mut bitwise_counts, &mut paths);
        (bitwise_counts, paths)
    }

    let checked = crate::front_end::checked_program(PROJECTED_INTEGER_MEMBER_BITWISE_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("projected bitwise member predicates lower");
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one bitwise route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one bitwise route")
    };
    let (root_counts, root_paths) = inspect_proposition(root_route.proposition());
    let (helper_counts, helper_paths) = inspect_proposition(helper_route.proposition());
    assert_eq!(root_counts, [1, 1, 1, 1]);
    assert_eq!(helper_counts, root_counts);
    assert_eq!(root_paths.len(), helper_paths.len());

    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let bits = fields
        .iter()
        .find(|field| field.identity == "bits")
        .expect("bits field");
    assert!(
        root_paths
            .iter()
            .all(|path| { path.first() == Some(&CanonicalStructuralPathSegment::Field(bits.id)) })
    );
    assert!(helper_paths.iter().all(|path| path.len() == 1));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected Unit call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "bits"
    ));
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one bitwise crash continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently rebases every nested bitwise member");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("bitwise member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("bitwise fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("bitwise semantics encode");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("bitwise proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 59,
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
    .expect("bitwise crash predicates remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        terminal_verifier::validate_module(&redirected),
        Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn total_policy_arithmetic_rebases_across_projected_calls_and_codecs() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn inspect_policy_terms(
        proposition: &Proposition,
    ) -> ([usize; 6], Vec<&[CanonicalStructuralPathSegment]>) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("policy arithmetic route is one conjunction")
        };
        let mut counts = [0; 6];
        let mut paths = Vec::new();
        for conjunct in conjuncts {
            let Proposition::Equal(
                ScalarTerm::Boolean(true),
                ScalarTerm::IntegerEqual { left, right, .. },
            ) = conjunct
            else {
                panic!("each policy arithmetic clause remains an integer equality")
            };
            let (index, operation_left, operation_right) = match left.as_ref() {
                ScalarTerm::WrappingIntegerAdd { left, right, .. } => (0, left, right),
                ScalarTerm::WrappingIntegerSubtract { left, right, .. } => (1, left, right),
                ScalarTerm::WrappingIntegerMultiply { left, right, .. } => (2, left, right),
                ScalarTerm::SaturatingIntegerAdd { left, right, .. } => (3, left, right),
                ScalarTerm::SaturatingIntegerSubtract { left, right, .. } => (4, left, right),
                ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => (5, left, right),
                _ => panic!("unexpected policy arithmetic term"),
            };
            counts[index] += 1;
            for term in [
                operation_left.as_ref(),
                operation_right.as_ref(),
                right.as_ref(),
            ] {
                let ScalarTerm::IntegerField { path, .. } = term else {
                    panic!("policy arithmetic operand remains a typed member path")
                };
                paths.push(path.as_slice());
            }
        }
        (counts, paths)
    }

    let checked =
        crate::front_end::checked_program(PROJECTED_INTEGER_MEMBER_POLICY_ARITHMETIC_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("projected wrapping and saturating member arithmetic lowers");
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one policy arithmetic route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one policy arithmetic route")
    };
    let (root_counts, root_paths) = inspect_policy_terms(root_route.proposition());
    let (helper_counts, helper_paths) = inspect_policy_terms(helper_route.proposition());
    assert_eq!(root_counts, [1; 6]);
    assert_eq!(helper_counts, root_counts);
    assert_eq!(root_paths.len(), 18);
    assert_eq!(helper_paths.len(), root_paths.len());

    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let values = fields
        .iter()
        .find(|field| field.identity == "values")
        .expect("values field");
    assert!(
        root_paths.iter().all(|path| {
            path.first() == Some(&CanonicalStructuralPathSegment::Field(values.id))
        })
    );
    assert!(helper_paths.iter().all(|path| path.len() == 1));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected Unit call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "values"
    ));
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one policy arithmetic continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently rebases every policy arithmetic operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("policy arithmetic route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("policy arithmetic fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("policy semantics encode");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("policy proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 67,
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
    .expect("policy arithmetic predicates remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        terminal_verifier::validate_module(&redirected),
        Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn wrapping_shifts_rebase_distinct_count_carriers_across_projected_calls() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn inspect_shift_terms(proposition: &Proposition) -> ([usize; 2], Vec<usize>) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("wrapping shift route is one conjunction")
        };
        let value_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let count_type = IntegerType::new(IntegerSign::Signed, 16).unwrap();
        let mut counts = [0; 2];
        let mut path_lengths = Vec::new();
        for conjunct in conjuncts {
            let Proposition::Equal(
                ScalarTerm::Boolean(true),
                ScalarTerm::IntegerEqual { left, right, .. },
            ) = conjunct
            else {
                panic!("each wrapping shift clause remains an integer equality")
            };
            let (index, value, count) = match left.as_ref() {
                ScalarTerm::WrappingIntegerShiftLeft {
                    value_type: actual_value,
                    count_type: actual_count,
                    value,
                    count,
                } => {
                    assert_eq!((*actual_value, *actual_count), (value_type, count_type));
                    (0, value, count)
                }
                ScalarTerm::WrappingIntegerShiftRight {
                    value_type: actual_value,
                    count_type: actual_count,
                    value,
                    count,
                } => {
                    assert_eq!((*actual_value, *actual_count), (value_type, count_type));
                    (1, value, count)
                }
                _ => panic!("unexpected wrapping shift term"),
            };
            counts[index] += 1;
            for term in [value.as_ref(), count.as_ref(), right.as_ref()] {
                let ScalarTerm::IntegerField { path, .. } = term else {
                    panic!("wrapping shift operand remains a typed member path")
                };
                path_lengths.push(path.len());
            }
        }
        (counts, path_lengths)
    }

    let checked = crate::front_end::checked_program(PROJECTED_INTEGER_MEMBER_WRAPPING_SHIFT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("projected wrapping shifts lower without count requirements");
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    assert!(root.contract.requires.is_empty());
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one wrapping shift route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one wrapping shift route")
    };
    let (root_counts, root_path_lengths) = inspect_shift_terms(root_route.proposition());
    let (helper_counts, helper_path_lengths) = inspect_shift_terms(helper_route.proposition());
    assert_eq!(root_counts, [1; 2]);
    assert_eq!(helper_counts, root_counts);
    assert!(root_path_lengths.iter().all(|length| *length == 2));
    assert!(helper_path_lengths.iter().all(|length| *length == 1));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected wrapping shift call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "values"
    ));
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one wrapping shift continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently rebases wrapping shift value and count paths");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("wrapping shift route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("wrapping shift fixed fuel recomputes");
    let semantics =
        encode_module(&lowered.semantic_module).expect("wrapping shift semantics encode");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("wrapping shift proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 73,
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
    .expect("wrapping shifts remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut forged_exact = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Helper::inspect"),
    )
    .expect("standalone wrapping shift helper lowers")
    .semantic_module;
    let CrashRouteGuard::Predicate(predicate) =
        &mut forged_exact.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(ScalarTerm::WrappingIntegerShiftLeft {
        value_type,
        count_type,
        value,
        count,
    }) = conjuncts.iter_mut().find_map(|conjunct| {
        let Proposition::Equal(_, ScalarTerm::IntegerEqual { left, .. }) = conjunct else {
            return None;
        };
        matches!(left.as_ref(), ScalarTerm::WrappingIntegerShiftLeft { .. })
            .then_some(left.as_mut())
    })
    else {
        unreachable!()
    };
    let exact = ScalarTerm::ExactIntegerShiftLeft {
        value_type: *value_type,
        count_type: *count_type,
        value: value.clone(),
        count: count.clone(),
    };
    **conjuncts
        .iter_mut()
        .find_map(|conjunct| {
            let Proposition::Equal(_, ScalarTerm::IntegerEqual { left, .. }) = conjunct else {
                return None;
            };
            matches!(left.as_ref(), ScalarTerm::WrappingIntegerShiftLeft { .. }).then_some(left)
        })
        .expect("wrapping shift term") = exact;
    *predicate = CrashPredicateTerm::new(proposition);
    assert!(matches!(
        terminal_verifier::validate_module(&forged_exact),
        Err(terminal_verifier::ModuleError::UnsafeStructuralCrashExactShift { .. })
    ));

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        terminal_verifier::validate_module(&redirected),
        Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn exact_shifts_rebase_complete_count_and_overflow_requirements() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let checked = crate::front_end::checked_program(PROJECTED_INTEGER_MEMBER_EXACT_SHIFT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("projected Exact shifts retain complete safety requirements");
    let root = &lowered.semantic_module.machines[0];
    assert_eq!(root.contract.requires.len(), 3);
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one Exact shift route")
    };
    let Proposition::Conjunction(conjuncts) = root_route.proposition() else {
        panic!("Exact shift route is one conjunction")
    };
    let mut counts = [0; 2];
    for conjunct in conjuncts {
        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::IntegerEqual { left, .. }) =
            conjunct
        else {
            panic!("each Exact shift clause remains an integer equality")
        };
        match left.as_ref() {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                ..
            } => {
                assert_eq!(value_type.bits(), 8);
                assert_eq!(count_type.bits(), 16);
                counts[0] += 1;
            }
            ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                ..
            } => {
                assert_eq!(value_type.bits(), 8);
                assert_eq!(count_type.bits(), 16);
                counts[1] += 1;
            }
            _ => panic!("unexpected Exact shift term"),
        }
    }
    assert_eq!(counts, [1; 2]);

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        requirement_obligations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected Exact shift call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "values"
    ));
    assert_eq!(requirement_obligations.len(), 3);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one Exact shift continuation")
    };
    assert_eq!(continuation, root_route);
    let reconstructed =
        terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("verifier reconstructs projected Exact shift requirements");
    assert_eq!(reconstructed.len(), 3);
    assert!(reconstructed.iter().all(|item| {
        root.contract
            .requires
            .contains(&item.obligation.proposition)
    }));

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently checks Exact shift count and overflow bounds");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("Exact shift route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("Exact shift fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("Exact shift semantics encode");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("Exact shift proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 79,
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
    .expect("Exact shifts remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut missing_requirements = lowered.semantic_module.clone();
    missing_requirements.machines[0].contract.requires.clear();
    assert!(matches!(
        terminal_verifier::validate_module(&missing_requirements),
        Err(terminal_verifier::ModuleError::UnsafeStructuralCrashExactShift { .. })
    ));

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        terminal_verifier::validate_module(&redirected),
        Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn policy_division_rebases_nonzero_requirements_across_projected_calls() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let checked =
        crate::front_end::checked_program(PROJECTED_INTEGER_MEMBER_POLICY_DIVISION_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("projected policy division retains exact nonzero requirements");
    let root = &lowered.semantic_module.machines[0];
    assert_eq!(root.contract.requires.len(), 2);
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one policy division route")
    };
    let Proposition::Conjunction(conjuncts) = root_route.proposition() else {
        panic!("policy division route is one conjunction")
    };
    let mut counts = [0; 4];
    for conjunct in conjuncts {
        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::IntegerEqual { left, .. }) =
            conjunct
        else {
            panic!("each policy division clause remains an integer equality")
        };
        match left.as_ref() {
            ScalarTerm::WrappingIntegerDivide { .. } => counts[0] += 1,
            ScalarTerm::WrappingIntegerRemainder { .. } => counts[1] += 1,
            ScalarTerm::SaturatingIntegerDivide { .. } => counts[2] += 1,
            ScalarTerm::SaturatingIntegerRemainder { .. } => counts[3] += 1,
            _ => panic!("unexpected policy division term"),
        }
    }
    assert_eq!(counts, [1; 4]);

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        requirement_obligations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected policy division call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "values"
    ));
    assert_eq!(requirement_obligations.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one policy division continuation")
    };
    assert_eq!(continuation, root_route);
    let reconstructed =
        terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("verifier reconstructs projected policy divisor requirements");
    assert_eq!(reconstructed.len(), 2);
    assert!(reconstructed.iter().all(|item| {
        root.contract
            .requires
            .contains(&item.obligation.proposition)
    }));

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts both independently safe policy divisors");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("policy division route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("policy division fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("policy division encodes");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("policy proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 71,
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
    .expect("policy division predicates remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut missing_requirement = lowered.semantic_module.clone();
    missing_requirement.machines[0].contract.requires.clear();
    assert!(matches!(
        terminal_verifier::validate_module(&missing_requirement),
        Err(terminal_verifier::ModuleError::UnsafeStructuralCrashPolicyDivisor { .. })
    ));

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        terminal_verifier::validate_module(&redirected),
        Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn wrapping_negative_one_literal_divisor_is_self_proving() {
    let checked = crate::front_end::checked_program(POLICY_NEGATIVE_ONE_LITERAL_DIVISION_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("Wrapping defines signed MIN divided or remaindered by negative one");
    assert!(
        lowered.semantic_module.machines[0]
            .contract
            .requires
            .is_empty()
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("negative one is independently nonzero under Wrapping");

    let mut zero = lowered.semantic_module.clone();
    let CrashRouteGuard::Predicate(predicate) =
        &mut zero.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(ScalarTerm::WrappingIntegerDivide {
        scalar_type, right, ..
    }) = conjuncts.iter_mut().find_map(|conjunct| {
        let Proposition::Equal(_, ScalarTerm::IntegerEqual { left, .. }) = conjunct else {
            return None;
        };
        matches!(left.as_ref(), ScalarTerm::WrappingIntegerDivide { .. }).then_some(left.as_mut())
    })
    else {
        unreachable!()
    };
    **right =
        ScalarTerm::integer(*scalar_type, semantic_vocabulary::IntegerValue::Signed(0)).unwrap();
    *predicate = CrashPredicateTerm::new(proposition);
    assert!(matches!(
        terminal_verifier::validate_module(&zero),
        Err(terminal_verifier::ModuleError::UnsafeStructuralCrashPolicyDivisor { .. })
    ));
}

#[test]
fn signed_runtime_member_divisor_requires_an_overflow_safe_bound() {
    let checked = crate::front_end::checked_program(NEGATIVE_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("a divisor bounded at or below negative two is total for every dividend");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently accepts the negative runtime-divisor bound");

    let mut overflow_permitting = lowered.semantic_module.clone();
    let Proposition::LessOrEqual(_, ScalarTerm::Integer { value, .. }) =
        &mut overflow_permitting.machines[0].contract.requires[0]
    else {
        unreachable!()
    };
    *value = semantic_vocabulary::IntegerValue::Signed(-1);
    assert!(matches!(
        terminal_verifier::validate_module(&overflow_permitting),
        Err(terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
    ));
}

#[test]
fn runtime_divisor_call_requirements_rebase_and_verify_exact_obligations() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let checked = crate::front_end::checked_program(RUNTIME_DIVISOR_CALL_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("a whole-root Unit call carries its exact runtime-divisor requirement");
    let root = &lowered.semantic_module.machines[0];
    let OperationKind::CallUnit {
        requirement_obligations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("root emits one structural Unit call")
    };
    let [obligation] = requirement_obligations.as_slice() else {
        panic!("the call owns one exact requirement obligation")
    };
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    assert_eq!(lowered.proof_bundle.evidence[0].obligation, *obligation);
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently rebases and proves the call requirement");
    assert_eq!(verified.accepted_facts().len(), 1);

    let semantics = encode_module(&lowered.semantic_module).expect("call semantics encode");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("call proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 61,
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
    .expect("verified runtime-divisor call executes as erased proof metadata");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);

    let mut missing = lowered.proof_bundle.clone();
    missing.evidence.clear();
    assert!(matches!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &missing,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(id)) if id == *obligation
    ));

    let mut wrong_assumption = lowered.proof_bundle.clone();
    let EvidenceRoute::CertificateDerived(certificate) = &mut wrong_assumption.evidence[0].route
    else {
        unreachable!()
    };
    certificate.proof.rule = ProofRule::Assumption { index: 1 };
    assert!(matches!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &wrong_assumption,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence { .. })
    ));
}

#[test]
fn projected_runtime_divisor_call_rebases_requirement_through_canonical_prefix() {
    let checked = crate::front_end::checked_program(PROJECTED_RUNTIME_DIVISOR_CALL_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("a projected Unit call rebases its runtime-divisor requirement");
    let root = &lowered.semantic_module.machines[0];
    let OperationKind::CallUnit {
        structural_arguments,
        requirement_obligations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("root emits one projected structural Unit call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "metrics"
    ));
    assert_eq!(requirement_obligations.len(), 1);
    let reconstructed =
        terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("the verifier reconstructs the projected call obligation");
    assert_eq!(reconstructed.len(), 1);
    assert_eq!(
        reconstructed[0].obligation.proposition, root.contract.requires[0],
        "the canonical argument prefix rebases the callee premise to the caller path"
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the projected call proof cites the exact rebased caller assumption");

    let semantics = encode_module(&lowered.semantic_module).expect("projected call encodes");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("projected proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );

    let mut wrong_prefix = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut wrong_prefix.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("decoy".to_owned())];
    assert!(matches!(
        terminal_verifier::validate_module(&wrong_prefix),
        Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
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

    let checked = crate::front_end::checked_program(DISJUNCTIVE_MEMBER_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
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

    let verified = terminal_verifier::verify_module(
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
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
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
    let invalid_result = terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected disjunctive validation result: {invalid_result:?}"
    );
}

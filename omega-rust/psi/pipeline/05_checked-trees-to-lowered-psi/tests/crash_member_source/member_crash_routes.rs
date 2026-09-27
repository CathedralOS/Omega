use super::COMPOSED_MEMBER_SOURCE;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{CanonicalStructuralPathSegment, Proposition, ScalarTerm};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalStructuralValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{CrashPredicateTerm, CrashRouteGuard, OperationKind};

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

    let checked = crate::front_end::checked_program(COMPOSED_MEMBER_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
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

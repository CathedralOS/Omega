use checked_trees::{
    CheckedBooleanExpression, CheckedScalarComputationKind, CheckedScalarExpression,
    CheckedStructuralAccess, CheckedStructuralPredicatePathSegment,
    CheckedUnitStructuralArgumentSourcePlan,
};
use terminal_psi::{OperationKind, StructuralAccess};

use super::{LIMITS, ORDERED, support};

#[test]
fn ordinary_limits_cannot_be_reclassified_as_copyable_in_checked_signature() {
    let (original, _, _, _) = support::publish(LIMITS, "enter");
    for owner in ["inspect", "enter"] {
        let symbol = original
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == owner)
            .unwrap()
            .symbol;
        let mut changed = original.clone();
        let graph = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == symbol)
            .unwrap();
        let parameter = &mut graph.states[0].structural_parameters[0];
        assert_eq!(
            parameter.multiplicity,
            language_semantics::Multiplicity::Affine
        );
        parameter.multiplicity = language_semantics::Multiplicity::Unrestricted;
        support::reject(
            &changed,
            &format!("{owner}: ordinary Limits relabeled copyable"),
        );
    }
}

#[test]
fn source_signature_rejects_erased_reordered_or_retyped_owned_formals() {
    let (original, _, _, _) = support::publish(ORDERED, "enter");
    for owner in ["inspect", "enter"] {
        let symbol = original
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == owner)
            .unwrap()
            .symbol;
        for mutation in [
            "missing owned formal",
            "swapped owned positions",
            "swapped scalar positions",
            "changed access",
            "changed type",
        ] {
            let mut changed = original.clone();
            let graph = changed
                .facts
                .flow
                .terminal_scalar_graphs
                .machines
                .iter_mut()
                .find(|graph| graph.machine == symbol)
                .unwrap();
            let state = &mut graph.states[0];
            assert_eq!(state.structural_parameters.len(), 2);
            match mutation {
                "missing owned formal" => {
                    state.structural_parameters.remove(0);
                }
                "swapped owned positions" => state.structural_parameters.swap(0, 1),
                "swapped scalar positions" => state.scalar_parameters.swap(0, 1),
                "changed access" => {
                    state.structural_parameters[0].access = CheckedStructuralAccess::SharedBorrow
                }
                "changed type" => state.structural_parameters[0].type_identity = "bool".into(),
                _ => unreachable!(),
            }
            support::reject(&changed, &format!("{owner}: {mutation}"));
        }
    }
}

#[test]
fn source_owned_actuals_reject_same_typed_substitution_access_and_type_drift() {
    let (original, _, _, _) = support::publish(ORDERED, "enter");
    let owned = original
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .iter()
        .filter_map(|(handle, argument)| {
            (argument.access == CheckedStructuralAccess::Owned).then_some(handle)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        owned.len(),
        4,
        "choose and inspect each receive two owned actuals"
    );
    // Mutate one occurrence at a time: the other same-typed actual remains valid.
    for handle in owned {
        for mutation in [
            "other owned parameter",
            "borrow instead of own",
            "other type",
        ] {
            let mut changed = original.clone();
            let argument = changed
                .facts
                .values
                .scalar_computations
                .structural_arguments
                .get_mut(handle);
            match mutation {
                "other owned parameter" => {
                    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
                        &mut argument.source
                    else {
                        panic!("whole owned input");
                    };
                    assert!(*parameter_index < 2);
                    *parameter_index = 1 - *parameter_index;
                }
                "borrow instead of own" => argument.access = CheckedStructuralAccess::SharedBorrow,
                "other type" => argument.type_identity = "bool".into(),
                _ => unreachable!(),
            }
            support::reject(&changed, mutation);
        }
    }
}

#[test]
fn source_field_observation_rejects_another_same_typed_field_or_parameter() {
    let (original, _, _, _) = support::publish(ORDERED, "enter");
    let observations = original
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(handle, node)| match &node.kind {
            CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(expression))
                if matches!(
                    expression.as_ref(),
                    CheckedBooleanExpression::StructuralParameterField { .. }
                ) =>
            {
                Some(handle)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observations.len(),
        2,
        "nested stamp arguments retain both authored field reads"
    );
    for handle in observations {
        for mutation in [
            "other same-typed field",
            "other same-typed parameter",
            "missing occurrence",
        ] {
            let mut changed = original.clone();
            let node = changed
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(handle);
            let CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(expression)) =
                &mut node.kind
            else {
                panic!("direct field observation");
            };
            let CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            } = expression.as_mut()
            else {
                panic!("direct Boolean field observation");
            };
            match mutation {
                "other same-typed field" => {
                    *path = vec![CheckedStructuralPredicatePathSegment::Field("spare".into())]
                }
                "other same-typed parameter" => {
                    assert!([1, 3].contains(parameter_position));
                    *parameter_position = 4 - *parameter_position;
                }
                "missing occurrence" => node.value_source = Default::default(),
                _ => unreachable!(),
            }
            support::reject(&changed, mutation);
        }
    }
}

#[test]
fn selected_owned_graph_requires_its_structural_type_catalog() {
    let (mut checked, _, _, _) = support::publish(LIMITS, "enter");
    let types = &mut checked.facts.flow.terminal_scalar_graphs.structural_types;
    assert!(
        !types.is_empty(),
        "selected owned graph retains its type custody"
    );
    types.clear();
    support::reject(&checked, "missing owned graph type catalog");
}

#[test]
fn independent_verifier_rejects_published_owned_signature_field_and_call_corruption() {
    let (_, original, _, proof_bytes) = support::publish(LIMITS, "enter");
    let proof = terminal_codec::decode_proof_bundle(&proof_bytes).unwrap();
    for mutation in [
        "missing signature",
        "write-only field read",
        "unknown field",
        "field result type",
        "missing actual",
        "argument access",
        "missing type",
        "missing affine disposal",
        "caller disposal after transfer",
    ] {
        let mut changed = original.clone();
        match mutation {
            "missing affine disposal" => {
                let owner = changed.machines.iter_mut().find(|machine| machine.blocks.iter().any(|block| matches!(&block.terminator, terminal_psi::Terminator::Return { cleanup_actions, .. } if !cleanup_actions.is_empty()))).expect("inspect discards its affine input");
                for block in &mut owner.blocks {
                    if let terminal_psi::Terminator::Return {
                        cleanup_actions, ..
                    } = &mut block.terminator
                    {
                        cleanup_actions.clear();
                    }
                }
            }
            "caller disposal after transfer" => {
                let root = changed
                    .machines
                    .iter_mut()
                    .find(|machine| machine.id == original.entry)
                    .unwrap();
                let place = root.structural_parameters[0].place;
                let mut returns = 0;
                for block in &mut root.blocks {
                    if let terminal_psi::Terminator::Return {
                        cleanup_actions, ..
                    } = &mut block.terminator
                    {
                        assert!(cleanup_actions.is_empty());
                        cleanup_actions.push(
                            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place),
                        );
                        returns += 1;
                    }
                }
                assert_eq!(returns, 1);
            }
            "missing signature" => changed
                .machines
                .iter_mut()
                .find(|machine| machine.id == original.entry)
                .unwrap()
                .structural_parameters
                .clear(),
            "write-only field read" => {
                let owner = changed
                    .machines
                    .iter_mut()
                    .find(|machine| {
                        machine
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .any(|operation| {
                                matches!(
                                    operation.kind,
                                    OperationKind::IntegerStructuralField { .. }
                                )
                            })
                    })
                    .unwrap();
                owner.structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
            }
            "unknown field" | "field result type" => {
                let operation = changed
                    .machines
                    .iter_mut()
                    .flat_map(|machine| &mut machine.blocks)
                    .flat_map(|block| &mut block.operations)
                    .find(|operation| {
                        matches!(operation.kind, OperationKind::IntegerStructuralField { .. })
                    })
                    .unwrap();
                if mutation == "unknown field" {
                    let OperationKind::IntegerStructuralField { field, .. } = &mut operation.kind
                    else {
                        unreachable!();
                    };
                    *field = semantic_vocabulary::StructuralFieldId::new(u64::MAX).unwrap();
                } else {
                    operation.result.scalar_mut().unwrap().scalar_type =
                        semantic_vocabulary::ScalarType::Boolean;
                }
            }
            "missing actual" | "argument access" => {
                let root = changed
                    .machines
                    .iter_mut()
                    .find(|machine| machine.id == original.entry)
                    .unwrap();
                let arguments = root
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find_map(|operation| {
                        if let OperationKind::CallStructuralScalar {
                            structural_arguments,
                            ..
                        } = &mut operation.kind
                        {
                            Some(structural_arguments)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                if mutation == "missing actual" {
                    arguments.clear();
                } else {
                    arguments[0].access = StructuralAccess::SharedBorrow;
                }
            }
            "missing type" => changed.structural_types.clear(),
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "accepted malformed owned module before proof checking: {mutation}"
        );
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &proof,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "verified corrupted owned artifact: {mutation}"
        );
    }
}

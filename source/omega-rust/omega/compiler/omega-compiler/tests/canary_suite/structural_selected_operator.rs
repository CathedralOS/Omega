use super::*;

#[test]
fn specialized_structural_fixed_operator_unit_terminal_custody_canary_compiles() {
    let canary =
        pass_canary("providers/specialized_structural_fixed_operator_unit_terminal_custody");
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("structural fixed-token Unit custody canary should check");
    let consume = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("structural Unit consumer");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == consume.symbol)
        .expect("checked structural Unit plan");
    let [
        psi_checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
            coordinate,
            result,
            structural_arguments,
            ..
        },
        psi_checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_discards,
            ..
        },
    ] = plan.operations.as_slice()
    else {
        panic!("exact structural-scalar call and Unit return")
    };
    assert_eq!(coordinate.statement_index, 0);
    assert_eq!(coordinate.call_ordinal, 0);
    assert_eq!(result.binding_ordinal, 0);
    assert_eq!(
        result.primitive_type,
        psi_typed_trees::types::PrimitiveType::Bool
    );
    assert_eq!(structural_arguments.len(), 2);
    assert!(structural_arguments.iter().all(|argument| {
        argument.path.is_empty()
            && argument.byte_sequence_literal.is_none()
            && argument.access == psi_checked_trees::CheckedStructuralAccess::Owned
    }));
    assert!(trivial_affine_discards.is_empty());
    omega_selected_dispatch::validate_selected_operator_terminal_custody(
        &checked,
        checked.selected_provider_plans(),
    )
    .expect("structural Unit call must retain its exact selected ProviderPlan");

    let produced =
        psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &checked,
            "consume",
        )
        .expect("structural fixed-token Unit application should reach Terminal");
    produced
        .boundary_operator_scope()
        .validate_for_artifact(produced.artifact())
        .expect("checked Unit D29 scope should bind the exact Terminal artifact");
    let [application] = produced.boundary_operator_scope().applications() else {
        panic!("one exact structural fixed-token Unit application")
    };
    let [occurrence] = produced.boundary_operator_scope().occurrences() else {
        panic!("one exact structural fixed-token Unit occurrence")
    };
    let module = psi_terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("structural fixed-token Unit Terminal semantics should decode");
    let calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == occurrence.terminal_operation())
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one exact D29-bound structural-scalar Terminal call")
    };
    let psi_terminal::OperationKind::CallStructuralScalar {
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = &call.kind
    else {
        panic!("Unit selection must lower as CallStructuralScalar")
    };
    assert_eq!(structural_arguments.len(), 2);
    assert!(structural_arguments.iter().all(|argument| {
        argument.path.is_empty() && argument.access == psi_terminal::StructuralAccess::Owned
    }));
    assert!(claim_transfers.is_empty());
    assert!(requirement_obligations.is_empty());
    assert!(crash_continuations.is_empty());

    let realizations =
        omega_selected_dispatch::derive_checked_specialized_operator_application_realizations(
            &checked,
            checked.selected_provider_plans(),
        )
        .expect("structural Unit application should rejoin its specialization");
    let [realization] = realizations.as_slice() else {
        panic!("one exact specialized structural Unit realization")
    };
    assert_eq!(realization.application_arguments, application.arguments);
}

#[test]
fn specialized_structural_fixed_operator_unit_terminal_custody_rejects_drift() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        DuplicateSource,
        Reordered,
        ProjectedPath,
        BorrowedAccess,
        MissingRealization,
        MissingProviderPlan,
        RestoredDiscard,
    }

    let canary =
        pass_canary("providers/specialized_structural_fixed_operator_unit_terminal_custody");
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("structural fixed-token Unit custody canary should check");
    let consume = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("structural Unit consumer");

    for drift in [
        Drift::DuplicateSource,
        Drift::Reordered,
        Drift::ProjectedPath,
        Drift::BorrowedAccess,
        Drift::MissingRealization,
        Drift::MissingProviderPlan,
        Drift::RestoredDiscard,
    ] {
        let mut drifted = checked.clone();
        let plan = drifted
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == consume.symbol)
            .expect("checked structural Unit plan");
        match drift {
            Drift::RestoredDiscard => {
                let Some(psi_checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
                    trivial_affine_discards,
                    ..
                }) = plan.operations.last_mut()
                else {
                    panic!("checked Unit return")
                };
                trivial_affine_discards.push(0);
            }
            drift => {
                let Some(
                    psi_checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                        realization_machine,
                        provider_plan_commitment,
                        structural_arguments,
                        ..
                    },
                ) = plan.operations.first_mut()
                else {
                    panic!("checked structural-scalar Unit call")
                };
                match drift {
                    Drift::DuplicateSource => {
                        structural_arguments[1].source_parameter_index =
                            structural_arguments[0].source_parameter_index;
                    }
                    Drift::Reordered => structural_arguments.swap(0, 1),
                    Drift::ProjectedPath => structural_arguments[0]
                        .path
                        .push(psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0)),
                    Drift::BorrowedAccess => {
                        structural_arguments[0].access =
                            psi_checked_trees::CheckedStructuralAccess::SharedBorrow;
                    }
                    Drift::MissingRealization => {
                        *realization_machine = psi_symbols::SymbolHandle::invalid();
                    }
                    Drift::MissingProviderPlan => {
                        *provider_plan_commitment =
                            psi_checked_trees::CheckedProviderPlanCommitment::default();
                    }
                    Drift::RestoredDiscard => unreachable!(),
                }
            }
        }
        if psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &drifted,
            "consume",
        )
        .is_ok()
        {
            panic!("{drift:?} must reject before Terminal publication");
        }
    }
}

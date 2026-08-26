use omega_backend_plan::{
    BoundNominalCallbackPlacement, CallbackPrivateRelocationDemand, CallbackThunkPlan,
    callback_placement_binding_identity, replay_callback_private_relocation_demands,
};
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

pub(super) fn plan_callback_private_relocations(
    placements: &[BoundNominalCallbackPlacement],
    thunks: &[CallbackThunkPlan],
) -> Result<Arc<[CallbackPrivateRelocationDemand]>, Diagnostic> {
    let mut demands = Vec::new();
    for (placement_index, placement) in placements.iter().enumerate() {
        let Some(materialization) = &placement.private_materialization else {
            continue;
        };
        let matching_thunks = thunks
            .iter()
            .filter(|thunk| thunk.placement_index == placement_index)
            .collect::<Vec<_>>();
        let [thunk] = matching_thunks.as_slice() else {
            return Err(Diagnostic::error(format!(
                "callback private materialization at placement {placement_index} resolves to {} thunks; exactly one is required",
                matching_thunks.len()
            )));
        };
        demands.push(CallbackPrivateRelocationDemand {
            placement_index,
            placement_identity: callback_placement_binding_identity(placement),
            binder: materialization.binder,
            destination: materialization.destination.clone(),
            requirement: materialization.requirement,
            function_identity: thunk.function_identity,
            private_symbol: Arc::clone(&thunk.private_symbol),
        });
    }
    replay_callback_private_relocation_demands(placements, thunks, &demands).map_err(|error| {
        Diagnostic::error(format!(
            "callback private relocation demand replay failed: {error}"
        ))
    })?;
    Ok(Arc::from(demands))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callback_thunks::plan_callback_thunks;
    use omega_backend_plan::{
        BoundCallbackPrivateMaterialization, BoundNominalCallbackPlacement,
        callback_thunk_placement_identity_fingerprint, replay_callback_private_relocation_demands,
    };
    use omega_calling_conventions::{
        CallSignature, CallbackBinderRequirement, CallbackMaterialization,
        CallbackMaterializationContext, CallbackRequirementId, CallingPolicy, NativeCallbackDemand,
        NativeParameterId, NativePlace, StaticMachineBinderId,
        validate_boundary_entry_plan_with_callback_materializations,
    };
    use omega_control_flow::{ControlFlowPlan, MachineFlow, StateFlow, StateKey};
    use psi_checked_trees::NominalMachineUseSite;
    use psi_symbols::SymbolHandle;

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn fixture() -> (ControlFlowPlan, BoundNominalCallbackPlacement) {
        let binder = StaticMachineBinderId::new(11).unwrap();
        let requirement = CallbackRequirementId::new(13).unwrap();
        let destination = NativePlace::Parameter(NativeParameterId::new(17).unwrap());
        let other_binder = StaticMachineBinderId::new(19).unwrap();
        let other_requirement = CallbackRequirementId::new(23).unwrap();
        let other_destination = NativePlace::Parameter(NativeParameterId::new(29).unwrap());
        let context = CallbackMaterializationContext {
            binders: vec![
                CallbackBinderRequirement {
                    binder,
                    requirement,
                },
                CallbackBinderRequirement {
                    binder: other_binder,
                    requirement: other_requirement,
                },
            ],
            demands: vec![
                NativeCallbackDemand {
                    destination: destination.clone(),
                    requirement,
                },
                NativeCallbackDemand {
                    destination: other_destination.clone(),
                    requirement: other_requirement,
                },
            ],
        };
        let signature = CallSignature::default();
        let inbound = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &signature,
        )
        .unwrap();
        let mut registrar_plan = inbound.plan().clone();
        registrar_plan.call.callback_materializations = vec![
            CallbackMaterialization {
                binder,
                destination: destination.clone(),
            },
            CallbackMaterialization {
                binder: other_binder,
                destination: other_destination,
            },
        ];
        let registrar = validate_boundary_entry_plan_with_callback_materializations(
            registrar_plan,
            &signature,
            &context,
        )
        .expect("target-closed registrar plan");
        let selected_machine = symbol(4);
        let selected_entry = symbol(5);
        let mut control_flow = ControlFlowPlan::default();
        let states = control_flow.states.insert_many([StateFlow {
            key: StateKey {
                machine: selected_machine,
                state: selected_entry,
                segment_index: 0,
            },
            ..Default::default()
        }]);
        control_flow.machines.insert(MachineFlow {
            symbol: selected_machine,
            states,
            ..Default::default()
        });
        let placement = BoundNominalCallbackPlacement {
            site: NominalMachineUseSite::Expression(
                psi_checked_trees::expression::ExpressionHandle::from_arena_index(9),
            ),
            registration_operation: symbol(3),
            static_machine_ordinal: 0,
            selected_machine,
            selected_entry,
            satisfaction_trait: symbol(1),
            satisfaction_requirement: symbol(2),
            canonical_requirement_overload: "Handler::call".to_owned(),
            boundary_calling_plan_fingerprint: inbound.contract_fingerprint(),
            boundary_entry_plan: inbound.plan().clone(),
            private_materialization: Some(BoundCallbackPrivateMaterialization {
                binder,
                destination,
                requirement,
                registrar_boundary_entry_plan: registrar.plan().clone(),
                registrar_calling_plan_fingerprint: registrar.contract_fingerprint(),
                context,
            }),
        };
        (control_flow, placement)
    }

    #[test]
    fn exact_private_materialization_becomes_one_address_free_demand() {
        let (control_flow, placement) = fixture();
        let thunks = plan_callback_thunks(&control_flow, std::slice::from_ref(&placement))
            .expect("exact callback thunk");
        let demands =
            plan_callback_private_relocations(std::slice::from_ref(&placement), thunks.as_ref())
                .expect("address-free private relocation demand");

        let materialization = placement.private_materialization.as_ref().unwrap();
        assert_eq!(demands.len(), 1);
        assert_eq!(demands[0].placement_index, 0);
        assert_eq!(demands[0].binder, materialization.binder);
        assert_eq!(demands[0].destination, materialization.destination);
        assert_eq!(demands[0].requirement, materialization.requirement);
        assert_eq!(demands[0].function_identity, thunks[0].function_identity);
        assert_eq!(demands[0].private_symbol, thunks[0].private_symbol);
        replay_callback_private_relocation_demands(
            std::slice::from_ref(&placement),
            thunks.as_ref(),
            demands.as_ref(),
        )
        .expect("complete address-free catalog replays");
    }

    #[test]
    fn private_relocation_replay_rejects_cardinality_order_and_identity_drift() {
        let (control_flow, placement) = fixture();
        let thunks = plan_callback_thunks(&control_flow, std::slice::from_ref(&placement)).unwrap();
        let demands =
            plan_callback_private_relocations(std::slice::from_ref(&placement), thunks.as_ref())
                .unwrap();

        assert!(
            replay_callback_private_relocation_demands(
                std::slice::from_ref(&placement),
                thunks.as_ref(),
                &[],
            )
            .is_err()
        );
        assert!(
            replay_callback_private_relocation_demands(
                std::slice::from_ref(&placement),
                &[],
                demands.as_ref(),
            )
            .is_err()
        );
        assert!(
            replay_callback_private_relocation_demands(
                std::slice::from_ref(&placement),
                thunks.as_ref(),
                &[demands[0].clone(), demands[0].clone()],
            )
            .is_err()
        );

        let mut binder_drift = demands[0].clone();
        binder_drift.binder = StaticMachineBinderId::new(19).unwrap();
        assert!(
            replay_callback_private_relocation_demands(
                std::slice::from_ref(&placement),
                thunks.as_ref(),
                &[binder_drift],
            )
            .is_err()
        );
        let mut destination_drift = demands[0].clone();
        destination_drift.destination = NativePlace::Parameter(NativeParameterId::new(23).unwrap());
        assert!(
            replay_callback_private_relocation_demands(
                std::slice::from_ref(&placement),
                thunks.as_ref(),
                &[destination_drift],
            )
            .is_err()
        );
        let mut requirement_drift = demands[0].clone();
        requirement_drift.requirement = CallbackRequirementId::new(29).unwrap();
        assert!(
            replay_callback_private_relocation_demands(
                std::slice::from_ref(&placement),
                thunks.as_ref(),
                &[requirement_drift],
            )
            .is_err()
        );

        let mut registrar_fingerprint_drift = placement.clone();
        registrar_fingerprint_drift
            .private_materialization
            .as_mut()
            .unwrap()
            .registrar_calling_plan_fingerprint ^= 1;
        assert!(
            replay_callback_private_relocation_demands(
                std::slice::from_ref(&registrar_fingerprint_drift),
                thunks.as_ref(),
                demands.as_ref(),
            )
            .is_err()
        );

        let mut registrar_plan_drift = placement.clone();
        registrar_plan_drift
            .private_materialization
            .as_mut()
            .unwrap()
            .registrar_boundary_entry_plan
            .state
            .preemption = omega_calling_conventions::Preemption::ProviderDefined;
        assert!(
            replay_callback_private_relocation_demands(
                std::slice::from_ref(&registrar_plan_drift),
                thunks.as_ref(),
                demands.as_ref(),
            )
            .is_err()
        );

        let mut retained_context_drift = placement.clone();
        let retained_context = &mut retained_context_drift
            .private_materialization
            .as_mut()
            .unwrap()
            .context;
        retained_context.binders.reverse();
        retained_context.demands.reverse();
        assert!(
            replay_callback_private_relocation_demands(
                std::slice::from_ref(&retained_context_drift),
                thunks.as_ref(),
                demands.as_ref(),
            )
            .is_err()
        );

        let baseline_summary = callback_thunk_placement_identity_fingerprint(thunks.as_ref());
        let mut context_order_drift = thunks.to_vec();
        let context = &mut context_order_drift[0]
            .placement_identity
            .private_materialization
            .as_mut()
            .unwrap()
            .context;
        context.binders.reverse();
        context.demands.reverse();
        assert_ne!(
            callback_thunk_placement_identity_fingerprint(&context_order_drift),
            baseline_summary,
            "the complete ordered registrar context must contribute to the thunk receipt fingerprint"
        );
        let mut symbol_drift = demands[0].clone();
        symbol_drift.private_symbol = Arc::from("__omega_callback_substituted");
        assert!(
            replay_callback_private_relocation_demands(
                std::slice::from_ref(&placement),
                thunks.as_ref(),
                &[symbol_drift],
            )
            .is_err()
        );

        let mut second = placement.clone();
        second.site = NominalMachineUseSite::Expression(
            psi_checked_trees::expression::ExpressionHandle::from_arena_index(10),
        );
        second.registration_operation = symbol(30);
        let placements = [placement, second];
        let thunks = plan_callback_thunks(&control_flow, &placements).unwrap();
        let ordered = plan_callback_private_relocations(&placements, thunks.as_ref()).unwrap();
        assert_eq!(
            ordered
                .iter()
                .map(|demand| demand.placement_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        let mut reordered = ordered.to_vec();
        reordered.reverse();
        assert!(
            replay_callback_private_relocation_demands(&placements, thunks.as_ref(), &reordered,)
                .is_err()
        );
    }
}

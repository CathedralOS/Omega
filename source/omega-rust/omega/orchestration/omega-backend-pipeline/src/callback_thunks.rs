use omega_backend_plan::{
    BoundNominalCallbackPlacement, CallbackThunkPlan, callback_placement_binding_identity,
    canonical_callback_private_symbol, plan_callback_root_schedule, replay_callback_root_schedule,
    validate_bound_nominal_callback_placement,
};
use omega_control_flow::ControlFlowPlan;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

pub(super) fn plan_callback_thunks(
    control_flow: &ControlFlowPlan,
    placements: &[BoundNominalCallbackPlacement],
) -> Result<Arc<[CallbackThunkPlan]>, Diagnostic> {
    let plans = placements
        .iter()
        .enumerate()
        .map(|(placement_index, placement)| {
            validate_bound_nominal_callback_placement(placement).map_err(|error| {
                Diagnostic::error(format!(
                    "nominal callback use for `{}` retained an invalid target calling plan: {error}",
                    placement.canonical_requirement_overload
                ))
            })?;
            let entry_key = control_flow
                .state_key_by_symbols(placement.selected_machine, placement.selected_entry)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "nominal callback use for `{}` selected an entry absent from the control-flow plan",
                        placement.canonical_requirement_overload
                    ))
                })?;
            if entry_key.segment_index != 0 {
                return Err(Diagnostic::error(format!(
                    "nominal callback use for `{}` selected noncanonical entry segment {}",
                    placement.canonical_requirement_overload, entry_key.segment_index
                )));
            }
            let function_identity =
                omega_control_flow::MachineFunctionIdentity::callback_thunk(
                    entry_key,
                    placement_index,
                )
                .expect("resolved callback entry must have a valid function identity");
            let private_symbol = canonical_callback_private_symbol(placement);
            let root_schedule = Arc::new(
                plan_callback_root_schedule(
                    placement_index,
                    placement,
                    entry_key,
                    function_identity,
                    Arc::clone(&private_symbol),
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "nominal callback use for `{}` could not retain its root schedule: {error}",
                        placement.canonical_requirement_overload
                    ))
                })?,
            );
            let plan = CallbackThunkPlan {
                placement_index,
                placement_identity: callback_placement_binding_identity(placement),
                entry_key,
                function_identity,
                private_symbol,
                root_schedule,
            };
            replay_callback_root_schedule(&plan.root_schedule, placement).map_err(|error| {
                Diagnostic::error(format!(
                    "nominal callback use for `{}` retained an invalid root schedule: {error}",
                    placement.canonical_requirement_overload
                ))
            })?;
            Ok(plan)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (index, plan) in plans.iter().enumerate() {
        if plans[..index]
            .iter()
            .any(|earlier| earlier.private_symbol == plan.private_symbol)
        {
            return Err(Diagnostic::error(format!(
                "duplicate compiler-private callback identity `{}`",
                plan.private_symbol
            )));
        }
    }
    Ok(Arc::from(plans))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{CallSignature, CallingPolicy};
    use omega_control_flow::{MachineFlow, StateFlow, StateKey};
    use psi_checked_trees::NominalMachineUseSite;
    use psi_symbols::SymbolHandle;

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn resource_receipt(
        machine: SymbolHandle,
        entry: SymbolHandle,
    ) -> psi_checked_trees::CheckedCallbackResourceReceipt {
        psi_checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(
            &psi_checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
                machine, entry, 0xfeed,
            ),
        )
        .expect("canonical checked callback resource receipt")
    }

    fn fixture() -> (ControlFlowPlan, BoundNominalCallbackPlacement) {
        let selected_machine = symbol(4);
        let selected_entry = symbol(5);
        let validated = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature::default(),
        )
        .expect("empty callback entry plan");
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
            boundary_calling_plan_fingerprint: validated.contract_fingerprint(),
            resource_receipt: resource_receipt(selected_machine, selected_entry),
            boundary_entry_plan: validated.plan().clone(),
            private_materialization: None,
        };
        (control_flow, placement)
    }

    #[test]
    fn callback_thunk_binds_exact_control_flow_entry_and_private_symbol() {
        let (control_flow, placement) = fixture();
        let placement_identity = callback_placement_binding_identity(&placement);

        let plans = plan_callback_thunks(&control_flow, &[placement.clone()])
            .expect("selected callback entry should resolve");

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].placement_index, 0);
        assert_eq!(plans[0].placement_identity, placement_identity);
        assert_eq!(plans[0].entry_key.machine, symbol(4));
        assert_eq!(plans[0].entry_key.state, symbol(5));
        assert_eq!(
            plans[0].function_identity.callback_thunk_placement_index(),
            Some(0)
        );
        assert_eq!(
            plans[0].function_identity.associated_source_continuation(),
            plans[0].entry_key
        );
        assert!(plans[0].private_symbol.starts_with("__omega_callback_e"));
        assert_eq!(plans[0].root_schedule.entry_key(), plans[0].entry_key);
        assert_eq!(
            plans[0].root_schedule.function_identity(),
            plans[0].function_identity
        );
        assert_eq!(
            plans[0].root_schedule.activation().runtime_flow_entry(),
            plans[0].entry_key
        );
        assert_eq!(
            plans[0].root_schedule.internal_call_plan(),
            &plans[0].root_schedule.boundary_entry_plan().plan().call
        );
        replay_callback_root_schedule(&plans[0].root_schedule, &placement)
            .expect("callback schedule must independently replay");
    }

    #[test]
    fn callback_thunk_rejects_selected_entry_lost_before_backend() {
        let (_, placement) = fixture();

        let error = plan_callback_thunks(&ControlFlowPlan::default(), &[placement])
            .expect_err("missing selected callback entry must reject");

        assert!(error.message.contains("absent from the control-flow plan"));
    }

    #[test]
    fn callback_thunk_rejects_duplicate_private_identity() {
        let (control_flow, placement) = fixture();

        let error = plan_callback_thunks(&control_flow, &[placement.clone(), placement])
            .expect_err("duplicate callback placement must not alias one private symbol");

        assert!(
            error
                .message
                .contains("duplicate compiler-private callback identity")
        );
    }

    #[test]
    fn callback_thunk_rejects_retained_plan_or_fingerprint_drift() {
        let (control_flow, placement) = fixture();

        let mut plan_drift = placement.clone();
        plan_drift.boundary_entry_plan.state.preemption =
            omega_calling_conventions::Preemption::ProviderDefined;
        let error = plan_callback_thunks(&control_flow, &[plan_drift])
            .expect_err("changed target plan must reject before thunk planning");
        assert!(
            error
                .message
                .contains("drifted from its retained fingerprint")
        );

        let mut fingerprint_drift = placement;
        fingerprint_drift.boundary_calling_plan_fingerprint ^= 1;
        let error = plan_callback_thunks(&control_flow, &[fingerprint_drift])
            .expect_err("changed plan fingerprint must reject before thunk planning");
        assert!(
            error
                .message
                .contains("drifted from its retained fingerprint")
        );
    }

    #[test]
    fn callback_thunk_rejects_noncanonical_selected_entry_segment() {
        let (mut control_flow, placement) = fixture();
        let state_handle = control_flow
            .states
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .expect("callback fixture state");
        control_flow.states.get_mut(state_handle).key.segment_index = 1;

        let error = plan_callback_thunks(&control_flow, &[placement])
            .expect_err("callback thunk must target the canonical selected entry");

        assert!(error.message.contains("noncanonical entry segment 1"));
    }
}

use crate::{InstructionSelectionInput, derive_boundary_call_return_mechanics_footprint};
use omega_abstract_operations::{
    AbstractFunctionPlan, AbstractOperation, AbstractOperationKind, AbstractOperationPlan,
    BoundaryFootprintFragment, BoundaryFootprintFragmentOrigin, BoundaryFootprintPlan,
    CallbackBoundaryFootprintPlan,
};
use omega_backend_plan::{
    CallbackThunkPlan, callback_placement_binding_identity, replay_callback_root_schedule,
};
use omega_calling_conventions::EntryControl;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

struct PreparedCallbackLeaf {
    placement_index: usize,
    identity: omega_control_flow::MachineFunctionIdentity,
    symbol: Arc<str>,
    source_key: omega_control_flow::StateKey,
    footprints: BoundaryFootprintPlan,
}

/// Select the first complete callback body slice: a genuinely payloadless,
/// resultless terminal leaf. Any callback needing activation-local body,
/// dispatch, or storage planning remains rejected instead of borrowing the
/// process root's plans.
pub(super) fn select_payloadless_callback_functions(
    input: &InstructionSelectionInput<'_>,
    plan: &mut AbstractOperationPlan,
) -> Result<(), Diagnostic> {
    select_payloadless_callback_roots(
        input.callback_placements,
        input.callback_thunks,
        input.control_flow,
        plan,
    )
}

fn select_payloadless_callback_roots(
    callback_placements: &[omega_backend_plan::BoundNominalCallbackPlacement],
    callback_thunks: &[CallbackThunkPlan],
    control_flow: &omega_control_flow::ControlFlowPlan,
    plan: &mut AbstractOperationPlan,
) -> Result<(), Diagnostic> {
    let prepared = prepare_payloadless_callback_functions(
        callback_placements,
        callback_thunks,
        control_flow,
        plan,
    )?;
    for callback in prepared {
        let instructions = plan.code.instructions.insert_many([
            AbstractOperation {
                kind: AbstractOperationKind::EnterFunction,
                source_key: callback.source_key,
                source_statement: 0,
            },
            AbstractOperation {
                kind: AbstractOperationKind::LeaveFunction,
                source_key: callback.source_key,
                source_statement: 0,
            },
        ]);
        plan.code.functions.insert(AbstractFunctionPlan {
            symbol: callback.symbol,
            identity: callback.identity,
            instructions,
        });
        plan.semantics
            .boundaries
            .callback_footprints
            .push(CallbackBoundaryFootprintPlan {
                placement_index: callback.placement_index,
                function_identity: callback.identity,
                footprints: callback.footprints,
            });
    }
    Ok(())
}

fn prepare_payloadless_callback_functions(
    callback_placements: &[omega_backend_plan::BoundNominalCallbackPlacement],
    callback_thunks: &[CallbackThunkPlan],
    control_flow: &omega_control_flow::ControlFlowPlan,
    plan: &AbstractOperationPlan,
) -> Result<Vec<PreparedCallbackLeaf>, Diagnostic> {
    if !plan.semantics.boundaries.callback_footprints.is_empty() {
        return Err(Diagnostic::error(
            "callback instruction selection requires an empty callback-footprint destination",
        ));
    }
    if callback_placements.len() != callback_thunks.len() {
        return Err(Diagnostic::error(format!(
            "callback instruction selection requires one thunk per placement, but retained {} placements and {} thunks",
            callback_placements.len(),
            callback_thunks.len()
        )));
    }
    let mut prepared = Vec::with_capacity(callback_placements.len());
    for (placement_index, (placement, thunk)) in
        callback_placements.iter().zip(callback_thunks).enumerate()
    {
        if thunk.placement_index != placement_index {
            return Err(Diagnostic::error(
                "callback thunk order drifted from canonical placement order",
            ));
        }
        validate_thunk_join(thunk, placement)?;
        validate_payloadless_terminal_leaf(control_flow, thunk)?;

        if plan
            .code
            .functions
            .iter()
            .any(|(_, function)| function.identity == thunk.function_identity)
            || prepared
                .iter()
                .any(|callback: &PreparedCallbackLeaf| callback.identity == thunk.function_identity)
        {
            return Err(Diagnostic::error(
                "callback leaf duplicates a compiler function identity",
            ));
        }
        if plan
            .code
            .functions
            .iter()
            .any(|(_, function)| function.symbol == thunk.private_symbol)
            || prepared
                .iter()
                .any(|callback| callback.symbol == thunk.private_symbol)
        {
            return Err(Diagnostic::error(
                "callback leaf duplicates a compiler function symbol",
            ));
        }

        let boundary = thunk.root_schedule.boundary_entry_plan();
        let selected = [
            AbstractOperationKind::EnterFunction,
            AbstractOperationKind::LeaveFunction,
        ];
        let evidence = derive_boundary_call_return_mechanics_footprint(boundary, &selected)
            .map_err(|error| Diagnostic::error(error.0))?;
        let mut footprints = BoundaryFootprintPlan::default();
        footprints
            .retain_validated_fragment(
                boundary,
                BoundaryFootprintFragment {
                    origin: BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                    evidence,
                },
            )
            .map_err(|error| Diagnostic::error(error.0))?;
        prepared.push(PreparedCallbackLeaf {
            placement_index,
            identity: thunk.function_identity,
            symbol: Arc::clone(&thunk.private_symbol),
            source_key: thunk.entry_key,
            footprints,
        });
    }
    Ok(prepared)
}

fn validate_thunk_join(
    thunk: &CallbackThunkPlan,
    placement: &omega_backend_plan::BoundNominalCallbackPlacement,
) -> Result<(), Diagnostic> {
    replay_callback_root_schedule(&thunk.root_schedule, placement)
        .map_err(|error| Diagnostic::error(error.0))?;
    let schedule = &thunk.root_schedule;
    if schedule.placement_index() != thunk.placement_index
        || schedule.placement_identity() != &thunk.placement_identity
        || thunk.placement_identity != callback_placement_binding_identity(placement)
        || schedule.entry_key() != thunk.entry_key
        || schedule.function_identity() != thunk.function_identity
        || schedule.private_symbol() != &thunk.private_symbol
    {
        return Err(Diagnostic::error(
            "callback thunk drifted from its exact root schedule or placement",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_backend_plan::{
        BoundNominalCallbackPlacement, CallbackThunkPlan, callback_placement_binding_identity,
        canonical_callback_private_symbol, plan_callback_root_schedule,
    };
    use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape};
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

    fn callback_fixture(
        policy: CallingPolicy,
        signature: &CallSignature,
        placement_index: usize,
        machine: u32,
        state: u32,
    ) -> (BoundNominalCallbackPlacement, CallbackThunkPlan) {
        let validated =
            omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(policy, signature)
                .expect("callback boundary");
        let placement = BoundNominalCallbackPlacement {
            site: NominalMachineUseSite::Expression(
                psi_checked_trees::expression::ExpressionHandle::from_arena_index(
                    9 + u32::try_from(placement_index).expect("fixture index"),
                ),
            ),
            registration_operation: symbol(3),
            static_machine_ordinal: u32::try_from(placement_index).expect("fixture ordinal"),
            selected_machine: symbol(machine),
            selected_entry: symbol(state),
            satisfaction_trait: symbol(1),
            satisfaction_requirement: symbol(2),
            canonical_requirement_overload: format!("Handler::call#{placement_index}"),
            boundary_calling_plan_fingerprint: validated.contract_fingerprint(),
            resource_receipt: resource_receipt(symbol(machine), symbol(state)),
            boundary_entry_plan: validated.plan().clone(),
            private_materialization: None,
        };
        let entry_key = StateKey {
            machine: placement.selected_machine,
            state: placement.selected_entry,
            segment_index: 0,
        };
        let function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(entry_key, placement_index)
                .expect("callback identity");
        let private_symbol = canonical_callback_private_symbol(&placement);
        let root_schedule = Arc::new(
            plan_callback_root_schedule(
                placement_index,
                &placement,
                entry_key,
                function_identity,
                Arc::clone(&private_symbol),
            )
            .expect("callback schedule"),
        );
        let thunk = CallbackThunkPlan {
            placement_index,
            placement_identity: callback_placement_binding_identity(&placement),
            entry_key,
            function_identity,
            private_symbol,
            root_schedule,
        };
        (placement, thunk)
    }

    fn control_flow(keys: &[StateKey]) -> omega_control_flow::ControlFlowPlan {
        let mut plan = omega_control_flow::ControlFlowPlan::default();
        for key in keys {
            let states = plan.states.insert_many([StateFlow {
                key: *key,
                ..Default::default()
            }]);
            plan.machines.insert(MachineFlow {
                symbol: key.machine,
                states,
                ..Default::default()
            });
        }
        plan
    }

    fn select_one(
        policy: CallingPolicy,
    ) -> (
        AbstractOperationPlan,
        BoundNominalCallbackPlacement,
        CallbackThunkPlan,
    ) {
        let (placement, thunk) = callback_fixture(policy, &CallSignature::default(), 0, 4, 5);
        let flow = control_flow(&[thunk.entry_key]);
        let mut plan = AbstractOperationPlan::default();
        select_payloadless_callback_roots(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&thunk),
            &flow,
            &mut plan,
        )
        .expect("payloadless leaf callback");
        (plan, placement, thunk)
    }

    #[test]
    fn selects_exact_payloadless_leaf_for_x86_64_and_aarch64() {
        for policy in [CallingPolicy::SystemVAMD64, CallingPolicy::Aapcs64] {
            let (plan, placement, thunk) = select_one(policy);
            let functions = plan.code.functions.storage_slice();
            assert_eq!(functions.len(), 1);
            assert_eq!(functions[0].identity, thunk.function_identity);
            assert_eq!(functions[0].symbol, thunk.private_symbol);
            let instructions = plan
                .code
                .instructions
                .span(functions[0].instructions)
                .expect("callback instructions");
            assert_eq!(instructions.len(), 2);
            assert_eq!(instructions[0].kind, AbstractOperationKind::EnterFunction);
            assert_eq!(instructions[1].kind, AbstractOperationKind::LeaveFunction);
            assert!(
                instructions
                    .iter()
                    .all(|row| row.source_key == thunk.entry_key)
            );

            assert_eq!(
                plan.semantics.boundaries.footprints,
                BoundaryFootprintPlan::default(),
                "callback evidence must not merge into process-entry evidence"
            );
            let [callback] = plan.semantics.boundaries.callback_footprints.as_slice() else {
                panic!("one callback footprint row")
            };
            assert_eq!(callback.placement_index, 0);
            assert_eq!(callback.function_identity, thunk.function_identity);
            assert_eq!(
                callback.footprints.boundary_contract_fingerprint,
                Some(placement.boundary_calling_plan_fingerprint)
            );
            assert_eq!(callback.footprints.fragments.len(), 1);
            assert_eq!(
                callback.footprints.fragments[0].origin,
                BoundaryFootprintFragmentOrigin::CallReturnMechanics
            );
        }
    }

    #[test]
    fn retains_multiple_callback_functions_and_footprints_in_exact_placement_order() {
        let (first_placement, first_thunk) = callback_fixture(
            CallingPolicy::SystemVAMD64,
            &CallSignature::default(),
            0,
            4,
            5,
        );
        let (second_placement, second_thunk) = callback_fixture(
            CallingPolicy::SystemVAMD64,
            &CallSignature::default(),
            1,
            6,
            7,
        );
        let flow = control_flow(&[first_thunk.entry_key, second_thunk.entry_key]);
        let mut plan = AbstractOperationPlan::default();

        select_payloadless_callback_roots(
            &[first_placement, second_placement],
            &[first_thunk.clone(), second_thunk.clone()],
            &flow,
            &mut plan,
        )
        .expect("two independent callback leaves");

        assert_eq!(
            plan.code
                .functions
                .iter()
                .map(|(_, function)| function.identity)
                .collect::<Vec<_>>(),
            vec![
                first_thunk.function_identity,
                second_thunk.function_identity
            ]
        );
        assert_eq!(
            plan.semantics
                .boundaries
                .callback_footprints
                .iter()
                .map(|row| (row.placement_index, row.function_identity))
                .collect::<Vec<_>>(),
            vec![
                (0, first_thunk.function_identity),
                (1, second_thunk.function_identity),
            ]
        );
    }

    #[test]
    fn rejects_argument_result_and_non_call_return_callbacks() {
        for signature in [
            CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            },
            CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(4, 4)),
            },
        ] {
            let (placement, thunk) =
                callback_fixture(CallingPolicy::SystemVAMD64, &signature, 0, 4, 5);
            let error = select_payloadless_callback_roots(
                &[placement],
                std::slice::from_ref(&thunk),
                &control_flow(&[thunk.entry_key]),
                &mut AbstractOperationPlan::default(),
            )
            .expect_err("payload/result must reject");
            assert!(error.message.contains("no parameters and no result"));
        }

        let (placement, thunk) = callback_fixture(
            CallingPolicy::LinuxSyscallX86_64,
            &CallSignature::default(),
            0,
            4,
            5,
        );
        let error = select_payloadless_callback_roots(
            &[placement],
            std::slice::from_ref(&thunk),
            &control_flow(&[thunk.entry_key]),
            &mut AbstractOperationPlan::default(),
        )
        .expect_err("supervisor callback must reject");
        assert!(error.message.contains("call-return control"));
    }

    #[test]
    fn rejects_state_parameters_operations_transitions_and_semantic_rows() {
        let (placement, thunk) = callback_fixture(
            CallingPolicy::SystemVAMD64,
            &CallSignature::default(),
            0,
            4,
            5,
        );
        let mut flow = control_flow(&[thunk.entry_key]);
        let state_handle = flow
            .states
            .iter()
            .find_map(|(handle, state)| (state.key == thunk.entry_key).then_some(handle))
            .expect("callback state");
        flow.states.get_mut(state_handle).parameters = flow
            .state_parameters
            .insert_many([omega_control_flow::StateParameterFlow::default()]);
        let error = select_payloadless_callback_roots(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&thunk),
            &flow,
            &mut AbstractOperationPlan::default(),
        )
        .expect_err("state parameters must reject");
        assert!(error.message.contains("state parameters"));

        let mut flow = control_flow(&[thunk.entry_key]);
        let state_handle = flow.states.iter().next().expect("state").0;
        flow.states.get_mut(state_handle).operations = flow
            .operations
            .insert_many([omega_control_flow::Operation::default()]);
        let error = select_payloadless_callback_roots(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&thunk),
            &flow,
            &mut AbstractOperationPlan::default(),
        )
        .expect_err("operations must reject");
        assert!(error.message.contains("body operations"));

        let mut flow = control_flow(&[thunk.entry_key]);
        let state_handle = flow.states.iter().next().expect("state").0;
        flow.states.get_mut(state_handle).transitions = flow
            .transitions
            .insert_many([omega_control_flow::TransitionFlow::default()]);
        let error = select_payloadless_callback_roots(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&thunk),
            &flow,
            &mut AbstractOperationPlan::default(),
        )
        .expect_err("transitions must reject");
        assert!(error.message.contains("transitions"));

        let mut flow = control_flow(&[thunk.entry_key]);
        let state_handle = flow.states.iter().next().expect("state").0;
        flow.states
            .get_mut(state_handle)
            .borrow
            .mutable_parameter_count = 1;
        let error = select_payloadless_callback_roots(
            &[placement],
            &[thunk],
            &flow,
            &mut AbstractOperationPlan::default(),
        )
        .expect_err("hidden body semantics must reject");
        assert!(error.message.contains("borrow, or permission semantics"));
    }

    #[test]
    fn rejects_cardinality_order_join_and_existing_identity_drift() {
        let (first_placement, first_thunk) = callback_fixture(
            CallingPolicy::SystemVAMD64,
            &CallSignature::default(),
            0,
            4,
            5,
        );
        let (second_placement, second_thunk) = callback_fixture(
            CallingPolicy::SystemVAMD64,
            &CallSignature::default(),
            1,
            6,
            7,
        );
        let flow = control_flow(&[first_thunk.entry_key, second_thunk.entry_key]);

        let error = select_payloadless_callback_roots(
            std::slice::from_ref(&first_placement),
            &[],
            &flow,
            &mut AbstractOperationPlan::default(),
        )
        .expect_err("missing thunk must reject");
        assert!(error.message.contains("one thunk per placement"));

        let error = select_payloadless_callback_roots(
            &[first_placement.clone(), second_placement.clone()],
            &[second_thunk.clone(), first_thunk.clone()],
            &flow,
            &mut AbstractOperationPlan::default(),
        )
        .expect_err("reordered thunks must reject");
        assert!(error.message.contains("canonical placement order"));

        let mut substituted = first_thunk.clone();
        substituted.private_symbol = Arc::clone(&second_thunk.private_symbol);
        let error = select_payloadless_callback_roots(
            std::slice::from_ref(&first_placement),
            &[substituted],
            &flow,
            &mut AbstractOperationPlan::default(),
        )
        .expect_err("schedule substitution must reject");
        assert!(error.message.contains("root schedule or placement"));

        let error = select_payloadless_callback_roots(
            std::slice::from_ref(&second_placement),
            std::slice::from_ref(&first_thunk),
            &flow,
            &mut AbstractOperationPlan::default(),
        )
        .expect_err("placement substitution must reject");
        assert!(error.message.contains("callback root schedule"));

        let mut plan = AbstractOperationPlan::default();
        plan.code.functions.insert(AbstractFunctionPlan {
            symbol: Arc::from("existing"),
            identity: first_thunk.function_identity,
            instructions: Default::default(),
        });
        let error = select_payloadless_callback_roots(
            std::slice::from_ref(&first_placement),
            std::slice::from_ref(&first_thunk),
            &flow,
            &mut plan,
        )
        .expect_err("duplicate identity must reject");
        assert!(
            error
                .message
                .contains("duplicates a compiler function identity")
        );

        let mut plan = AbstractOperationPlan::default();
        plan.code.functions.insert(AbstractFunctionPlan {
            symbol: Arc::clone(&first_thunk.private_symbol),
            identity: omega_control_flow::MachineFunctionIdentity::source(StateKey {
                machine: symbol(40),
                state: symbol(41),
                segment_index: 0,
            }),
            instructions: Default::default(),
        });
        let error = select_payloadless_callback_roots(
            std::slice::from_ref(&first_placement),
            std::slice::from_ref(&first_thunk),
            &flow,
            &mut plan,
        )
        .expect_err("duplicate symbol must reject");
        assert!(
            error
                .message
                .contains("duplicates a compiler function symbol")
        );

        let mut plan = AbstractOperationPlan::default();
        plan.semantics
            .boundaries
            .callback_footprints
            .push(CallbackBoundaryFootprintPlan {
                placement_index: 99,
                function_identity: first_thunk.function_identity,
                footprints: BoundaryFootprintPlan::default(),
            });
        let error = select_payloadless_callback_roots(
            &[first_placement, second_placement],
            &[first_thunk, second_thunk],
            &flow,
            &mut plan,
        )
        .expect_err("preexisting callback evidence must reject");
        assert!(
            error
                .message
                .contains("empty callback-footprint destination")
        );
    }
}

fn validate_payloadless_terminal_leaf(
    control_flow: &omega_control_flow::ControlFlowPlan,
    thunk: &CallbackThunkPlan,
) -> Result<(), Diagnostic> {
    let call = thunk.root_schedule.internal_call_plan();
    if !call.parameters.is_empty() || call.result.is_some() {
        return Err(Diagnostic::error(
            "callback leaf selection currently requires no parameters and no result",
        ));
    }
    if call.entry_control != EntryControl::CallReturn {
        return Err(Diagnostic::error(
            "callback leaf selection currently requires ordinary call-return control",
        ));
    }
    let state = control_flow
        .state_by_key(thunk.entry_key)
        .ok_or_else(|| Diagnostic::error("callback leaf lost its exact control-flow entry"))?;
    if !state.parameters.is_empty() {
        return Err(Diagnostic::error(
            "callback leaf selection currently rejects state parameters",
        ));
    }
    if !state.operations.is_empty() {
        return Err(Diagnostic::error(
            "callback leaf selection currently rejects body operations",
        ));
    }
    if !state.transitions.is_empty() {
        return Err(Diagnostic::error(
            "callback leaf selection currently rejects transitions",
        ));
    }
    if state.values != omega_control_flow::StateValueSummary::default()
        || state.boundaries != omega_control_flow::StateBoundarySummary::default()
        || state.borrow != omega_control_flow::StateBorrowSummary::default()
        || state.ownership != omega_control_flow::StateOwnershipSummary::default()
    {
        return Err(Diagnostic::error(
            "callback leaf selection currently rejects retained body, boundary, borrow, or permission semantics",
        ));
    }
    Ok(())
}

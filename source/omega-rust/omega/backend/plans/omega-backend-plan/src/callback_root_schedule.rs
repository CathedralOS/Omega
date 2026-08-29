use crate::{
    BoundNominalCallbackPlacement, CallbackPlacementBindingIdentity,
    callback_placement_binding_identity, canonical_callback_private_symbol,
    validate_bound_nominal_callback_placement,
};
use omega_calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallSignature, PlanDiagnostic, ValidatedBoundaryEntryPlan,
    validate_boundary_entry_plan, validate_call_plan,
};
use omega_function_identity::{MachineFunctionIdentity, StateKey};
use std::sync::Arc;

/// One callback-local root activation before any target instruction is chosen.
///
/// The four identities deliberately remain separate. Future per-root planners
/// will populate their respective plans under this schedule; treating the
/// process root's dispatch namespace or frame as interchangeable with a
/// re-entrant callback root would alias activation-local state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackRootActivationIdentity {
    placement_index: usize,
    function_identity: MachineFunctionIdentity,
    runtime_flow_entry: StateKey,
    dispatch_entry: StateKey,
    dispatch_index: u32,
    storage_entry: StateKey,
    storage_dispatch_index: u32,
    frame_entry: StateKey,
    frame_dispatch_index: u32,
}

impl CallbackRootActivationIdentity {
    fn canonical(
        placement_index: usize,
        function_identity: MachineFunctionIdentity,
        entry_key: StateKey,
    ) -> Self {
        Self {
            placement_index,
            function_identity,
            runtime_flow_entry: entry_key,
            dispatch_entry: entry_key,
            dispatch_index: 0,
            storage_entry: entry_key,
            storage_dispatch_index: 0,
            frame_entry: entry_key,
            frame_dispatch_index: 0,
        }
    }

    pub const fn placement_index(&self) -> usize {
        self.placement_index
    }

    pub const fn function_identity(&self) -> MachineFunctionIdentity {
        self.function_identity
    }

    pub const fn runtime_flow_entry(&self) -> StateKey {
        self.runtime_flow_entry
    }

    pub const fn dispatch_entry(&self) -> StateKey {
        self.dispatch_entry
    }

    pub const fn dispatch_index(&self) -> u32 {
        self.dispatch_index
    }

    pub const fn storage_entry(&self) -> StateKey {
        self.storage_entry
    }

    pub const fn storage_dispatch_index(&self) -> u32 {
        self.storage_dispatch_index
    }

    pub const fn frame_entry(&self) -> StateKey {
        self.frame_entry
    }

    pub const fn frame_dispatch_index(&self) -> u32 {
        self.frame_dispatch_index
    }
}

/// Address-free recipe for entering one selected callback machine.
///
/// This carrier is intentionally not `Clone`: one thunk owns one root schedule.
/// `CallbackThunkPlan` shares that ownership through `Arc` only because backend
/// plans themselves remain cheaply clonable. The schedule retains ABI and
/// activation identity, not thunk bytes, relocation placement, resource
/// ceilings, registration authority, or an installed-root lease.
#[derive(Debug, PartialEq, Eq)]
pub struct CallbackRootSchedule {
    placement_index: usize,
    placement_identity: CallbackPlacementBindingIdentity,
    entry_key: StateKey,
    function_identity: MachineFunctionIdentity,
    private_symbol: Arc<str>,
    activations: Vec<CallbackRootActivationIdentity>,
    boundary_entry_plan: ValidatedBoundaryEntryPlan,
    internal_call_plan: CallPlan,
}

impl CallbackRootSchedule {
    pub const fn placement_index(&self) -> usize {
        self.placement_index
    }

    pub const fn placement_identity(&self) -> &CallbackPlacementBindingIdentity {
        &self.placement_identity
    }

    pub const fn entry_key(&self) -> StateKey {
        self.entry_key
    }

    pub const fn function_identity(&self) -> MachineFunctionIdentity {
        self.function_identity
    }

    pub fn private_symbol(&self) -> &Arc<str> {
        &self.private_symbol
    }

    pub fn activation(&self) -> &CallbackRootActivationIdentity {
        // Construction and replay admit exactly one activation.
        &self.activations[0]
    }

    pub const fn boundary_entry_plan(&self) -> &ValidatedBoundaryEntryPlan {
        &self.boundary_entry_plan
    }

    pub const fn internal_call_plan(&self) -> &CallPlan {
        &self.internal_call_plan
    }
}

/// Construct the sole canonical address-free root schedule for one callback
/// thunk. The internal entry recipe is retained separately from the boundary
/// state contract so later lowering cannot silently switch either side of the
/// argument/result bridge.
pub fn plan_callback_root_schedule(
    placement_index: usize,
    placement: &BoundNominalCallbackPlacement,
    entry_key: StateKey,
    function_identity: MachineFunctionIdentity,
    private_symbol: Arc<str>,
) -> Result<CallbackRootSchedule, PlanDiagnostic> {
    let validated = validate_bound_nominal_callback_placement(placement)?;
    let schedule = CallbackRootSchedule {
        placement_index,
        placement_identity: callback_placement_binding_identity(placement),
        entry_key,
        function_identity,
        private_symbol,
        activations: vec![CallbackRootActivationIdentity::canonical(
            placement_index,
            function_identity,
            entry_key,
        )],
        internal_call_plan: validated.plan().call.clone(),
        boundary_entry_plan: validated,
    };
    replay_callback_root_schedule(&schedule, placement)?;
    Ok(schedule)
}

/// Independently replay one callback root schedule against its authoritative
/// checked placement. This stops before target instruction selection.
pub fn replay_callback_root_schedule(
    schedule: &CallbackRootSchedule,
    placement: &BoundNominalCallbackPlacement,
) -> Result<(), PlanDiagnostic> {
    let expected_entry = StateKey {
        machine: placement.selected_machine,
        state: placement.selected_entry,
        segment_index: 0,
    };
    if schedule.entry_key != expected_entry {
        return Err(PlanDiagnostic(
            "callback root schedule entry or canonical segment drifted".into(),
        ));
    }
    if schedule.placement_identity != callback_placement_binding_identity(placement) {
        return Err(PlanDiagnostic(
            "callback root schedule placement identity drifted".into(),
        ));
    }
    if schedule.function_identity.callback_thunk_placement_index() != Some(schedule.placement_index)
        || schedule.function_identity.associated_source_continuation() != expected_entry
    {
        return Err(PlanDiagnostic(
            "callback root schedule thunk identity drifted".into(),
        ));
    }
    if schedule.private_symbol != canonical_callback_private_symbol(placement) {
        return Err(PlanDiagnostic(
            "callback root schedule private thunk identity drifted".into(),
        ));
    }

    let expected_boundary = validate_bound_nominal_callback_placement(placement)?;
    if schedule.boundary_entry_plan != expected_boundary {
        return Err(PlanDiagnostic(
            "callback root schedule boundary entry plan drifted".into(),
        ));
    }
    let signature = signature_for_boundary(schedule.boundary_entry_plan.plan());
    let replayed_boundary =
        validate_boundary_entry_plan(schedule.boundary_entry_plan.plan().clone(), &signature)?;
    if replayed_boundary != schedule.boundary_entry_plan {
        return Err(PlanDiagnostic(
            "callback root schedule boundary entry plan is noncanonical".into(),
        ));
    }
    validate_call_plan(&schedule.internal_call_plan, &signature)?;
    if schedule.internal_call_plan != schedule.boundary_entry_plan.plan().call {
        return Err(PlanDiagnostic(
            "callback root schedule argument/result bridge drifted".into(),
        ));
    }

    let [activation] = schedule.activations.as_slice() else {
        return Err(PlanDiagnostic(
            "callback root schedule requires exactly one activation".into(),
        ));
    };
    if activation.placement_index != schedule.placement_index
        || activation.function_identity != schedule.function_identity
    {
        return Err(PlanDiagnostic(
            "callback root activation thunk identity drifted".into(),
        ));
    }
    if activation.runtime_flow_entry != expected_entry
        || activation.dispatch_entry != expected_entry
        || activation.dispatch_index != 0
    {
        return Err(PlanDiagnostic(
            "callback root schedule runtime-flow or dispatch identity drifted".into(),
        ));
    }
    if activation.storage_entry != expected_entry || activation.storage_dispatch_index != 0 {
        return Err(PlanDiagnostic(
            "callback root schedule storage identity drifted".into(),
        ));
    }
    if activation.frame_entry != expected_entry || activation.frame_dispatch_index != 0 {
        return Err(PlanDiagnostic(
            "callback root schedule frame identity drifted".into(),
        ));
    }
    Ok(())
}

fn signature_for_boundary(plan: &BoundaryEntryPlan) -> CallSignature {
    CallSignature {
        parameters: plan
            .call
            .parameters
            .iter()
            .map(|parameter| parameter.shape)
            .collect(),
        result: plan.call.result.as_ref().map(|result| result.shape),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, MachineRegister, ValueLocation, ValueShape,
    };
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

    fn placement() -> BoundNominalCallbackPlacement {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(4, 4)),
        };
        let validated = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &signature,
        )
        .expect("callback plan");
        let selected_machine = symbol(4);
        let selected_entry = symbol(5);
        BoundNominalCallbackPlacement {
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
            boundary_calling_plan_report_fingerprint: validated.contract_fingerprint(),
            resource_receipt: resource_receipt(selected_machine, selected_entry),
            boundary_entry_plan: validated.plan().clone(),
            private_materialization: None,
        }
    }

    fn schedule() -> (BoundNominalCallbackPlacement, CallbackRootSchedule) {
        let placement = placement();
        let entry_key = StateKey {
            machine: placement.selected_machine,
            state: placement.selected_entry,
            segment_index: 0,
        };
        let function_identity = MachineFunctionIdentity::callback_thunk(entry_key, 0)
            .expect("callback function identity");
        let private_symbol = canonical_callback_private_symbol(&placement);
        let schedule = plan_callback_root_schedule(
            0,
            &placement,
            entry_key,
            function_identity,
            private_symbol,
        )
        .expect("callback root schedule");
        (placement, schedule)
    }

    #[test]
    fn schedule_retains_one_activation_and_exact_boundary_bridge() {
        let (placement, schedule) = schedule();

        replay_callback_root_schedule(&schedule, &placement).expect("schedule replay");
        assert_eq!(
            schedule.activation().runtime_flow_entry(),
            schedule.entry_key()
        );
        assert_eq!(schedule.activation().dispatch_index(), 0);
        assert_eq!(schedule.activation().storage_dispatch_index(), 0);
        assert_eq!(schedule.activation().frame_dispatch_index(), 0);
        assert_eq!(
            schedule.activation().function_identity(),
            schedule.function_identity()
        );
        assert_eq!(
            schedule.internal_call_plan(),
            &schedule.boundary_entry_plan().plan().call
        );
    }

    #[test]
    fn replay_rejects_missing_duplicate_and_drifted_activation_identity() {
        let (placement, mut missing_schedule) = schedule();
        missing_schedule.activations.clear();
        assert!(
            replay_callback_root_schedule(&missing_schedule, &placement)
                .unwrap_err()
                .0
                .contains("exactly one activation")
        );

        let (placement, mut duplicate_schedule) = schedule();
        duplicate_schedule
            .activations
            .push(duplicate_schedule.activations[0].clone());
        assert!(
            replay_callback_root_schedule(&duplicate_schedule, &placement)
                .unwrap_err()
                .0
                .contains("exactly one activation")
        );

        let (placement, mut runtime_schedule) = schedule();
        runtime_schedule.activations[0].placement_index = 1;
        assert!(
            replay_callback_root_schedule(&runtime_schedule, &placement)
                .unwrap_err()
                .0
                .contains("activation thunk identity")
        );

        let (placement, mut runtime_schedule) = schedule();
        runtime_schedule.activations[0].runtime_flow_entry.state = symbol(8);
        assert!(
            replay_callback_root_schedule(&runtime_schedule, &placement)
                .unwrap_err()
                .0
                .contains("runtime-flow or dispatch")
        );

        let (placement, mut schedule) = schedule();
        schedule.activations[0].dispatch_index = 1;
        assert!(
            replay_callback_root_schedule(&schedule, &placement)
                .unwrap_err()
                .0
                .contains("runtime-flow or dispatch")
        );
    }

    #[test]
    fn replay_rejects_storage_frame_and_entry_segment_drift() {
        let (placement, mut storage_schedule) = schedule();
        storage_schedule.activations[0].storage_entry.state = symbol(8);
        assert!(
            replay_callback_root_schedule(&storage_schedule, &placement)
                .unwrap_err()
                .0
                .contains("storage identity")
        );

        let (placement, mut frame_schedule) = schedule();
        frame_schedule.activations[0].frame_dispatch_index = 1;
        assert!(
            replay_callback_root_schedule(&frame_schedule, &placement)
                .unwrap_err()
                .0
                .contains("frame identity")
        );

        let (placement, mut schedule) = schedule();
        schedule.entry_key.segment_index = 1;
        assert!(
            replay_callback_root_schedule(&schedule, &placement)
                .unwrap_err()
                .0
                .contains("canonical segment")
        );
    }

    #[test]
    fn replay_rejects_placement_thunk_and_private_symbol_substitution() {
        let (placement, mut placement_schedule) = schedule();
        placement_schedule
            .placement_identity
            .satisfaction_requirement = symbol(8);
        assert!(
            replay_callback_root_schedule(&placement_schedule, &placement)
                .unwrap_err()
                .0
                .contains("placement identity")
        );

        let (mut compact_equal_placement, compact_equal_schedule) = schedule();
        let report = compact_equal_placement.boundary_calling_plan_report_fingerprint;
        compact_equal_placement.boundary_entry_plan.state.preemption =
            omega_calling_conventions::Preemption::ProviderDefined;
        assert_eq!(
            compact_equal_placement.boundary_calling_plan_report_fingerprint,
            report,
        );
        assert!(
            replay_callback_root_schedule(&compact_equal_schedule, &compact_equal_placement)
                .unwrap_err()
                .0
                .contains("placement identity"),
            "schedule replay must compare the exact plan before compact report coordinates",
        );

        let (placement, mut thunk_schedule) = schedule();
        thunk_schedule.function_identity =
            MachineFunctionIdentity::callback_thunk(thunk_schedule.entry_key, 1)
                .expect("substituted thunk identity");
        assert!(
            replay_callback_root_schedule(&thunk_schedule, &placement)
                .unwrap_err()
                .0
                .contains("thunk identity")
        );

        let (placement, mut schedule) = schedule();
        schedule.private_symbol = Arc::from("__omega_callback_substituted");
        assert!(
            replay_callback_root_schedule(&schedule, &placement)
                .unwrap_err()
                .0
                .contains("private thunk identity")
        );
    }

    #[test]
    fn replay_rejects_boundary_argument_and_result_recipe_drift() {
        let (placement, mut argument_schedule) = schedule();
        argument_schedule.internal_call_plan.parameters[0].locations[0] = ValueLocation::Register {
            register: MachineRegister::X86Rdx,
            value_byte_offset: 0,
            byte_size: 8,
        };
        let error = replay_callback_root_schedule(&argument_schedule, &placement)
            .expect_err("argument recipe drift must reject");
        assert!(error.0.contains("argument/result bridge"));

        let (placement, mut schedule) = schedule();
        schedule
            .internal_call_plan
            .result
            .as_mut()
            .expect("direct callback result")
            .locations[0] = ValueLocation::Register {
            register: MachineRegister::X86Rdx,
            value_byte_offset: 0,
            byte_size: 4,
        };
        assert!(
            replay_callback_root_schedule(&schedule, &placement)
                .unwrap_err()
                .0
                .contains("argument/result bridge")
        );
    }
}

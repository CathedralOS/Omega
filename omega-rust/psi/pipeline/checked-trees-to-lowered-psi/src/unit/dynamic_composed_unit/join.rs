//! The checked two-predecessor dynamic descriptor join, for either call result.
//!
//! The runtime phi is the callee's ordinary dynamic descriptor parameter. Each
//! predecessor call supplies its own exact selection; no representative table
//! or joined-table vocabulary is introduced. The control split, caller ABI,
//! sources, realizations, applications and descriptor catalog are lowered once
//! here. A [`JoinedDynamicCall`] lane supplies only what its call plan decides:
//! exact validation, result agreement and type, and the forwarded helper
//! chain. Each conformance member keeps its own result kind either way.

use super::{
    Block, CheckedBooleanExpression, CheckedDynamicScalarCallPlan, CheckedDynamicUnitCallPlan,
    CheckedScalarExpression, CheckedStructuralAccess, CheckedTrees,
    CheckedUnitStructuralPathSegment, ClosedConformanceApplication, ClosedConformanceRow,
    LoweredPsi, LoweredSourceCallOccurrence, LoweringError, Multiplicity, Operation, OperationKind,
    OperationResult, PrimitiveType, ProofBundle, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralPlaceKind,
    TerminalDynamicConformanceSelection, TerminalDynamicDescriptorArgument,
    TerminalDynamicDescriptorParameter, TerminalDynamicDescriptorSource,
    TerminalDynamicDispatchCatalog, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalParameterDynamicDispatch, Terminator, ValueDeclaration, allocate_dense, block_id,
    edge_id, lookup_type_id, lower_installation_machine_service_ceiling, lower_root_service_reach,
    machine_id, operation_id, place_id, terminal_scalar_type, unit, unsupported, value_id,
};
use crate::unit::dynamic_composed_unit::applications::{
    exact_machine_service_summary, lower_exact_application,
};
use crate::unit::dynamic_composed_unit::dynamic_lanes::{
    DynamicLoweringLane, ForwardedHelperCall, ForwardedHelperIds, LoweredDynamicRealization,
};
use crate::unit::dynamic_composed_unit::forwarded_helpers::{
    extend_parameter_forwarding_catalog, forwarded_helper_chain_ids,
    materialize_forwarded_helper_chain,
};
use crate::unit::dynamic_composed_unit::plan_validation::validate_exact_direct_plan;
use crate::unit::dynamic_composed_unit::realizations::{
    DynamicCallableTable, collect_dynamic_realizations, materialize_dynamic_realizations,
};
use crate::unit::dynamic_composed_unit::source_lowering::{
    dynamic_parameter_interface, validate_and_lower_dynamic_source,
};
use crate::unit::dynamic_composed_unit::store_operations::empty_terminal_contract;
use crate::unit::dynamic_composed_unit::structural_types::{
    lower_dynamic_structural_types_for_source, terminal_structural_multiplicity,
};
use checked_trees::{
    CheckedDynamicDescriptorTransferPlan, CheckedDynamicJoinBranchPlan,
    CheckedDynamicJoinControlPlan, CheckedDynamicRealizationCallablePlan,
    CheckedDynamicScalarCallOrigin, CheckedDynamicUnitCallOrigin, CheckedUnitCallCoordinate,
};
use semantic_vocabulary::{MachineId, ScalarType};
use symbols::SymbolHandle;

/// One lane of the join: what a branch call plan decides beyond the custody
/// every branch shares. The join names a result shape only through
/// [`Self::result_type`].
pub(super) trait JoinedDynamicCall {
    /// The lane's forwarded helper identities.
    type Helper: ForwardedHelperCall;

    /// The custody both lanes' call plans share.
    fn view(&self) -> JoinedCallView<'_>;

    /// The call's closed conformance table.
    fn callable_table(&self) -> DynamicCallableTable<'_>;

    /// The lane's exact validation of one branch as a direct call.
    fn validate_exact(&self, checked: &CheckedTrees) -> Result<(), LoweringError>;

    /// Result agreement between the two branches beyond the shared caller ABI.
    fn results_match(&self, other: &Self) -> bool;

    /// Agreement between the two branches' forwarded helper bodies.
    fn helper_bodies_match(&self, other: &Self) -> bool;

    /// The branch call's scalar result type, or `None` for a Unit call.
    fn result_type(&self) -> Result<Option<ScalarType>, LoweringError>;

    /// Identities for the forwarded helper chain, numbered after the
    /// realization machines.
    fn helper_ids(
        &self,
        realizations: &[LoweredDynamicRealization],
        next_block: &mut u64,
        next_operation: &mut u64,
        next_value: &mut u64,
        next_edge: &mut u64,
    ) -> Result<Vec<Self::Helper>, LoweringError>;

    /// The forwarded helper chain's machines. A helper that evaluates its own
    /// body records the values it computes before its call.
    #[allow(clippy::too_many_arguments)]
    fn materialize_helpers(
        &self,
        checked: &CheckedTrees,
        application: &ClosedConformanceApplication,
        selected_row: &ClosedConformanceRow,
        helpers: &[Self::Helper],
        next_block: &mut u64,
        next_operation: &mut u64,
        next_value: &mut u64,
        next_edge: &mut u64,
        source_calls: &mut [LoweredSourceCallOccurrence],
    ) -> Result<Vec<TerminalMachine>, LoweringError>;
}

/// One branch call's custody, independent of its result.
pub(super) struct JoinedCallView<'a> {
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    caller_attachment_type_identity: &'a str,
    caller_multiplicity: Multiplicity,
    caller_parameter_access: CheckedStructuralAccess,
    coordinate: CheckedUnitCallCoordinate,
    source_parameter_position: u32,
    source_access: CheckedStructuralAccess,
    source_path: &'a [CheckedUnitStructuralPathSegment],
    source_type_identity: &'a str,
    requirement: SymbolHandle,
    realization_machine: SymbolHandle,
    realization_callables: &'a [CheckedDynamicRealizationCallablePlan],
    forwarding_transfers: &'a [CheckedDynamicDescriptorTransferPlan],
    /// The forwarded origin's state and call coordinate; `None` when local.
    forwarded_origin: Option<(SymbolHandle, CheckedUnitCallCoordinate)>,
}

impl JoinedCallView<'_> {
    pub(super) fn realization_machine(&self) -> SymbolHandle {
        self.realization_machine
    }
}

impl JoinedDynamicCall for CheckedDynamicScalarCallPlan {
    type Helper = ForwardedHelperIds;

    fn view(&self) -> JoinedCallView<'_> {
        JoinedCallView {
            caller_machine: self.caller_machine,
            caller_state: self.caller_state,
            caller_attachment_type_identity: &self.caller_attachment_type_identity,
            caller_multiplicity: self.caller_multiplicity,
            caller_parameter_access: self.caller_parameter_access,
            coordinate: self.coordinate,
            source_parameter_position: self.source_parameter_position,
            source_access: self.source_access,
            source_path: &self.source_path,
            source_type_identity: &self.source_type_identity,
            requirement: self.requirement,
            realization_machine: self.realization_machine,
            realization_callables: &self.realization_callables,
            forwarding_transfers: &self.forwarding_transfers,
            forwarded_origin: match self.origin {
                CheckedDynamicScalarCallOrigin::Local => None,
                CheckedDynamicScalarCallOrigin::Forwarded {
                    state, coordinate, ..
                } => Some((state, coordinate)),
            },
        }
    }

    fn callable_table(&self) -> DynamicCallableTable<'_> {
        self.into()
    }

    fn validate_exact(&self, checked: &CheckedTrees) -> Result<(), LoweringError> {
        validate_exact_direct_plan(checked, self).map(|_| ())
    }

    /// The branch results bind one scalar type, and neither branch retains a
    /// caller store or Unit continuation the join would not lower.
    fn results_match(&self, other: &Self) -> bool {
        self.result.primitive_type == other.result.primitive_type
            && [self, other].into_iter().all(|plan| {
                plan.caller_structural_scalar_field_store.is_none()
                    && plan.unit_continuation.is_none()
            })
    }

    fn helper_bodies_match(&self, other: &Self) -> bool {
        self.forwarding_helpers == other.forwarding_helpers
    }

    fn result_type(&self) -> Result<Option<ScalarType>, LoweringError> {
        terminal_scalar_type(self.result.primitive_type).map(Some)
    }

    fn helper_ids(
        &self,
        realizations: &[LoweredDynamicRealization],
        next_block: &mut u64,
        next_operation: &mut u64,
        next_value: &mut u64,
        next_edge: &mut u64,
    ) -> Result<Vec<ForwardedHelperIds>, LoweringError> {
        forwarded_helper_chain_ids(
            self,
            realizations,
            next_block,
            next_operation,
            next_value,
            next_edge,
        )
    }

    fn materialize_helpers(
        &self,
        checked: &CheckedTrees,
        application: &ClosedConformanceApplication,
        selected_row: &ClosedConformanceRow,
        helpers: &[ForwardedHelperIds],
        next_block: &mut u64,
        next_operation: &mut u64,
        next_value: &mut u64,
        next_edge: &mut u64,
        source_calls: &mut [LoweredSourceCallOccurrence],
    ) -> Result<Vec<TerminalMachine>, LoweringError> {
        materialize_forwarded_helper_chain(
            checked,
            self,
            application,
            selected_row,
            helpers,
            next_block,
            next_operation,
            next_value,
            next_edge,
            source_calls,
        )
    }
}

impl JoinedDynamicCall for CheckedDynamicUnitCallPlan {
    type Helper = unit::ForwardedUnitHelperIds;

    fn view(&self) -> JoinedCallView<'_> {
        JoinedCallView {
            caller_machine: self.caller_machine,
            caller_state: self.caller_state,
            caller_attachment_type_identity: &self.caller_attachment_type_identity,
            caller_multiplicity: self.caller_multiplicity,
            caller_parameter_access: self.caller_parameter_access,
            coordinate: self.coordinate,
            source_parameter_position: self.source_parameter_position,
            source_access: self.source_access,
            source_path: &self.source_path,
            source_type_identity: &self.source_type_identity,
            requirement: self.requirement,
            realization_machine: self.realization_machine,
            realization_callables: &self.realization_callables,
            forwarding_transfers: &self.forwarding_transfers,
            forwarded_origin: match self.origin {
                CheckedDynamicUnitCallOrigin::Local => None,
                CheckedDynamicUnitCallOrigin::Forwarded {
                    state, coordinate, ..
                } => Some((state, coordinate)),
            },
        }
    }

    fn callable_table(&self) -> DynamicCallableTable<'_> {
        self.into()
    }

    fn validate_exact(&self, checked: &CheckedTrees) -> Result<(), LoweringError> {
        unit::validate_exact_unit_plan(checked, self, DynamicLoweringLane::Direct)
    }

    /// A Unit call binds no result.
    fn results_match(&self, _other: &Self) -> bool {
        true
    }

    /// Unit helpers only forward the descriptor to the next call.
    fn helper_bodies_match(&self, _other: &Self) -> bool {
        true
    }

    fn result_type(&self) -> Result<Option<ScalarType>, LoweringError> {
        Ok(None)
    }

    fn helper_ids(
        &self,
        realizations: &[LoweredDynamicRealization],
        next_block: &mut u64,
        next_operation: &mut u64,
        _next_value: &mut u64,
        next_edge: &mut u64,
    ) -> Result<Vec<unit::ForwardedUnitHelperIds>, LoweringError> {
        unit::forwarded_unit_helper_ids(self, realizations, next_block, next_operation, next_edge)
    }

    fn materialize_helpers(
        &self,
        checked: &CheckedTrees,
        application: &ClosedConformanceApplication,
        selected_row: &ClosedConformanceRow,
        helpers: &[unit::ForwardedUnitHelperIds],
        _next_block: &mut u64,
        _next_operation: &mut u64,
        _next_value: &mut u64,
        _next_edge: &mut u64,
        _source_calls: &mut [LoweredSourceCallOccurrence],
    ) -> Result<Vec<TerminalMachine>, LoweringError> {
        unit::materialize_forwarded_unit_helper_chain(
            checked,
            self,
            application,
            selected_row,
            helpers,
        )
    }
}

/// Lower one checked join: validate the control split and both branch calls,
/// lower their shared caller ABI and sources once, retain each branch's exact
/// conformance application, then emit the split caller, the realizations and
/// the lane's forwarded helper chain.
pub(super) fn lower<Call: JoinedDynamicCall>(
    checked: &CheckedTrees,
    control: &CheckedDynamicJoinControlPlan,
    when_true: &CheckedDynamicJoinBranchPlan<Call>,
    when_false: &CheckedDynamicJoinBranchPlan<Call>,
) -> Result<LoweredPsi, LoweringError> {
    validate_join_control_plan(checked, control, when_true, when_false)?;
    let branches = [&when_true.call, &when_false.call];
    for branch in branches {
        branch.validate_exact(checked)?;
    }
    let [first, second] = branches;
    let [first_view, second_view] = branches.map(|branch| branch.view());
    if first_view.caller_attachment_type_identity != second_view.caller_attachment_type_identity
        || first_view.caller_attachment_type_identity != control.caller_attachment_type_identity
        || first_view.source_type_identity != second_view.source_type_identity
        || first_view.source_access != second_view.source_access
        || first_view.caller_parameter_access != second_view.caller_parameter_access
        || first_view.caller_multiplicity != second_view.caller_multiplicity
        || !first.results_match(second)
    {
        return unsupported("joined dynamic branches do not share one caller ABI");
    }

    let (structural_types, type_ids) = lower_dynamic_structural_types_for_source(
        checked,
        &control.caller_attachment_type_identity,
        first_view.caller_attachment_type_identity,
        first_view.source_path,
        first_view.source_type_identity,
    )?;
    let caller_attachment = lookup_type_id(&type_ids, first_view.caller_attachment_type_identity)?;
    let source_type = lookup_type_id(&type_ids, first_view.source_type_identity)?;
    let caller_access = match first_view.caller_parameter_access {
        CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
        CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
        _ => return unsupported("joined dynamic caller requires borrowed self"),
    };
    let caller_self = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: true,
        structural_type: caller_attachment,
        multiplicity: terminal_structural_multiplicity(first_view.caller_multiplicity),
        access: caller_access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let sources = [
        lower_source(&caller_self, &first_view, &structural_types, &type_ids)?,
        lower_source(&caller_self, &second_view, &structural_types, &type_ids)?,
    ];

    let mut lowered_realizations = joined_realizations(checked, branches)?;
    for (index, realization) in lowered_realizations.iter_mut().enumerate() {
        realization.machine = machine_id(
            u64::try_from(index)
                .map_err(|_| LoweringError::Unsupported("joined realization count exceeds u64"))?
                .checked_add(2)
                .ok_or(LoweringError::Unsupported(
                    "joined realization identity overflowed",
                ))?,
        );
    }
    let first_realizations =
        realizations_for_plan(first_view.realization_callables, &lowered_realizations)?;
    let second_realizations =
        realizations_for_plan(second_view.realization_callables, &lowered_realizations)?;
    let caller_machine = machine_id(1);
    let (first_application, first_row) = lower_exact_application(
        checked,
        &first.callable_table(),
        caller_machine,
        &first_realizations,
    )?;
    let (second_application, second_row) = lower_exact_application(
        checked,
        &second.callable_table(),
        caller_machine,
        &second_realizations,
    )?;
    let (requirements, requirement_slot) =
        dynamic_parameter_interface(&first_application, &first_row)?;
    let (second_requirements, second_slot) =
        dynamic_parameter_interface(&second_application, &second_row)?;
    if requirements != second_requirements
        || requirement_slot != second_slot
        || first_application.trait_identity != second_application.trait_identity
    {
        return unsupported("joined dynamic conformances do not expose one exact interface");
    }

    // Values 2 and 3 bind the branch results of a scalar lane; helper and
    // realization identities follow the caller's three blocks, two branch
    // calls and four edges.
    let mut next_block = 4_u64;
    let mut next_place = 2_u64;
    let mut next_operation = 3_u64;
    let mut next_value = 2_u64;
    let mut next_edge = 5_u64;
    let result_type = first.result_type()?;
    let branch_results = [
        branch_result(result_type, &mut next_value)?,
        branch_result(result_type, &mut next_value)?,
    ];
    let helper_ids = first.helper_ids(
        &lowered_realizations,
        &mut next_block,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
    )?;
    let first_helper = helper_ids.first().ok_or(LoweringError::Unsupported(
        "joined dynamic control has no forwarded helper",
    ))?;
    let (first_helper_machine, first_helper_operation) =
        (first_helper.machine(), first_helper.operation());
    let [true_result, false_result] = branch_results;
    let caller_blocks = vec![
        Block {
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(1),
                when_true: terminal_psi::SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(1),
                    target: block_id(2),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: terminal_psi::SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        branch_block(
            block_id(2),
            operation_id(1),
            edge_id(3),
            first_helper_machine,
            true_result,
        ),
        branch_block(
            block_id(3),
            operation_id(2),
            edge_id(4),
            first_helper_machine,
            false_result,
        ),
    ];

    let mut realization_machines = Vec::new();
    for realization in &lowered_realizations {
        let owner = branches
            .into_iter()
            .find(|branch| {
                plan_contains_realization(branch.view().realization_callables, realization)
            })
            .ok_or(LoweringError::Unsupported(
                "joined realization has no checked branch owner",
            ))?;
        realization_machines.extend(materialize_dynamic_realizations(
            checked,
            &owner.callable_table(),
            std::slice::from_ref(realization),
            source_type,
            &structural_types,
            &mut next_block,
            &mut next_place,
            &mut next_operation,
            &mut next_value,
            &mut next_edge,
        )?);
    }

    let mut dynamic_dispatch = TerminalDynamicDispatchCatalog {
        parameters: vec![TerminalDynamicDescriptorParameter {
            owner: first_helper_machine,
            ordinal: 0,
            source_position: 0,
            trait_identity: first_application.trait_identity.clone(),
            access: sources[0].access,
            requirements,
        }],
        arguments: vec![
            TerminalDynamicDescriptorArgument {
                owner: caller_machine,
                operation: operation_id(1),
                parameter_ordinal: 0,
                source: TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
            },
            TerminalDynamicDescriptorArgument {
                owner: caller_machine,
                operation: operation_id(2),
                parameter_ordinal: 0,
                source: TerminalDynamicDescriptorSource::Selection { ordinal: 1 },
            },
        ],
        selections: vec![
            TerminalDynamicConformanceSelection {
                owner: caller_machine,
                ordinal: 0,
                source: sources[0].clone(),
                conformance_application_report_fingerprint: first_application.report_fingerprint,
                conformance_application_commitment: first_application.commitment,
            },
            TerminalDynamicConformanceSelection {
                owner: caller_machine,
                ordinal: 1,
                source: sources[1].clone(),
                conformance_application_report_fingerprint: second_application.report_fingerprint,
                conformance_application_commitment: second_application.commitment,
            },
        ],
        rebound_descriptors: Vec::new(),
        stored_descriptors: Vec::new(),
        direct_dispatches: Vec::new(),
        indirect_dispatches: Vec::new(),
        stored_dispatches: Vec::new(),
        parameter_dispatches: vec![TerminalParameterDynamicDispatch {
            owner: first_helper_machine,
            operation: first_helper_operation,
            parameter_ordinal: 0,
            requirement_slot,
        }],
    };
    extend_parameter_forwarding_catalog(&mut dynamic_dispatch, &helper_ids)?;
    let mut source_call_occurrences = joined_source_call_occurrences(first, second, &helper_ids)?;
    let helpers = first.materialize_helpers(
        checked,
        &first_application,
        &first_row,
        &helper_ids,
        &mut next_block,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
        &mut source_call_occurrences,
    )?;
    let mut applications = vec![first_application, second_application];
    applications.sort_by(|left, right| {
        (
            left.owner,
            left.declaration_identity.as_str(),
            left.report_fingerprint,
        )
            .cmp(&(
                right.owner,
                right.declaration_identity.as_str(),
                right.report_fingerprint,
            ))
    });
    applications.dedup();

    let caller_reach = lower_installation_machine_service_ceiling(
        checked,
        first_view.caller_machine,
        checked
            .facts
            .service_reaches
            .plan_for_machine(first_view.caller_machine)
            .ok_or(LoweringError::Unsupported(
                "joined dynamic caller has no checked service contract",
            ))?,
        exact_machine_service_summary(checked, first_view.caller_machine)?,
        &[],
    )?;
    let root_service_reach = lower_root_service_reach(checked, first_view.caller_machine, &[])?;
    let mut machines = vec![TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: caller_machine,
        attachment: Some(caller_attachment),
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(1),
            scalar_type: ScalarType::Boolean,
        }],
        structural_parameters: vec![caller_self.clone()],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: caller_self.place,
            kind: StructuralPlaceKind::Parameter {
                position: caller_self.position,
                is_self: caller_self.is_self,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: caller_reach,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: caller_blocks,
        contract: empty_terminal_contract(caller_machine.get()),
    }];
    machines.extend(realization_machines);
    machines.extend(helpers);
    machines.sort_by_key(|machine| machine.id);

    Ok(LoweredPsi {
        semantic_module: TerminalModule {
            structural_types,
            root_service_reach,
            closed_conformance_applications: applications,
            dynamic_dispatch,
            machines,
            ..TerminalModule::for_entry(caller_machine)
        },
        proof_bundle: ProofBundle {
            crash_obligations: Vec::new(),
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        },
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences: Vec::new(),
        selected_ieee_float_comparison_occurrences: Vec::new(),
        selected_integer_comparison_occurrences: Vec::new(),
    })
}

/// The joined caller is the `when_true` branch's caller; both branches must
/// name it, and the control plan's split must enter exactly those states.
fn validate_join_control_plan<Call: JoinedDynamicCall>(
    checked: &CheckedTrees,
    control: &CheckedDynamicJoinControlPlan,
    when_true: &CheckedDynamicJoinBranchPlan<Call>,
    when_false: &CheckedDynamicJoinBranchPlan<Call>,
) -> Result<(), LoweringError> {
    let (true_call, false_call) = (when_true.call.view(), when_false.call.view());
    let caller_machine = true_call.caller_machine;
    let (when_true, when_false) = (&when_true.successor, &when_false.successor);
    let entry_state = control.entry_state;
    if when_true.target_state != true_call.caller_state
        || when_false.target_state != false_call.caller_state
        || false_call.caller_machine != caller_machine
        || when_true.statement_ordinal != 0
        || when_false.statement_ordinal != 1
        || !when_true.transfers.is_empty()
        || !when_false.transfers.is_empty()
        || !when_true.scalar_arguments.is_empty()
        || !when_false.scalar_arguments.is_empty()
        || !when_true
            .trivial_affine_discard_parameter_positions
            .is_empty()
        || !when_false
            .trivial_affine_discard_parameter_positions
            .is_empty()
    {
        return unsupported("joined dynamic control plan drifted from checked custody");
    }
    let [parameter] = control.scalar_parameters.as_slice() else {
        return unsupported("joined dynamic control requires one Boolean parameter");
    };
    if parameter.source_position != 1
        || parameter.primitive_type != PrimitiveType::Bool
        || !matches!(
            &control.guard,
            CheckedScalarExpression::Boolean(boolean)
                if matches!(boolean.as_ref(), CheckedBooleanExpression::Parameter { position: 0 })
        )
    {
        return unsupported("joined dynamic control guard drifted from its Boolean input");
    }
    let states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, state)| {
            (state.machine_symbol == caller_machine && state.state_symbol == entry_state)
                .then_some(state)
        })
        .collect::<Vec<_>>();
    let [entry] = states.as_slice() else {
        return unsupported("joined dynamic control lost its exact entry state");
    };
    let calls = checked.facts.flow.control.calls.span_or_empty(entry.calls);
    if calls.len() != 2
        || [
            (0_usize, when_true.target_state),
            (1_usize, when_false.target_state),
        ]
        .into_iter()
        .any(|(statement_index, target)| {
            calls
                .iter()
                .filter(|call| {
                    call.statement_index == statement_index
                        && call.call_ordinal == 0
                        && !call.has_receiver
                        && call.target_symbol == target
                })
                .count()
                != 1
        })
    {
        return unsupported("joined dynamic control successors drifted from checked flow");
    }
    Ok(())
}

fn lower_source(
    caller_self: &StructuralParameterDeclaration,
    call: &JoinedCallView<'_>,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<StructuralArgument, LoweringError> {
    validate_and_lower_dynamic_source(
        caller_self,
        call.source_parameter_position,
        call.caller_parameter_access,
        call.caller_multiplicity,
        call.source_access,
        call.caller_attachment_type_identity,
        call.source_path,
        call.source_type_identity,
        structural_types,
        type_ids,
    )
}

fn joined_realizations<Call: JoinedDynamicCall>(
    checked: &CheckedTrees,
    branches: [&Call; 2],
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    let mut joined = Vec::new();
    for branch in branches {
        for candidate in collect_dynamic_realizations(checked, &branch.callable_table(), 2)? {
            if let Some(existing) = joined.iter().find(|existing: &&LoweredDynamicRealization| {
                existing.source_machine == candidate.source_machine
                    && existing.source_state == candidate.source_state
                    && existing.callable_identity == candidate.callable_identity
            }) {
                if existing.result != candidate.result {
                    return unsupported("joined dynamic realization result drifted");
                }
            } else {
                joined.push(candidate);
            }
        }
    }
    if joined.is_empty() {
        return unsupported("joined dynamic plan has no realizations");
    }
    Ok(joined)
}

fn realizations_for_plan(
    callables: &[CheckedDynamicRealizationCallablePlan],
    joined: &[LoweredDynamicRealization],
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    let retained = joined
        .iter()
        .filter(|realization| plan_contains_realization(callables, realization))
        .cloned()
        .collect::<Vec<_>>();
    if retained.len() != callables.len() {
        return unsupported("joined conformance realization roster is incomplete");
    }
    Ok(retained)
}

fn plan_contains_realization(
    callables: &[CheckedDynamicRealizationCallablePlan],
    realization: &LoweredDynamicRealization,
) -> bool {
    callables.iter().any(|callable| {
        callable.realization_machine == realization.source_machine
            && callable.realization_state == realization.source_state
            && callable.realization_identity == realization.checked_identity
    })
}

/// A scalar lane binds each branch call's result to a fresh value; a Unit
/// lane binds none.
fn branch_result(
    result_type: Option<ScalarType>,
    next_value: &mut u64,
) -> Result<Option<ValueDeclaration>, LoweringError> {
    result_type
        .map(|scalar_type| {
            Ok(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(next_value)?),
                scalar_type,
            })
        })
        .transpose()
}

/// One predecessor: the branch's call into the first forwarded helper, then
/// the caller's Unit return.
fn branch_block(
    block: semantic_vocabulary::BlockId,
    operation: semantic_vocabulary::OperationId,
    edge: semantic_vocabulary::EdgeId,
    callee: MachineId,
    result: Option<ValueDeclaration>,
) -> Block {
    let (result, kind) = match result {
        Some(value) => (
            OperationResult::Scalar(value),
            OperationKind::CallStructuralScalar {
                callee,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ),
        None => (
            OperationResult::Unit,
            OperationKind::CallUnit {
                callee,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ),
    };
    Block {
        structural_parameters: Vec::new(),
        id: block,
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: vec![Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result,
            kind,
        }],
        terminator: Terminator::ReturnUnit {
            edge,
            trivial_affine_discards: Vec::new(),
        },
    }
}

fn joined_source_call_occurrences<Call: JoinedDynamicCall>(
    when_true: &Call,
    when_false: &Call,
    helpers: &[Call::Helper],
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    let (true_call, false_call) = (when_true.view(), when_false.view());
    if helpers.len() != true_call.forwarding_transfers.len() + 1
        || true_call.forwarding_transfers != false_call.forwarding_transfers
        || !when_true.helper_bodies_match(when_false)
    {
        return unsupported("joined source-call helper chain drifted from checked custody");
    }
    let join_state = true_call
        .forwarding_transfers
        .first()
        .map(|transfer| transfer.caller_state);
    let mut occurrences = [
        (&true_call, operation_id(1)),
        (&false_call, operation_id(2)),
    ]
    .into_iter()
    .map(|(branch, operation)| {
        let Some((state, _)) = branch.forwarded_origin else {
            return unsupported("joined branch lost its forwarded source target");
        };
        Ok(LoweredSourceCallOccurrence {
            source_site: None,
            source_state: branch.caller_state,
            statement_index: usize::try_from(branch.coordinate.statement_index)
                .map_err(|_| LoweringError::Unsupported("joined call statement exceeds usize"))?,
            call_ordinal: usize::try_from(branch.coordinate.call_ordinal)
                .map_err(|_| LoweringError::Unsupported("joined call ordinal exceeds usize"))?,
            terminal_operation: operation,
            source_target: join_state.unwrap_or(state),
            source_values_before_call: Vec::new(),
        })
    })
    .collect::<Result<Vec<_>, _>>()?;
    for (transfer, helper) in true_call.forwarding_transfers.iter().zip(helpers) {
        occurrences.push(LoweredSourceCallOccurrence {
            source_site: None,
            source_state: transfer.caller_state,
            statement_index: usize::try_from(transfer.coordinate.statement_index).map_err(
                |_| LoweringError::Unsupported("joined forwarding statement exceeds usize"),
            )?,
            call_ordinal: usize::try_from(transfer.coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("joined forwarding call ordinal exceeds usize")
            })?,
            terminal_operation: helper.operation(),
            source_target: transfer.target_state,
            source_values_before_call: Vec::new(),
        });
    }
    let Some((state, coordinate)) = true_call.forwarded_origin else {
        return unsupported("joined dispatch lost its forwarded source coordinate");
    };
    occurrences.push(LoweredSourceCallOccurrence {
        source_site: None,
        source_state: state,
        statement_index: usize::try_from(coordinate.statement_index)
            .map_err(|_| LoweringError::Unsupported("joined dispatch statement exceeds usize"))?,
        call_ordinal: usize::try_from(coordinate.call_ordinal)
            .map_err(|_| LoweringError::Unsupported("joined dispatch ordinal exceeds usize"))?,
        terminal_operation: helpers
            .last()
            .ok_or(LoweringError::Unsupported(
                "joined source-call chain has no final helper",
            ))?
            .operation(),
        source_target: true_call.requirement,
        source_values_before_call: Vec::new(),
    });
    Ok(occurrences)
}

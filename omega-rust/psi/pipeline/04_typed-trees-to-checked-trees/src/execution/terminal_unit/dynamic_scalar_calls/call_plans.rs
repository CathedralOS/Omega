//! One checked dynamic call, for either result.
//!
//! A caller selects a descriptor from one attachment field (directly, rebound
//! once, or stored through one aggregate field) and calls a requirement
//! through it, in its own body or at the end of a forwarded helper chain.
//! Everything but the result is the same for a scalar and a Unit call: the
//! call's ordinary shape and receiver, the selection, the borrowed source
//! argument, the selected conformance row, requirement and realization, the
//! callable roster, both contracts and the call's service reach.
//! [`build_checked_dynamic_call`] checks that custody once. A
//! [`DynamicResultLane`] supplies what its result adds: the caller statement
//! that consumes the result, the declared return, the selected body, the
//! forwarded helper bodies, and its own publication. Checks only one lane
//! needs stay in that lane: the Unit rung refuses a stored descriptor and any
//! service reach, and the scalar lane owns its caller store and Unit
//! continuation.

use super::forwarded_calls::{ForwardedDynamicCall, forwarded_transfer_path_is_exact};
use super::realization_bodies::checked_call_service_reach;
use super::realization_callables::{
    checked_dynamic_realization_callables, dynamic_family_realization, dynamic_family_tuple,
};
use super::receivers::{DynamicReceiverPlace, dynamic_receiver_place, statement_receiver_place};
use super::result_lanes::DynamicResultLane;
use crate::execution::terminal_unit::types::{
    ShapeCollector, machine_binders, structural_access_for_type_reference, terminal_field_identity,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedStructuralAccess, CheckedUnitCallCoordinate,
    CheckedUnitStructuralPathSegment, ExpressionNode, MachineSupplyMode, ServiceReachSummary,
    StatementNode, SymbolHandle, TypeReferenceNode, TypedTrees,
};
use crate::semantic::calls::CallSite;
use checked_trees::{CheckedDynamicBinding, CheckedDynamicDispatchPlan};
use typed_trees::name::Identifier;
use typed_trees::type_identity::TypeIdentityRequest;

/// The parts of one authored call a dynamic lane reads, whichever form the
/// call's result gives it: a Unit call is a statement, a scalar call an
/// expression that a local binds.
pub(super) struct AuthoredCall<'program> {
    pub(super) target_symbol: SymbolHandle,
    pub(super) target: &'program Identifier,
    pub(super) machine_arguments: &'program [typed_trees::expression::StaticMachineArgument],
    pub(super) argument_count: usize,
    /// No evidence term names one of the callee's obligations.
    pub(super) evidence_free: bool,
    /// The call runs its target's ordinary nominal route and keeps its result:
    /// no static requirement dispatch, quotient or private layout request, and
    /// no `_ =` discard.
    pub(super) ordinary_route: bool,
    receiver: AuthoredReceiver<'program>,
}

#[derive(Clone, Copy)]
enum AuthoredReceiver<'program> {
    Expression(typed_trees::expression::ExpressionHandle),
    Statement(&'program typed_trees::statement::TableCall),
}

impl<'program> AuthoredCall<'program> {
    pub(super) fn of(program: &'program TypedTrees, site: &CallSite<'program>) -> Option<Self> {
        match *site {
            CallSite::Statement(call) => Some(Self {
                target_symbol: call.target_symbol,
                target: &call.target,
                machine_arguments: &call.machine_arguments,
                argument_count: program
                    .statement_table
                    .expression_handles(call.arguments)
                    .len(),
                evidence_free: call.evidence_arguments.is_empty(),
                ordinary_route: call.static_requirement_dispatch.is_none() && !call.discards_result,
                receiver: AuthoredReceiver::Statement(call),
            }),
            CallSite::Expression { call, .. } => Some(Self {
                target_symbol: call.target_symbol,
                target: &call.target,
                machine_arguments: &call.machine_arguments,
                argument_count: program
                    .expression_table
                    .expression_handles(call.arguments)
                    .len(),
                evidence_free: call.evidence_arguments.is_empty(),
                ordinary_route: call.selects_only_nominal_route(),
                receiver: AuthoredReceiver::Expression(call.receiver),
            }),
            CallSite::TransitionNamed { .. } => None,
        }
    }

    /// The place the call dispatches through.
    fn receiver_place(&self, program: &TypedTrees) -> Option<DynamicReceiverPlace> {
        match self.receiver {
            AuthoredReceiver::Expression(receiver) => dynamic_receiver_place(program, receiver),
            AuthoredReceiver::Statement(call) => statement_receiver_place(program, call),
        }
    }
}

/// The final helper of a forwarded call: its machine and state, the
/// coordinate of its dispatching call and the descriptor parameter it calls
/// through. Both plans' origins record exactly this.
#[derive(Clone, Copy)]
pub(super) struct ForwardedDispatch {
    pub(super) machine: SymbolHandle,
    pub(super) state: SymbolHandle,
    pub(super) coordinate: CheckedUnitCallCoordinate,
    pub(super) parameter: SymbolHandle,
}

/// The custody both lanes' plans share, gathered once by
/// [`build_checked_dynamic_call`]. A lane moves it into its plan.
pub(super) struct DynamicCallCustody<HelperBody> {
    /// The final forwarded helper; `None` for a local call.
    pub(super) forwarded: Option<ForwardedDispatch>,
    pub(super) forwarding_transfers: Vec<checked_trees::CheckedDynamicDescriptorTransferPlan>,
    /// The forwarded helper bodies, outermost first; empty for a local call.
    pub(super) forwarding_helpers: Vec<HelperBody>,
    pub(super) caller_machine: SymbolHandle,
    pub(super) caller_state: SymbolHandle,
    pub(super) caller_attachment_type_identity: String,
    pub(super) caller_multiplicity: language_semantics::Multiplicity,
    pub(super) caller_parameter_access: CheckedStructuralAccess,
    pub(super) caller_contract_report_fingerprint: u64,
    pub(super) caller_contract_commitment: checked_trees::MachineContractCommitment,
    pub(super) caller_service_reach: ServiceReachSummary,
    pub(super) coordinate: CheckedUnitCallCoordinate,
    pub(super) receiver_binding: SymbolHandle,
    pub(super) selection: checked_trees::DynamicConformanceBindingFact,
    pub(super) source_parameter_position: u32,
    pub(super) source_access: CheckedStructuralAccess,
    pub(super) source_field: SymbolHandle,
    pub(super) source_path: Vec<CheckedUnitStructuralPathSegment>,
    pub(super) source_type_identity: String,
    pub(super) source_multiplicity: language_semantics::Multiplicity,
    pub(super) target_trait: SymbolHandle,
    pub(super) selected_conformance: SymbolHandle,
    pub(super) declaring_trait: SymbolHandle,
    pub(super) requirement: SymbolHandle,
    pub(super) requirement_identity: String,
    pub(super) realization_machine: SymbolHandle,
    pub(super) realization_state: SymbolHandle,
    pub(super) realization_identity: String,
    pub(super) family_tuple: Box<[String]>,
    pub(super) realization_callables: Vec<checked_trees::CheckedDynamicRealizationCallablePlan>,
    pub(super) realization_contract_report_fingerprint: u64,
    pub(super) realization_contract_commitment: checked_trees::MachineContractCommitment,
    pub(super) checked_call_service_reach: ServiceReachSummary,
}

/// The caller a lane publishes its plan for: what the scalar lane's caller
/// store and Unit continuation read beyond the shared custody.
pub(super) struct DynamicCaller<'program, 'facts> {
    pub(super) program: &'program TypedTrees,
    pub(super) facts: &'facts CheckFacts,
    pub(super) boundaries: &'facts [CheckedBoundaryMachinePlan],
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) statements: &'program [StatementNode],
    pub(super) stored: Option<&'facts checked_trees::DynamicDescriptorStorageFact>,
    pub(super) source_definition: &'program typed_trees::data::DataDefinition,
}

/// Build one checked dynamic call under its result lane. The call either
/// dispatches in `state` itself or, when `forwarded` names a helper chain, in
/// the chain's final helper; `stored` names the aggregate field a local call's
/// descriptor travels through. Any fact the call cannot describe omits it.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_checked_dynamic_call<'program, 'facts, Lane: DynamicResultLane>(
    program: &'program TypedTrees,
    facts: &'facts CheckFacts,
    binding_facts: &checked_trees::DynamicConformanceBindingFacts,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    flow_call: &checked_trees::FlowCallFact,
    call_site: CallSite<'program>,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &'facts [CheckedBoundaryMachinePlan],
    forwarded: Option<ForwardedDynamicCall<'program, '_, Lane::HelperBody>>,
    stored: Option<&'facts checked_trees::DynamicDescriptorStorageFact>,
) -> Option<CheckedDynamicDispatchPlan> {
    if !Lane::authors(&call_site) || (stored.is_some() && !Lane::STORES_DESCRIPTORS) {
        return None;
    }
    let coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(flow_call.statement_index).ok()?,
        call_ordinal: u32::try_from(flow_call.call_ordinal).ok()?,
    };
    let forwarded_selection = forwarded
        .as_ref()
        .and_then(|forwarded| forwarded.transfer.sole_selection().cloned());
    let mut forwarding_transfers = Vec::new();
    let mut forwarding_helpers = Vec::new();
    let mut forwarded_parameter_type = None;
    // A forwarded call dispatches in its final helper, through the one plain
    // parameter the last transfer fills; a local call dispatches here.
    let (
        dispatch_state,
        dispatch_flow_call,
        dispatch_site,
        selection_binding,
        selection_name,
        forwarded_dispatch,
    ) = match forwarded {
        Some(forwarded) => {
            if stored.is_some()
                || !Lane::authors(&forwarded.call_site)
                || !forwarded_transfer_path_is_exact(&forwarded)
            {
                return None;
            }
            let [parameter] = program.state_parameters(forwarded.state) else {
                return None;
            };
            let forwarded_parameter = forwarded
                .prior_transfers
                .last()
                .map(|transfer| transfer.parameter)
                .unwrap_or(forwarded.transfer.parameter);
            if parameter.is_self || parameter.is_const || parameter.symbol != forwarded_parameter {
                return None;
            }
            forwarded_parameter_type = Some(parameter.type_reference);
            let dispatch = ForwardedDispatch {
                machine: forwarded.machine.symbol,
                state: forwarded.state.symbol,
                coordinate: CheckedUnitCallCoordinate {
                    statement_index: u32::try_from(forwarded.flow_call.statement_index).ok()?,
                    call_ordinal: u32::try_from(forwarded.flow_call.call_ordinal).ok()?,
                },
                parameter: forwarded.flow_call.receiver_symbol,
            };
            let selection_name = forwarded.transfer.sole_selection()?.binding_name.clone();
            forwarding_transfers = forwarded.prior_transfers;
            forwarding_helpers = forwarded.helpers;
            (
                forwarded.state,
                forwarded.flow_call,
                forwarded.call_site,
                forwarded.transfer.source_binding,
                selection_name,
                Some(dispatch),
            )
        }
        None => {
            let (selection_binding, selection_name) = stored
                .map(|storage| {
                    (
                        storage.selection.binding,
                        storage.selection.binding_name.clone(),
                    )
                })
                .unwrap_or((flow_call.receiver_symbol, Identifier::default()));
            (
                state,
                flow_call,
                call_site,
                selection_binding,
                selection_name,
                None,
            )
        }
    };
    let dispatch_call = AuthoredCall::of(program, &dispatch_site)?;
    if coordinate.call_ordinal != 0
        || dispatch_flow_call.call_ordinal != 0
        || !dispatch_flow_call.has_receiver
        || !dispatch_flow_call.receiver_symbol.is_valid()
        || !dispatch_flow_call.target_symbol.is_valid()
        || dispatch_call.target_symbol != dispatch_flow_call.target_symbol
        || !dispatch_call.ordinary_route
        || dispatch_call.argument_count != 0
        || !dispatch_call.evidence_free
    {
        return None;
    }

    let receiver_place = dispatch_call.receiver_place(program)?;
    let receiver_name = receiver_place.path.last()?;
    let expected_selection_name = if selection_name.as_str().is_empty() {
        receiver_name
    } else {
        &selection_name
    };
    match stored {
        Some(storage) => {
            if receiver_place.root != storage.destination_binding
                || (receiver_place.leaf.is_valid()
                    && receiver_place.leaf != storage.destination_field)
                || receiver_place.path != storage.destination_path
                || storage.destination_field != dispatch_flow_call.receiver_symbol
                || storage.selection.binding != selection_binding
            {
                return None;
            }
        }
        None => {
            if receiver_place.path.len() != 1
                || receiver_place.leaf != dispatch_flow_call.receiver_symbol
                || receiver_place.root != receiver_place.leaf
            {
                return None;
            }
        }
    }

    let statements = program.statement_table.statements(state.statement_nodes);
    let lane = Lane::caller_result(program, statements, flow_call, &call_site)?;
    if !lane.admits_helper_bodies(&forwarding_helpers) {
        return None;
    }

    let mut selections = binding_facts
        .selections
        .iter()
        .filter(|selection| {
            selection.machine == machine.symbol
                && selection.state == state.symbol
                && selection.binding == selection_binding
                && selection.binding_name == *expected_selection_name
                && selection.statement_index < flow_call.statement_index
        })
        .collect::<Vec<_>>();
    selections.sort_by_key(|selection| selection.statement_index);
    if selections
        .windows(2)
        .any(|pair| pair[0].statement_index >= pair[1].statement_index)
    {
        return None;
    }
    let (rebound_from, selection) = match selections.as_slice() {
        [selection] => (None, *selection),
        [initial, rebound] => (Some(*initial), *rebound),
        _ => return None,
    };
    if let Some(forwarded_selection) = forwarded_selection.as_ref()
        && selection != forwarded_selection
    {
        return None;
    }
    let selection = selection.clone();
    let selected_conformance = selection.conformance.filter(|symbol| symbol.is_valid())?;

    let (source_parameter_position, caller_parameter_access, source_access) =
        checked_source_argument(program, facts, state, statements, &selection)?;
    // The final helper's parameter lends the descriptor with the selection's
    // own access.
    if forwarded_parameter_type.is_some_and(|type_reference| {
        structural_access_for_type_reference(program, type_reference) != Some(source_access)
    }) {
        return None;
    }
    let attachment = sole(
        program
            .data_definitions()
            .iter()
            .filter(|data| data.symbol == machine.attached_data_symbol),
    )?;
    let caller_attachment_type_identity =
        shapes.add_attached_data(attachment, &machine_binders(program, machine))?;
    let (source_field, source_path, source_type_identity) =
        checked_self_attachment_source(program, machine, &selection)?;
    let source_definition = sole(
        program
            .data_definitions()
            .iter()
            .filter(|data| data.symbol == selection.source_data),
    )?;
    let rebound_from = match rebound_from {
        Some(initial) => Some(checked_rebound_dynamic_selection(
            program,
            facts,
            machine,
            state,
            statements,
            flow_call.statement_index,
            initial,
            &selection,
            source_parameter_position,
            caller_parameter_access,
            source_access,
            &source_type_identity,
        )?),
        None => None,
    };

    let target_trait = sole(
        program
            .traits()
            .iter()
            .filter(|definition| definition.symbol == selection.target_trait),
    )?;
    let conformance = sole(
        program
            .conformances()
            .iter()
            .filter(|conformance| conformance.symbol == selected_conformance),
    )?;
    if conformance.trait_name != target_trait.name {
        return None;
    }

    let row = sole(
        selection
            .rows
            .iter()
            .filter(|row| row.requirement == dispatch_flow_call.target_symbol),
    )?
    .clone();
    if row.requirement_identity.is_empty()
        || row.realization_identity.is_empty()
        || program.symbols.name(row.requirement) != dispatch_call.target.as_str()
    {
        return None;
    }

    let declaring_trait = sole(
        program
            .traits()
            .iter()
            .filter(|definition| definition.symbol == row.declaring_trait),
    )?;
    let requirement = sole(
        program
            .trait_machine_signatures(declaring_trait)
            .iter()
            .filter(|requirement| requirement.symbol == row.requirement),
    )?;
    let [requirement_self] = program.state_signature_parameters(requirement) else {
        return None;
    };
    if program
        .normalized_trait_requirement_overload_identity(declaring_trait, requirement)
        .identity()
        != row.requirement_identity
        || !lane.returns(program, requirement.return_type)
        || !requirement_self.is_self
        || structural_access_for_type_reference(program, requirement_self.type_reference)
            != Some(source_access)
    {
        return None;
    }
    let family_tuple = dynamic_family_tuple(program, requirement, dispatch_call.machine_arguments)?;

    let closed_row = sole(
        program
            .closed_conformance_rows(conformance)
            .unwrap_or_default()
            .iter()
            .filter(|candidate| {
                candidate.declaring_trait == row.declaring_trait
                    && candidate.requirement == row.requirement
                    && candidate.realization_machine == row.realization_machine
                    && candidate.realization_state == row.realization_state
            }),
    )?;
    let normalized = crate::facts::normalized_dynamic_row_identities(program, closed_row).ok()?;
    if normalized.0 != row.requirement_identity || normalized.1 != row.realization_identity {
        return None;
    }

    let row_realization_machine = sole(
        program
            .machines()
            .iter()
            .filter(|candidate| candidate.symbol == row.realization_machine),
    )?;
    if row_realization_machine.supply_mode != MachineSupplyMode::CheckedBody
        || row_realization_machine.attached_data_symbol != selection.source_data
        || program
            .normalized_machine_overload_identity(row_realization_machine)?
            .identity()
            != row.realization_identity
    {
        return None;
    }
    let row_realization_state = sole(
        program
            .machine_states(row_realization_machine)
            .iter()
            .filter(|candidate| candidate.symbol == row.realization_state),
    )?;
    let (realization_machine, realization_state, realization_identity) =
        dynamic_family_realization(
            program,
            row_realization_machine,
            row_realization_state,
            row.realization_identity.clone(),
            &family_tuple,
        )?;
    let [realization_self] = program.state_parameters(realization_state) else {
        return None;
    };
    if realization_machine.supply_mode != MachineSupplyMode::CheckedBody
        || realization_machine.attached_data_symbol != selection.source_data
        || !lane.returns(program, realization_state.return_type)
        || !realization_self.is_self
        || structural_access_for_type_reference(program, realization_self.type_reference)
            != Some(source_access)
    {
        return None;
    }
    let selected_body =
        lane.selected_body(program, facts, realization_machine, realization_state)?;

    let contract = facts
        .contract_plans
        .for_machine(realization_machine.symbol)?;
    if contract.report_fingerprint == 0 || contract.commitment.is_zero() {
        return None;
    }
    let realization_callables = checked_dynamic_realization_callables(
        program,
        facts,
        conformance,
        &selection,
        source_access,
    )?;
    let dispatch_coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(dispatch_flow_call.statement_index).ok()?,
        call_ordinal: u32::try_from(dispatch_flow_call.call_ordinal).ok()?,
    };
    let checked_call_service_reach = checked_call_service_reach(
        facts,
        dispatch_state.symbol,
        dispatch_flow_call,
        dispatch_coordinate,
    )?;
    let caller_contract = facts.contract_plans.for_machine(machine.symbol)?;
    if caller_contract.report_fingerprint == 0 || caller_contract.commitment.is_zero() {
        return None;
    }
    let caller_reach_fact = facts.service_reaches.for_machine(machine.symbol)?;
    let caller_service_reach = ServiceReachSummary {
        direct: caller_reach_fact.inferred_direct,
        transitive: caller_reach_fact.inferred_transitive,
    };
    if !Lane::admits_service_reach(
        &facts.service_reaches.rows,
        checked_call_service_reach,
        caller_service_reach,
    ) {
        return None;
    }

    let custody = DynamicCallCustody {
        forwarded: forwarded_dispatch,
        forwarding_transfers,
        forwarding_helpers,
        caller_machine: machine.symbol,
        caller_state: state.symbol,
        caller_attachment_type_identity,
        caller_multiplicity: attachment.properties.multiplicity,
        caller_parameter_access,
        caller_contract_report_fingerprint: caller_contract.report_fingerprint,
        caller_contract_commitment: caller_contract.commitment,
        caller_service_reach,
        coordinate,
        receiver_binding: selection_binding,
        selection,
        source_parameter_position,
        source_access,
        source_field,
        source_path,
        source_type_identity,
        source_multiplicity: source_definition.properties.multiplicity,
        target_trait: target_trait.symbol,
        selected_conformance,
        declaring_trait: row.declaring_trait,
        requirement: row.requirement,
        requirement_identity: row.requirement_identity,
        realization_machine: realization_machine.symbol,
        realization_state: realization_state.symbol,
        realization_identity,
        family_tuple,
        realization_callables,
        realization_contract_report_fingerprint: contract.report_fingerprint,
        realization_contract_commitment: contract.commitment,
        checked_call_service_reach,
    };
    let caller = DynamicCaller {
        program,
        facts,
        boundaries,
        machine,
        state,
        statements,
        stored,
        source_definition,
    };
    // A stored descriptor carries exactly the call's one selection. The lane
    // publishes first so the checks run in their established order.
    let stored_selection_matches =
        stored.is_none_or(|storage| storage.selection == custody.selection);
    let call = lane.publish(&caller, shapes, custody, selected_body)?;
    let binding = match (stored, rebound_from) {
        (Some(storage), None) if stored_selection_matches => {
            let StatementNode::LocalData(destination) = statements.get(storage.statement_index)?
            else {
                return None;
            };
            if destination.symbol != storage.destination_binding {
                return None;
            }
            let destination_type_identity = program
                .type_identity(TypeIdentityRequest {
                    binders: &machine_binders(program, machine),
                    ..TypeIdentityRequest::ordinary(destination.type_reference)
                })
                .into_string();
            let destination_field_identity =
                terminal_field_identity(program, storage.destination_field)?;
            CheckedDynamicBinding::Stored {
                descriptor: checked_trees::CheckedDynamicStoredDescriptorPlan {
                    storage: storage.clone(),
                    destination_type_identity,
                    destination_field_identity,
                },
                call,
            }
        }
        (Some(_), _) => return None,
        (None, Some(initial)) => CheckedDynamicBinding::Rebound {
            initial,
            latest: call,
        },
        (None, None) => CheckedDynamicBinding::Direct(call),
    };
    Some(Lane::dispatch_plan(binding))
}

/// The one element an exact lookup must find.
fn sole<T>(mut candidates: impl Iterator<Item = T>) -> Option<T> {
    let candidate = candidates.next()?;
    candidates.next().is_none().then_some(candidate)
}

#[allow(clippy::too_many_arguments)]
fn checked_rebound_dynamic_selection(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    call_statement_index: usize,
    initial: &checked_trees::DynamicConformanceBindingFact,
    rebound: &checked_trees::DynamicConformanceBindingFact,
    source_parameter_position: u32,
    caller_parameter_access: CheckedStructuralAccess,
    source_access: CheckedStructuralAccess,
    source_type_identity: &str,
) -> Option<checked_trees::CheckedDynamicSelectionPlan> {
    if initial.statement_index.checked_add(1)? != rebound.statement_index
        || rebound.statement_index.checked_add(1)? != call_statement_index
        || initial.binding != rebound.binding
        || initial.binding_name != rebound.binding_name
        || initial.machine != rebound.machine
        || initial.state != rebound.state
        || initial.source_data != rebound.source_data
        || initial.target_trait != rebound.target_trait
        || initial.conformance.is_none()
        || rebound.conformance.is_none()
    {
        return None;
    }
    let (initial_position, initial_caller_access, initial_source_access) =
        checked_source_argument(program, facts, state, statements, initial)?;
    if initial_position != source_parameter_position
        || initial_caller_access != caller_parameter_access
        || initial_source_access != source_access
    {
        return None;
    }
    let (source_field, source_path, initial_source_type_identity) =
        checked_self_attachment_source(program, machine, initial)?;
    if initial_source_type_identity != source_type_identity {
        return None;
    }
    Some(checked_trees::CheckedDynamicSelectionPlan {
        fact: initial.clone(),
        field: source_field,
        path: source_path,
        type_identity: initial_source_type_identity,
    })
}

fn checked_source_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    selection: &checked_trees::DynamicConformanceBindingFact,
) -> Option<(u32, CheckedStructuralAccess, CheckedStructuralAccess)> {
    let self_parameters = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self)
        .collect::<Vec<_>>();
    let [(source_parameter_position, self_parameter)] = self_parameters.as_slice() else {
        return None;
    };
    let source_parameter_position = u32::try_from(*source_parameter_position).ok()?;
    let root_access = structural_access_for_type_reference(program, self_parameter.type_reference)?;
    if !matches!(
        root_access,
        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
    ) {
        return None;
    }

    let local_declarations = statements
        .iter()
        .take(selection.statement_index.saturating_add(1))
        .filter_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == selection.binding).then_some(local)
        })
        .collect::<Vec<_>>();
    let [local] = local_declarations.as_slice() else {
        return None;
    };
    let local_access = structural_access_for_type_reference(program, local.type_reference)?;
    if !matches!(
        local_access,
        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
    ) || (local_access == CheckedStructuralAccess::MutableBorrow
        && root_access != CheckedStructuralAccess::MutableBorrow)
    {
        return None;
    }

    let occurrence_facts = facts
        .dynamic_conformances
        .selections
        .iter()
        .filter(|candidate| {
            candidate.machine == selection.machine
                && candidate.state == selection.state
                && candidate.binding == selection.binding
                && candidate.statement_index == selection.statement_index
                && candidate.source_symbol == selection.source_symbol
                && candidate.source_data == selection.source_data
                && candidate.target_trait == selection.target_trait
                && candidate.conformance == selection.conformance
                && candidate.rows == selection.rows
        })
        .collect::<Vec<_>>();
    let [occurrence_fact] = occurrence_facts.as_slice() else {
        return None;
    };
    let ExpressionNode::Cast(cast) = program
        .expression_table
        .expression(occurrence_fact.occurrence)
    else {
        return None;
    };
    let TypeReferenceNode::DynamicTrait {
        symbol,
        conformance,
        ..
    } = program
        .type_reference_table
        .type_reference(cast.target_type)
    else {
        return None;
    };
    if *symbol != selection.target_trait || *conformance != selection.conformance {
        return None;
    }
    let selection_value = match statements.get(selection.statement_index)? {
        StatementNode::LocalData(local) if local.symbol == selection.binding => local.initial_value,
        StatementNode::Assignment(assignment) => assignment.value,
        _ => return None,
    };
    let ExpressionNode::Borrow(selection_borrow) =
        program.expression_table.expression(selection_value)
    else {
        return None;
    };
    let cast_access = match selection_borrow.access {
        language_semantics::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        language_semantics::ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
    };
    if selection_borrow.target != occurrence_fact.occurrence || cast_access != local_access {
        return None;
    }
    let source_place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        selection.statement_index,
        cast.value,
    )?;
    if source_place.root != facts::PlaceRoot::Symbol(self_parameter.symbol)
        || source_place.segments
            != [facts::PlaceSegment::Field {
                symbol: selection.source_symbol,
            }]
    {
        return None;
    }

    Some((source_parameter_position, root_access, cast_access))
}

fn checked_self_attachment_source(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    selection: &checked_trees::DynamicConformanceBindingFact,
) -> Option<(SymbolHandle, Vec<CheckedUnitStructuralPathSegment>, String)> {
    let [self_name, field_name] = selection.source_path.as_slice() else {
        return None;
    };
    if !self_name.is_self_receiver()
        || field_name != &selection.source_name
        || !machine.attached_data_symbol.is_valid()
        || !selection.source_symbol.is_valid()
        || !selection.source_data.is_valid()
    {
        return None;
    }
    let attachments = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == machine.attached_data_symbol)
        .collect::<Vec<_>>();
    let [attachment] = attachments.as_slice() else {
        return None;
    };
    let fields = program
        .data_members(attachment)
        .iter()
        .filter_map(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == selection.source_symbol).then_some(field)
        })
        .collect::<Vec<_>>();
    let [field] = fields.as_slice() else {
        return None;
    };
    if field.name != *field_name || field.relevance.is_erased() {
        return None;
    }
    let TypeReferenceNode::Named { symbol, .. } = program
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        return None;
    };
    if *symbol != selection.source_data {
        return None;
    }
    let field_identity = terminal_field_identity(program, field.symbol)?;
    let source_type_identity = program
        .normalized_type_identity(field.type_reference)
        .into_string();
    (!source_type_identity.is_empty()).then_some((
        field.symbol,
        vec![CheckedUnitStructuralPathSegment::Field(field_identity)],
        source_type_identity,
    ))
}

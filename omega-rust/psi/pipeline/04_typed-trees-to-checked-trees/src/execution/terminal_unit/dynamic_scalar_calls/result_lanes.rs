//! The scalar and Unit result lanes of one checked dynamic call.
//!
//! [`build_checked_dynamic_call`](super::call_plans::build_checked_dynamic_call)
//! checks the custody both results share; a [`DynamicResultLane`] decides
//! only what its result changes. A scalar call is the initializer of an
//! immutable primitive local, and its plan carries that binding, the selected
//! realization's scalar body, an optional caller field store and an optional
//! Unit continuation over the result. A Unit call is a statement that ends
//! its caller's state, binds nothing and selects an operation-free
//! realization. Neither lane fabricates the other's result.

use super::call_plans::{DynamicCallCustody, DynamicCaller, ForwardedDispatch};
use super::forwarded_calls::{scalar_helper_body, unit_helper_body};
use super::realization_bodies::{CheckedRealizationScalarBody, checked_realization_scalar_body};
use crate::execution::terminal_unit::types::{ShapeCollector, is_unit, terminal_field_identity};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedUnitScalarResultBindingPlan, StatementNode, SymbolHandle, TypeReferenceNode, TypedTrees,
};
use crate::semantic::calls::CallSite;
use checked_trees::{CheckedDynamicBinding, CheckedDynamicDispatchPlan};
use language_semantics::{ServiceReachRowTable, ServiceReachSummary};
use typed_trees::types::TypeReferenceHandle;

/// What one result lane decides beyond the custody every dynamic call shares.
/// A lane value is the caller's result custody: the scalar binding, or
/// nothing for Unit.
pub(super) trait DynamicResultLane: Sized {
    /// The lane's published call plan.
    type Plan;
    /// One forwarded helper's retained body.
    type HelperBody;
    /// The selected realization's body, as the plan retains it.
    type SelectedBody;

    /// Whether a descriptor stored through an aggregate field can reach this
    /// lane's call. Unit lowering has no stored-descriptor lane.
    const STORES_DESCRIPTORS: bool;

    /// Whether `site` has this lane's call form: a statement for Unit, an
    /// expression for a scalar result.
    fn authors(site: &CallSite<'_>) -> bool;

    /// The caller statement that performs the call and consumes its result.
    fn caller_result(
        program: &TypedTrees,
        statements: &[StatementNode],
        flow_call: &checked_trees::FlowCallFact,
        site: &CallSite<'_>,
    ) -> Option<Self>;

    /// Whether a requirement or realization declaring `return_type` returns
    /// this call's result.
    fn returns(&self, program: &TypedTrees, return_type: TypeReferenceHandle) -> bool;

    /// One forwarded helper's body around its one call at `inner_call`.
    fn helper_body(
        program: &TypedTrees,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        inner_call: &checked_trees::FlowCallFact,
        inner_site: &CallSite<'_>,
    ) -> Option<Self::HelperBody>;

    /// Whether every forwarded helper body returns this call's result.
    fn admits_helper_bodies(&self, helpers: &[Self::HelperBody]) -> bool;

    /// The selected realization's checked body.
    fn selected_body(
        &self,
        program: &TypedTrees,
        facts: &CheckFacts,
        realization_machine: &typed_trees::machine::Machine,
        realization_state: &typed_trees::state::State,
    ) -> Option<Self::SelectedBody>;

    /// Whether the call and its caller may reach the services they do.
    fn admits_service_reach(
        rows: &ServiceReachRowTable,
        call: ServiceReachSummary,
        caller: ServiceReachSummary,
    ) -> bool;

    /// Publish the plan: the shared custody plus the lane's own parts.
    fn publish(
        self,
        caller: &DynamicCaller<'_, '_>,
        shapes: &mut ShapeCollector<'_>,
        custody: DynamicCallCustody<Self::HelperBody>,
        body: Self::SelectedBody,
    ) -> Option<Self::Plan>;

    /// Wrap one binding of this lane's plans as a dispatch.
    fn dispatch_plan(binding: CheckedDynamicBinding<Self::Plan>) -> CheckedDynamicDispatchPlan;
}

/// Both plans name their shared custody identically; only the origin enum
/// differs. A lane names its plan and origin and adds its own fields.
macro_rules! checked_dynamic_call_plan {
    ($plan:ident, $origin:ident, $custody:expr, { $($field:ident: $value:expr),* $(,)? }) => {{
        let custody = $custody;
        checked_trees::$plan {
            origin: match custody.forwarded {
                None => checked_trees::$origin::Local,
                Some(ForwardedDispatch {
                    machine,
                    state,
                    coordinate,
                    parameter,
                }) => checked_trees::$origin::Forwarded {
                    machine,
                    state,
                    coordinate,
                    parameter,
                },
            },
            forwarding_transfers: custody.forwarding_transfers,
            forwarding_helpers: custody.forwarding_helpers,
            caller_machine: custody.caller_machine,
            caller_state: custody.caller_state,
            caller_attachment_type_identity: custody.caller_attachment_type_identity,
            caller_multiplicity: custody.caller_multiplicity,
            caller_parameter_access: custody.caller_parameter_access,
            caller_contract_report_fingerprint: custody.caller_contract_report_fingerprint,
            caller_contract_commitment: custody.caller_contract_commitment,
            caller_service_reach: custody.caller_service_reach,
            coordinate: custody.coordinate,
            receiver_binding: custody.receiver_binding,
            selection: custody.selection,
            source_parameter_position: custody.source_parameter_position,
            source_access: custody.source_access,
            source_field: custody.source_field,
            source_path: custody.source_path,
            source_type_identity: custody.source_type_identity,
            source_multiplicity: custody.source_multiplicity,
            target_trait: custody.target_trait,
            selected_conformance: custody.selected_conformance,
            declaring_trait: custody.declaring_trait,
            requirement: custody.requirement,
            requirement_identity: custody.requirement_identity,
            realization_machine: custody.realization_machine,
            realization_state: custody.realization_state,
            realization_identity: custody.realization_identity,
            family_tuple: custody.family_tuple,
            realization_callables: custody.realization_callables,
            realization_contract_report_fingerprint: custody
                .realization_contract_report_fingerprint,
            realization_contract_commitment: custody.realization_contract_commitment,
            checked_call_service_reach: custody.checked_call_service_reach,
            $($field: $value,)*
        }
    }};
}

/// A scalar call's result: the immutable primitive local its call
/// initializes, at its position among the caller's scalar bindings.
pub(super) struct ScalarResultLane {
    result_binding: SymbolHandle,
    result: CheckedUnitScalarResultBindingPlan,
}

impl DynamicResultLane for ScalarResultLane {
    type Plan = checked_trees::CheckedDynamicScalarCallPlan;
    type HelperBody = checked_trees::CheckedDynamicScalarHelperPlan;
    type SelectedBody = CheckedRealizationScalarBody;

    const STORES_DESCRIPTORS: bool = true;

    fn authors(site: &CallSite<'_>) -> bool {
        matches!(site, CallSite::Expression { .. })
    }

    fn caller_result(
        program: &TypedTrees,
        statements: &[StatementNode],
        flow_call: &checked_trees::FlowCallFact,
        site: &CallSite<'_>,
    ) -> Option<Self> {
        let CallSite::Expression { expression, .. } = *site else {
            return None;
        };
        let StatementNode::LocalData(result_local) = statements.get(flow_call.statement_index)?
        else {
            return None;
        };
        if result_local.is_mutable
            || !result_local.symbol.is_valid()
            || result_local.initial_value != expression
        {
            return None;
        }
        let primitive_type = program.primitive_type_reference(result_local.type_reference)?;
        let binding_ordinal = statements[..flow_call.statement_index]
            .iter()
            .filter(|statement| {
                matches!(
                    statement,
                    StatementNode::LocalData(local)
                        if !local.is_mutable
                            && local.initial_value.is_valid()
                            && program.primitive_type_reference(local.type_reference).is_some()
                )
            })
            .count();
        Some(Self {
            result_binding: result_local.symbol,
            result: CheckedUnitScalarResultBindingPlan {
                statement_index: u32::try_from(flow_call.statement_index).ok()?,
                binding_ordinal: u32::try_from(binding_ordinal).ok()?,
                primitive_type,
            },
        })
    }

    fn returns(&self, program: &TypedTrees, return_type: TypeReferenceHandle) -> bool {
        program.primitive_type_reference(return_type) == Some(self.result.primitive_type)
    }

    fn helper_body(
        program: &TypedTrees,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        inner_call: &checked_trees::FlowCallFact,
        inner_site: &CallSite<'_>,
    ) -> Option<Self::HelperBody> {
        scalar_helper_body(program, facts, machine, state, inner_call, inner_site)
    }

    /// Each helper binds and returns this call's result type.
    fn admits_helper_bodies(&self, helpers: &[Self::HelperBody]) -> bool {
        helpers.iter().all(|helper| {
            helper.call_result.primitive_type == self.result.primitive_type
                && helper.scalar_control.primitive_type == self.result.primitive_type
        })
    }

    fn selected_body(
        &self,
        program: &TypedTrees,
        facts: &CheckFacts,
        realization_machine: &typed_trees::machine::Machine,
        realization_state: &typed_trees::state::State,
    ) -> Option<Self::SelectedBody> {
        checked_realization_scalar_body(
            program,
            facts,
            realization_machine,
            realization_state,
            self.result.primitive_type,
        )
    }

    /// A scalar result may select Unit control whose leaves reach services;
    /// lowering checks the reach it retains.
    fn admits_service_reach(
        _rows: &ServiceReachRowTable,
        _call: ServiceReachSummary,
        _caller: ServiceReachSummary,
    ) -> bool {
        true
    }

    /// The plan retains the caller's field store when the bounded
    /// three-statement shape admits one, and the checked Unit control its
    /// result immediately selects. Without that control, the call ends the
    /// caller's state.
    fn publish(
        self,
        caller: &DynamicCaller<'_, '_>,
        shapes: &mut ShapeCollector<'_>,
        custody: DynamicCallCustody<Self::HelperBody>,
        body: Self::SelectedBody,
    ) -> Option<Self::Plan> {
        let caller_store =
            checked_caller_structural_scalar_field_store_plan(caller, &custody, &self);
        let mut plan = checked_dynamic_call_plan!(
            CheckedDynamicScalarCallPlan,
            CheckedDynamicScalarCallOrigin,
            custody,
            {
                result_binding: self.result_binding,
                result: self.result,
                realization_return_expression: body.return_expression,
                realization_structural_scalar_field_stores: body.structural_scalar_field_stores,
                caller_structural_scalar_field_store: caller_store,
                unit_continuation: None,
            }
        );
        plan.unit_continuation =
            crate::execution::terminal_unit::composed_control::build_direct_dynamic_unit_continuation(
                caller.program,
                caller.facts,
                shapes,
                caller.boundaries,
                caller.machine,
                caller.state,
                &plan,
                caller.stored,
            );
        let retained_statement_count = usize::try_from(plan.coordinate.statement_index)
            .ok()?
            .checked_add(1)?;
        if plan.unit_continuation.is_none() && caller.statements.len() != retained_statement_count {
            return None;
        }
        Some(plan)
    }

    fn dispatch_plan(binding: CheckedDynamicBinding<Self::Plan>) -> CheckedDynamicDispatchPlan {
        CheckedDynamicDispatchPlan::Scalar(binding)
    }
}

/// A Unit call binds no result.
pub(super) struct UnitResultLane;

impl DynamicResultLane for UnitResultLane {
    type Plan = checked_trees::CheckedDynamicUnitCallPlan;
    type HelperBody = checked_trees::CheckedDynamicUnitHelperPlan;
    type SelectedBody = ();

    const STORES_DESCRIPTORS: bool = false;

    fn authors(site: &CallSite<'_>) -> bool {
        matches!(site, CallSite::Statement(_))
    }

    /// The call statement itself, as the last statement of its state.
    fn caller_result(
        _program: &TypedTrees,
        statements: &[StatementNode],
        flow_call: &checked_trees::FlowCallFact,
        site: &CallSite<'_>,
    ) -> Option<Self> {
        let CallSite::Statement(call) = *site else {
            return None;
        };
        (matches!(
            statements.get(flow_call.statement_index),
            Some(StatementNode::Call(candidate)) if std::ptr::eq(candidate, call)
        ) && statements.len() == flow_call.statement_index.checked_add(1)?)
        .then_some(Self)
    }

    fn returns(&self, program: &TypedTrees, return_type: TypeReferenceHandle) -> bool {
        is_unit(program, return_type)
    }

    fn helper_body(
        program: &TypedTrees,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        inner_call: &checked_trees::FlowCallFact,
        inner_site: &CallSite<'_>,
    ) -> Option<Self::HelperBody> {
        unit_helper_body(program, facts, machine, state, inner_call, inner_site)
    }

    /// A Unit helper returns Unit by construction.
    fn admits_helper_bodies(&self, _helpers: &[Self::HelperBody]) -> bool {
        true
    }

    /// The first Unit rung selects only an operation-free, contract-free body.
    fn selected_body(
        &self,
        program: &TypedTrees,
        _facts: &CheckFacts,
        _realization_machine: &typed_trees::machine::Machine,
        realization_state: &typed_trees::state::State,
    ) -> Option<Self::SelectedBody> {
        (program
            .statement_table
            .statements(realization_state.statement_nodes)
            .is_empty()
            && program.state_contracts(realization_state).is_empty())
        .then_some(())
    }

    /// Neither the Unit call nor its caller reaches a service.
    fn admits_service_reach(
        rows: &ServiceReachRowTable,
        call: ServiceReachSummary,
        caller: ServiceReachSummary,
    ) -> bool {
        [
            call.direct,
            call.transitive,
            caller.direct,
            caller.transitive,
        ]
        .into_iter()
        .all(|row| rows.services(row).is_empty())
    }

    fn publish(
        self,
        _caller: &DynamicCaller<'_, '_>,
        _shapes: &mut ShapeCollector<'_>,
        custody: DynamicCallCustody<Self::HelperBody>,
        (): Self::SelectedBody,
    ) -> Option<Self::Plan> {
        Some(checked_dynamic_call_plan!(
            CheckedDynamicUnitCallPlan,
            CheckedDynamicUnitCallOrigin,
            custody,
            {}
        ))
    }

    fn dispatch_plan(binding: CheckedDynamicBinding<Self::Plan>) -> CheckedDynamicDispatchPlan {
        CheckedDynamicDispatchPlan::Unit(binding)
    }
}

/// The scalar caller's one field store before its selection, when the
/// caller is exactly `self.carrier.field = literal; let erased = ...; let
/// result = erased.call();` over a mutable borrowed `self`.
fn checked_caller_structural_scalar_field_store_plan(
    caller: &DynamicCaller<'_, '_>,
    custody: &DynamicCallCustody<checked_trees::CheckedDynamicScalarHelperPlan>,
    lane: &ScalarResultLane,
) -> Option<checked_trees::CheckedStructuralScalarFieldStorePlan> {
    let program = caller.program;
    let facts = caller.facts;
    let state = caller.state;
    let selection = &custody.selection;
    let [
        StatementNode::Assignment(assignment),
        StatementNode::LocalData(selection_local),
        StatementNode::LocalData(result_local),
    ] = caller.statements.get(..3)?
    else {
        return None;
    };
    if selection.statement_index != 1
        || custody.coordinate.statement_index != 2
        || custody.coordinate.call_ordinal != 0
        || selection_local.symbol != selection.binding
        || result_local.symbol != lane.result_binding
        || custody.caller_parameter_access != CheckedStructuralAccess::MutableBorrow
    {
        return None;
    }

    let destination_parameter = program
        .state_parameters(state)
        .get(usize::try_from(custody.source_parameter_position).ok()?)?;
    let TypeReferenceNode::Reference { access, .. } = program
        .type_reference_table
        .type_reference(destination_parameter.type_reference)
    else {
        return None;
    };
    if !destination_parameter.is_self
        || destination_parameter.is_const
        || !destination_parameter.is_mutable
        || *access != language_semantics::ReferenceAccess::Mutable
    {
        return None;
    }

    let destination = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        0,
        assignment.target,
    )?;
    let [
        facts::PlaceSegment::Field {
            symbol: carrier_field,
        },
        facts::PlaceSegment::Field {
            symbol: primitive_field,
        },
    ] = destination.segments.as_slice()
    else {
        return None;
    };
    if destination.root != facts::PlaceRoot::Symbol(destination_parameter.symbol)
        || *carrier_field != custody.source_field
        || *carrier_field != selection.source_symbol
        || !primitive_field.is_valid()
    {
        return None;
    }

    let direct_fields = program
        .data_members(caller.source_definition)
        .iter()
        .filter_map(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == *primitive_field).then_some(field)
        })
        .collect::<Vec<_>>();
    let [direct_field] = direct_fields.as_slice() else {
        return None;
    };
    let primitive_type = program.primitive_type_reference(direct_field.type_reference)?;
    if direct_field.relevance.is_erased() {
        return None;
    }

    let expected_mutation_path =
        facts::canonical_place_label_from_parts(program, destination.root, &destination.segments);
    let mutation_paths = facts
        .mutation
        .for_machine(caller.machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)?
        .frame
        .complete_paths()?;
    if !matches!(mutation_paths, [path] if path == &expected_mutation_path) {
        return None;
    }

    let value = facts.values.scalar_expressions.expression_at(
        state.symbol,
        0,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    let direct_literal = matches!(value, CheckedScalarExpression::IntegerLiteral { .. })
        || matches!(
            value,
            CheckedScalarExpression::Boolean(expression)
                if matches!(
                    expression.as_ref(),
                    checked_trees::CheckedBooleanExpression::Constant(_)
                )
        );
    if !direct_literal || crate::values::scalar_expression_type(value) != Some(primitive_type) {
        return None;
    }

    Some(checked_trees::CheckedStructuralScalarFieldStorePlan {
        statement_index: 0,
        destination: checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter {
            position: custody.source_parameter_position,
        },
        carrier_path: custody.source_path.clone(),
        field_identity: terminal_field_identity(program, direct_field.symbol)?,
        primitive_type,
        value: checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(value.clone()),
    })
}

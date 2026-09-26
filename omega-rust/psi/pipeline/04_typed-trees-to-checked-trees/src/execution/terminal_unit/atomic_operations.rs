//! Atomic events: one `CheckedAtomicAccessPlan` per source atomic operation.
//!
//! The parser's carrier (`crate::validation::atomic_assignment_carrier`) spells a
//! fetch, swap, or compare-exchange as two statements — a result placeholder
//! `let prior: T = 0;` and the carrier assignment `place = Atomic { model }` —
//! and a store as the carrier assignment alone. A load is the value
//! `place.load(o)` of an immutable local. The sequence plans them as follows:
//!
//! - the placeholder local reserves the next dense scalar binding and plans
//!   nothing ([`result_placeholder`]); its literal initializer is not a value
//!   the program observes;
//! - the carrier assignment plans the event ([`writing_event`]), binding the
//!   reserved ordinal to the instruction-observed prior;
//! - a load local plans the event with its own binding ([`load_event`]).
//!
//! The place is an atomic field reached through static fields and at most one
//! literal element of a structural parameter — the (root, carrier path,
//! field) location a scalar field store names. Planning requires the field
//! to be declared atomic storage; a
//! modifying event needs exclusive (`&mut`) authority over its root, because
//! Terminal types do not yet mark atomic cells and a shared-borrow write is
//! only sound on one. That shared-receiver form (all atomic receivers are
//! shared in the source contract) therefore still declines here, with its
//! own trace phase, rather than producing a module verification refuses.
//! Operands are the carrier's own `AtomicOperand` scalar rows, rechecked
//! against the decoder's operand expressions.

use super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, ExpressionNode, Multiplicity, PrimitiveType, StatementNode,
    TypedTrees,
};
use crate::checked_trees::{
    CheckedAtomicAccessPlan, CheckedAtomicEvent, CheckedAtomicReadModifyWrite,
    CheckedCallScalarArgument,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::validation::AtomicAccessOperation;

/// The reserved result of a writing event whose carrier is the next
/// statement: `local` is the placeholder when that carrier's result names it.
pub(super) fn result_placeholder(
    program: &TypedTrees,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    statement_index: usize,
    local: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::TableLocalData,
) -> bool {
    if local.is_mutable {
        return false;
    }
    let Some(StatementNode::Assignment(assignment)) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index + 1)
    else {
        return false;
    };
    crate::validation::atomic_assignment_carrier(program, assignment)
        .is_some_and(|carrier| names_local(program, carrier.result, local))
}

/// Whether the local at `statement_index` belongs to an atomic event — a
/// load or a writing event's result placeholder — and so must be planned by
/// the sequence rather than evaluated as a pure prefix initializer.
pub(super) fn owns_local(
    program: &TypedTrees,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    statement_index: usize,
) -> bool {
    let Some(StatementNode::LocalData(local)) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return false;
    };
    crate::validation::atomic_load_carrier(program, local.initial_value).is_some()
        || result_placeholder(program, state, statement_index, local)
}

/// Whether the carrier's generated result name binds `local`: its resolved
/// symbol when the desugar left one, otherwise its spelling.
fn names_local(
    program: &TypedTrees,
    result: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    local: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::TableLocalData,
) -> bool {
    if !program.expression_table.expression_is_valid(result) {
        return false;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(result) else {
        return false;
    };
    if path.symbol.is_valid() {
        return path.symbol == local.symbol;
    }
    matches!(
        program.expression_table.name_path_members(path.members),
        [member] if *member == local.name
    )
}

/// The load `let v: T = place.load(o);` at `statement_index`, binding `v`.
#[allow(clippy::too_many_arguments)]
pub(super) fn load_event(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    local: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::TableLocalData,
    binding_ordinal: u32,
    trace: &LocalConstructionTrace,
) -> Option<CheckedAtomicAccessPlan> {
    let carrier = crate::validation::atomic_load_carrier(program, local.initial_value)?;
    let AtomicAccessOperation::Load(ordering) = carrier.operation else {
        return None;
    };
    trace.phase("atomic access: load place");
    let place = place(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        statement_index,
        carrier.place,
        false,
        trace,
    )?;
    trace.phase("atomic access: load result binding");
    if local.is_mutable
        || program.primitive_type_reference(local.type_reference)? != place.primitive_type
    {
        return None;
    }
    Some(CheckedAtomicAccessPlan {
        statement_index,
        parameter_index: place.parameter_index,
        carrier_path: place.carrier_path,
        field_identity: place.field_identity,
        primitive_type: place.primitive_type,
        event: CheckedAtomicEvent::Load { ordering },
        result: Some(CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal,
            primitive_type: place.primitive_type,
        }),
    })
}

/// The writing event the carrier assignment at `statement_index` denotes.
/// `reserved` is the placeholder binding an observing event binds; a store
/// takes none.
#[allow(clippy::too_many_arguments)]
pub(super) fn writing_event(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    assignment: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::TableAssignment,
    reserved: Option<CheckedUnitScalarResultBindingPlan>,
    trace: &LocalConstructionTrace,
) -> Option<CheckedAtomicAccessPlan> {
    let carrier = crate::validation::atomic_assignment_carrier(program, assignment)?;
    trace.phase("atomic access: writing place");
    let place = place(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        statement_index,
        carrier.place,
        true,
        trace,
    )?;
    let primitive_type = place.primitive_type;
    trace.phase("atomic access: operands");
    let operands = carrier
        .operands
        .iter()
        .enumerate()
        .map(|(ordinal, operand)| {
            operand_row(
                program,
                facts,
                state,
                statement_index,
                u32::try_from(ordinal).ok()?,
                *operand,
                primitive_type,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let fetch = |operation, ordering| {
        let [operand] = <[CheckedCallScalarArgument; 1]>::try_from(operands.clone()).ok()?;
        Some(CheckedAtomicEvent::ReadModifyWrite {
            operation,
            ordering,
            operand,
        })
    };
    let event = match carrier.operation {
        AtomicAccessOperation::Store(ordering) => {
            let [value] = <[CheckedCallScalarArgument; 1]>::try_from(operands.clone()).ok()?;
            CheckedAtomicEvent::Store { ordering, value }
        }
        AtomicAccessOperation::Swap(ordering) => {
            let [value] = <[CheckedCallScalarArgument; 1]>::try_from(operands.clone()).ok()?;
            CheckedAtomicEvent::Swap { ordering, value }
        }
        AtomicAccessOperation::FetchAdd(ordering) => {
            fetch(CheckedAtomicReadModifyWrite::FetchAdd, ordering)?
        }
        AtomicAccessOperation::FetchSub(ordering) => {
            fetch(CheckedAtomicReadModifyWrite::FetchSub, ordering)?
        }
        AtomicAccessOperation::FetchAnd(ordering) => {
            fetch(CheckedAtomicReadModifyWrite::FetchAnd, ordering)?
        }
        AtomicAccessOperation::FetchOr(ordering) => {
            fetch(CheckedAtomicReadModifyWrite::FetchOr, ordering)?
        }
        AtomicAccessOperation::FetchXor(ordering) => {
            fetch(CheckedAtomicReadModifyWrite::FetchXor, ordering)?
        }
        AtomicAccessOperation::CompareExchange { success, failure } => {
            let [expected, replacement] =
                <[CheckedCallScalarArgument; 2]>::try_from(operands.clone()).ok()?;
            CheckedAtomicEvent::CompareExchange {
                success,
                failure,
                expected,
                replacement,
            }
        }
        AtomicAccessOperation::Load(_) | AtomicAccessOperation::CompareExchangeOnce { .. } => {
            return None;
        }
    };
    trace.phase("atomic access: result binding");
    let result = match (event.observes_resident(), reserved) {
        (true, Some(result))
            if result.primitive_type == primitive_type
                && result.statement_index.checked_add(1) == Some(statement_index) =>
        {
            Some(result)
        }
        (false, None) => None,
        _ => return None,
    };
    Some(CheckedAtomicAccessPlan {
        statement_index,
        parameter_index: place.parameter_index,
        carrier_path: place.carrier_path,
        field_identity: place.field_identity,
        primitive_type,
        event,
        result,
    })
}

/// The retained `AtomicOperand` row for one authored operand, exact for the
/// leaf carrier.
fn operand_row(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    statement_index: u32,
    operand_ordinal: u32,
    operand: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    primitive_type: PrimitiveType,
) -> Option<CheckedCallScalarArgument> {
    let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AtomicOperand { operand_ordinal },
    )?;
    (binding.expression == operand
        && crate::values::scalar_expression_type(value) == Some(primitive_type)
        && super::structural_scalar_store::scalar_custody_is_exact(
            program,
            facts,
            state,
            binding,
            value,
            primitive_type,
        ))
    .then(|| CheckedCallScalarArgument::Pure(value.clone()))
}

/// The atomic field `expression` names at `statement_index`.
struct AtomicPlace {
    parameter_index: u32,
    carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    field_identity: String,
    primitive_type: PrimitiveType,
}

/// The atomic field `expression` names at `statement_index`: a structural
/// parameter root, the static carrier path to the record holding the field,
/// the field, and its carrier.
#[allow(clippy::too_many_arguments)]
fn place(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    modifies: bool,
    trace: &LocalConstructionTrace,
) -> Option<AtomicPlace> {
    trace.phase("atomic access: place: atomic storage");
    if !crate::validation::place_is_atomic_storage(program, machine, Some(state), expression) {
        return None;
    }
    let primitive_type =
        crate::validation::declared_place_type_raw(program, machine, Some(state), expression)
            .and_then(|declared| program.primitive_type_reference(declared))?;
    let ordinal = usize::try_from(statement_index).ok()?;
    let canonical = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        ordinal,
        expression,
    )?;
    let crate::fact_plan::PlaceRoot::Symbol(symbol) = canonical.root else {
        return None;
    };
    // An erased borrow carrier names its captured referent's storage.
    let (symbol, segments) = match super::receiver_aliases::aliases(program, facts, machine, state)
        .unwrap_or_default()
        .iter()
        .find(|alias| alias.owner == symbol)
    {
        Some(alias) => {
            let mut segments = alias.segments.clone();
            segments.extend_from_slice(&canonical.segments);
            (alias.root, segments)
        }
        None => (symbol, canonical.segments.clone()),
    };
    // One event names one location: fields and literal elements only, each
    // literal element within its declared extent, ending at the atomic field.
    // A runtime element names no single location.
    trace.phase("atomic access: place: static field path");
    if !crate::validation::place_has_builtin_coordinates(program, machine, Some(state), expression)
        || !segments.iter().all(|segment| {
            matches!(
                segment,
                crate::fact_plan::PlaceSegment::Field { .. }
                    | crate::fact_plan::PlaceSegment::FixedIndex { .. }
            )
        })
    {
        return None;
    }
    // Every literal element lies within its declared extent; a runtime
    // selector has no retained coordinate here, so it refuses.
    let selectors = super::structural_scalar_store::TargetSelectors::resolve(
        program,
        facts,
        machine,
        state,
        statement_index,
        expression,
    )?;
    let mut carrier_path =
        super::structural_scalar_store::checked_unit_path(program, &selectors, &segments)?;
    let Some(CheckedUnitStructuralPathSegment::Field(field_identity)) = carrier_path.pop() else {
        return None;
    };
    trace.phase("atomic access: place: parameter authority");
    let (parameter_index, parameter) =
        structural_parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| {
                program
                    .state_parameters(state)
                    .get(parameter.position as usize)
                    .is_some_and(|source| source.symbol == symbol)
            })?;
    let authorized = if modifies {
        parameter.access == CheckedStructuralAccess::MutableBorrow
    } else {
        matches!(
            parameter.access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::SharedBorrow
        )
    };
    if modifies && parameter.access == CheckedStructuralAccess::SharedBorrow {
        trace.phase("atomic access: place: shared receiver without atomic cell identity");
        return None;
    }
    if !authorized
        || parameter.multiplicity == Multiplicity::Linear
        || !parameter.qualifications.is_empty()
    {
        return None;
    }
    Some(AtomicPlace {
        parameter_index: u32::try_from(parameter_index).ok()?,
        carrier_path,
        field_identity,
        primitive_type,
    })
}

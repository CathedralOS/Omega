//! Atomic events: one `CheckedAtomicAccessPlan` per source atomic operation.
//!
//! The parser's carrier (`validation::atomic_assignment_carrier`) spells a
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
//! The place is an atomic field reached through an ordinary shared structural
//! receiver, retaining its actual home and stored-reference crossings. Static
//! coordinates select one exact cell. Planning requires the field
//! to be declared atomic storage. Its retained field type supplies atomic-cell
//! identity, so shared access authorizes the event without granting ordinary
//! shared mutation. Independent Terminal verification checks that identity.
//! Operands retain `AtomicOperand` roles, either as pure scalar rows or ordinary
//! computation roots. The latter own their nested calls and ordered effects;
//! neither the generated update model nor a separate statement call executes.
//! Both forms bind the decoder's exact authored expression and cell value type.

use super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralPathSegment, PrimitiveType, StatementNode,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use checked_trees::{
    CheckedAtomicAccessPlan, CheckedAtomicEvent, CheckedAtomicReadModifyWrite,
    CheckedCallScalarArgument, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use validation::AtomicAccessOperation;

/// The reserved result of a writing event whose carrier is the next
/// statement: `local` is the placeholder when that carrier's result names it.
pub(super) fn result_placeholder(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    local: &typed_trees::statement::TableLocalData,
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
    validation::atomic_assignment_carrier(program, assignment)
        .is_some_and(|carrier| names_local(program, carrier.result, local))
}

/// Whether the carrier's generated result name binds `local`: its resolved
/// symbol when the desugar left one, otherwise its spelling.
fn names_local(
    program: &dyn ::validation::CheckingInput,
    result: typed_trees::expression::ExpressionHandle,
    local: &typed_trees::statement::TableLocalData,
) -> bool {
    if !program.expression_table.expression_is_valid(result) {
        return false;
    }
    let Some(path) = program.expression_table.name_path_node(result) else {
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
    binding_ordinal: u32,
    trace: &LocalConstructionTrace,
    shapes: &mut super::types::ShapeCollector<'_>,
) -> Option<CheckedAtomicAccessPlan> {
    let carrier = validation::atomic_load_carrier(program, local.initial_value)?;
    let AtomicAccessOperation::Load(ordering) = carrier.operation else {
        return None;
    };
    trace.phase("atomic access: load place");
    let place = place(
        program,
        facts,
        machine,
        state,
        statement_index,
        carrier.place,
        trace,
        shapes,
    )?;
    trace.phase("atomic access: load result binding");
    if local.is_mutable
        || program.primitive_type_reference(local.type_reference)? != place.primitive_type
    {
        return None;
    }
    Some(CheckedAtomicAccessPlan {
        statement_index,
        receiver: place.receiver,
        receiver_root_type_identity: place.receiver_root_type_identity,
        field_case: place.field_case,
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    reserved: Option<CheckedUnitScalarResultBindingPlan>,
    trace: &LocalConstructionTrace,
    shapes: &mut super::types::ShapeCollector<'_>,
) -> Option<CheckedAtomicAccessPlan> {
    let carrier = validation::atomic_assignment_carrier(program, assignment)?;
    trace.phase("atomic access: writing place");
    let place = place(
        program,
        facts,
        machine,
        state,
        statement_index,
        carrier.place,
        trace,
        shapes,
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
                machine.symbol,
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
        receiver: place.receiver,
        receiver_root_type_identity: place.receiver_root_type_identity,
        field_case: place.field_case,
        field_identity: place.field_identity,
        primitive_type,
        event,
        result,
    })
}

/// The retained `AtomicOperand` source for one authored operand, exact for the
/// leaf carrier. The event consumes the ordinary computation when it owns
/// calls; otherwise the bound pure row supplies the same value coordinate.
fn operand_row(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    operand_ordinal: u32,
    operand: typed_trees::expression::ExpressionHandle,
    primitive_type: PrimitiveType,
) -> Option<CheckedCallScalarArgument> {
    let role = CheckedScalarExpressionRole::AtomicOperand { operand_ordinal };
    let computations = &facts.values.scalar_computations;
    let mut roots = computations.roots_for_statement_role(state.symbol, statement_index, role);
    if let Some(root) = roots.next() {
        if roots.next().is_some()
            || !computations.is_authored_root(root, machine, operand, primitive_type)
            || facts
                .values
                .scalar_expressions
                .expression_at(state.symbol, statement_index, role)
                .is_some()
        {
            return None;
        }
        return Some(CheckedCallScalarArgument::Computation(root.root));
    }
    let (binding, value) =
        facts
            .values
            .scalar_expressions
            .bound_expression_at(state.symbol, statement_index, role)?;
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
    receiver_root_type_identity: typed_trees::type_identity::TypeIdentityId,
    receiver: CheckedUnitStructuralArgumentPlan,
    field_case: Option<checked_trees::CheckedMemberIdentity>,
    field_identity: checked_trees::CheckedMemberIdentity,
    primitive_type: PrimitiveType,
}

/// The atomic field `expression` names at `statement_index`: the shared
/// receiver of its containing record, the field, and its scalar carrier.
#[allow(clippy::too_many_arguments)]
fn place(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    trace: &LocalConstructionTrace,
    shapes: &mut super::types::ShapeCollector<'_>,
) -> Option<AtomicPlace> {
    trace.phase("atomic access: place: atomic storage");
    if !validation::place_is_atomic_storage(program, machine, Some(state), expression) {
        return None;
    }
    let primitive_type =
        validation::declared_place_type_raw(program, machine, Some(state), expression)
            .and_then(|declared| program.primitive_type_reference(declared))?;
    let ordinal = usize::try_from(statement_index).ok()?;
    let canonical = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        ordinal,
        expression,
    )?;
    let symbol = canonical.root.as_symbol()?;
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
    if !validation::place_has_builtin_coordinates(program, machine, Some(state), expression)
        || !segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
                    | facts::PlaceSegment::Case { .. }
            )
        })
    {
        return None;
    }
    let mut carrier = validation::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(symbol),
        segments,
    };
    let Some(facts::PlaceSegment::Field { symbol: field }) = carrier.segments.pop() else {
        return None;
    };
    let field_identity =
        crate::execution::terminal_unit::types::terminal_field_identity(program, field)?;
    // The final case selects the field namespace of the sum itself. It is
    // not a structural child that can be passed as a standalone receiver.
    let field_case = if let Some(facts::PlaceSegment::Case { variant }) = carrier.segments.last() {
        let mut cases = program
            .data_definitions()
            .iter()
            .flat_map(|owner| program.data_members(owner))
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Variant(case) if case.symbol == *variant => {
                    Some(case)
                }
                _ => None,
            });
        let case = cases.next()?;
        if cases.next().is_some() {
            return None;
        }
        let identity =
            checked_trees::CheckedMemberIdentity::of_member(case.identity, case.name.clone());
        carrier.segments.pop();
        Some(identity)
    } else {
        None
    };

    // Only the concrete storage crossed by this performed operation is a
    // demand. Stored pointer declarations remain identity-only, including
    // recursive generic families that have no runtime dereference here.
    let binders = super::types::machine_binders(program, machine);
    let mut root_identity = None;
    for position in 0..=carrier.segments.len() {
        let reference = validation::canonical_place_type_reference(
            program,
            state.symbol,
            ordinal,
            &validation::CanonicalPlace {
                root: carrier.root,
                segments: carrier.segments[..position].to_vec(),
            },
        )?;
        let identity = if position == 0 && program.state_local_declaration(state, symbol).is_some()
        {
            shapes.add_stored_type(reference, &binders, &[])?
        } else {
            shapes.add_type(reference, &binders, &[])?
        };
        if position == 0 {
            root_identity = Some(identity);
        }
    }
    let receiver_root_type_identity = root_identity?;
    trace.phase("atomic access: place: shared receiver");
    // Atomic source admission already checks shared access to the dedicated
    // cell. Reuse the ordinary observation path to retain its containing
    // record, including stored descriptors, rather than substituting their
    // initializer's backing or requiring a parameter-shaped spelling.
    let (reference, path) = super::calls::projected_observed_path(
        program,
        machine.symbol,
        state.symbol,
        ordinal,
        &carrier,
        &|_| None,
    )?;
    if path.iter().any(|segment| {
        matches!(
            segment,
            CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                | CheckedUnitStructuralPathSegment::ComputedIndex(_)
                | CheckedUnitStructuralPathSegment::FixedByteRange { .. }
        )
    }) {
        return None;
    }
    let receiver = CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
        path,
        type_identity: program.normalized_type_identity(reference),
        access: CheckedStructuralAccess::SharedBorrow,
    };
    Some(AtomicPlace {
        receiver,
        receiver_root_type_identity,
        field_case,
        field_identity,
        primitive_type,
    })
}

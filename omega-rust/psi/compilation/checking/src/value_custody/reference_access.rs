//! Receiving a reference value cannot manufacture the source's access.
//!
//! Calls, local initialization and field construction share this judgment at the exact
//! receiving leaf. A mutable slot or byte-compatible shape is not authority
//! over its referent. Declared reference forwarding preserves every enclosing
//! access, while lifetime, exclusivity, element/domain correspondence and
//! storage admission remain independent checks at their existing owners.

use super::expression_types::declared_reference_access;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

pub(crate) enum ReferenceAccessRefusal {
    ExplicitWriteOnlyRequired,
    WriteOnlyMayNotRead,
    MutableSourceRequired,
    ExclusiveBorrowSourceRequired {
        root: symbols::SymbolHandle,
        access: language_semantics::ReferenceAccess,
    },
    /// An exclusive borrow of a value that names no place: a temporary has
    /// no storage to lend.
    NamedPlaceRequired {
        access: language_semantics::ReferenceAccess,
    },
}

/// The shared access rule for every receiving reference leaf;
/// it grants no loan and never derives permission from a destination type.
pub(crate) fn receiving_reference_access_refusal(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    value: ExpressionHandle,
    required: TypeReferenceHandle,
) -> Option<ReferenceAccessRefusal> {
    use language_semantics::ReferenceAccess;
    // Selection completes one of the retained arm values. Every possible
    // result must supply the same receiving permission; the destination's
    // access and the selected subject cannot supply it for an arm. Pattern
    // effects, coverage and each arm's loans keep their ordinary formation
    // judgments. Reuse those exact result handles rather than deriving one
    // access from a representative arm or flattening the source expression.
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(value) {
        return program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .find_map(|arm| {
                receiving_reference_access_refusal(
                    program,
                    machine,
                    state,
                    prior_statements,
                    arm.value,
                    required,
                )
            });
    }
    let expected_access = declared_reference_access(program, required);
    let completed_borrow = completed_reference_borrow_source(program, machine, state, value);
    let supplied_access = completed_borrow
        .map(|(_, access)| access)
        .or_else(|| declared_call_reference_access(program, value));
    if let Some((source, access)) = completed_borrow
        && access.is_exclusive()
        && let Some(refusal) =
            borrow_source_access_refusal(program, machine, state, prior_statements, source, access)
    {
        return Some(refusal);
    }
    if expected_access == Some(ReferenceAccess::WriteOnly)
        && !matches!(program.expression_table.expression(value),
            ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::WriteOnly)
    {
        return Some(ReferenceAccessRefusal::ExplicitWriteOnlyRequired);
    }
    if supplied_access == Some(ReferenceAccess::WriteOnly)
        && expected_access != Some(ReferenceAccess::WriteOnly)
    {
        return Some(ReferenceAccessRefusal::WriteOnlyMayNotRead);
    }
    if expected_access == Some(ReferenceAccess::Mutable)
        && !matches!(supplied_access, Some(ReferenceAccess::Mutable))
        && !value_forwards_mutable_reference(program, machine, state, prior_statements, value)
    {
        return Some(ReferenceAccessRefusal::MutableSourceRequired);
    }
    None
}

pub(crate) fn is_reference_destination(
    program: &dyn crate::CheckingInput,
    required: TypeReferenceHandle,
) -> bool {
    declared_reference_access(program, required).is_some()
}

/// A completed borrowing operation retains its exact storage source and
/// source-selected access. The call-shape owner excludes declared machines
/// with the same spelling; the receiver's actual carrier distinguishes the
/// compiler collection operation. Window geometry retains its selected builtin
/// meaning. Neither a receiving reference type nor a slot's mutability can
/// supply access, and this query establishes no extent or loan.
fn completed_reference_borrow_source(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
) -> Option<(ExpressionHandle, language_semantics::ReferenceAccess)> {
    use language_semantics::ReferenceAccess;
    use language_semantics::declaration_selection::CollectionViewOperation;

    match program.expression_table.expression(value) {
        ExpressionNode::Borrow(borrow) => Some((borrow.target, borrow.access)),
        ExpressionNode::Call(call) => {
            let operation = crate::collection_operations::collection_view_call(program, call)?;
            let reference = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                state,
                call.receiver,
            )?;
            if !crate::collection_operations::type_reference_is_collection(program, reference) {
                return None;
            }
            let access = match operation {
                CollectionViewOperation::SharedSlice => ReferenceAccess::Shared,
                CollectionViewOperation::MutableSlice => ReferenceAccess::Mutable,
                // Text and byte-content conversions have different carrier and
                // source laws; their declared result/formation owners remain.
                CollectionViewOperation::TextView | CollectionViewOperation::Bytes => return None,
            };
            Some((call.receiver, access))
        }
        ExpressionNode::Indexed(indexed)
            if program.expression_table.range_node(indexed.index).is_some()
                && crate::value_custody::places::place_has_builtin_coordinates(
                    program, machine, state, value,
                ) =>
        {
            let reference = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                state,
                indexed.collection,
            )?;
            if !crate::collection_operations::type_reference_is_collection(program, reference) {
                return None;
            }
            Some((
                indexed.collection,
                declared_reference_access(program, reference)?,
            ))
        }
        _ => None,
    }
}

/// A selected ordinary callable forwards its declared result access; it does
/// not form a new borrow of a binding slot. Compiler-owned borrowing operations
/// use their retained source instead, through the common judgment above.
fn declared_call_reference_access(
    program: &dyn crate::CheckingInput,
    argument: ExpressionHandle,
) -> Option<language_semantics::ReferenceAccess> {
    let ExpressionNode::Call(call) = program.expression_table.expression(argument) else {
        return None;
    };
    crate::machine_calls::calls::resolved_call_result_type(program, call)
        .and_then(|reference| declared_reference_access(program, reference))
}

/// Forward the selected reference permission, not the mutability of its slot.
/// An owned aggregate can carry an exclusive reference without a mutable local
/// binding. Every enclosing reference still limits access: reading that same
/// leaf through a shared aggregate cannot recover its exclusive permission.
fn value_forwards_mutable_reference(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    argument: ExpressionHandle,
) -> bool {
    let Some((root, binding_type, _)) = source_place_root(
        program,
        current_machine,
        current_state,
        prior_statements,
        argument,
    ) else {
        return false;
    };
    crate::value_custody::expression_types::place_forwards_mutable_reference(
        program,
        argument,
        root,
        binding_type,
    )
}

/// Every exclusive borrowing operation lends its source place's own access. A representation
/// recast (`&mut place as &mut [T]`) lends the place it views, exactly as
/// borrow admission resolves it; the viewed type grants no writability.
fn borrow_source_access_refusal(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    mut source: ExpressionHandle,
    access: language_semantics::ReferenceAccess,
) -> Option<ReferenceAccessRefusal> {
    if let Some(cast) = program.expression_table.cast_node(source)
        && cast.form.is_recast()
    {
        source = cast.value;
        source = program.expression_table.peeled_expression_handle(source);
    }
    let Some((root, binding_type, owned_root_is_writable)) =
        source_place_root(program, machine, state, prior_statements, source)
    else {
        return Some(ReferenceAccessRefusal::NamedPlaceRequired { access });
    };
    (!crate::value_custody::expression_types::place_permits_reference_borrow(
        program,
        source,
        root,
        binding_type,
        owned_root_is_writable,
        access,
    ))
    .then_some(ReferenceAccessRefusal::ExclusiveBorrowSourceRequired { root, access })
}

fn source_place_root(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    argument: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, TypeReferenceHandle, bool)> {
    if !crate::value_custody::places::place_has_builtin_coordinates(
        program,
        current_machine,
        current_state,
        argument,
    ) {
        return None;
    }
    let mut root = argument;
    loop {
        let receiver = match program.expression_table.expression(root) {
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            ExpressionNode::Name(_) => break,
            _ => return None,
        };
        root = receiver;
    }
    let path = program.expression_table.name_path_node(root)?;
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if path.head_symbol != path.symbol {
        return None;
    }
    if let Some(state) = current_state
        && let Some(binding_type) = crate::value_custody::locals::state_binding_type(
            program,
            state,
            prior_statements,
            path.symbol,
            name.as_str(),
        )
    {
        let owned_root_is_writable = match program.symbols.get(path.symbol).kind {
            // Borrow-root formation lends every owned local's contents;
            // `let mut` separately gates replacing the binding itself.
            // This matches stage04's retained BorrowWritableRootFact roster,
            // not WritableRoots::contains_for_write's bare reassignment test.
            symbols::SymbolKind::Local => true,
            symbols::SymbolKind::Parameter => program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.symbol == path.symbol && parameter.is_mutable),
            _ => false,
        };
        return Some((path.symbol, binding_type, owned_root_is_writable));
    }
    program
        .machine_owned_data_by_symbol(current_machine, path.symbol)
        .map(|owned| (path.symbol, owned.type_reference, true))
}

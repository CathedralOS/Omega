//! Receiving a reference value cannot manufacture the source's access.
//!
//! Calls and structural field construction share this judgment at the exact
//! receiving leaf. A mutable slot or byte-compatible shape is not authority
//! over its referent. Declared reference forwarding preserves every enclosing
//! access, while lifetime, exclusivity, element/domain correspondence and
//! storage admission remain independent checks at their existing owners.

use super::expression_types::declared_reference_access;
use typed_trees::TypedTrees;
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
}

/// This is the call's existing access rule generalized to receiving fields;
/// it grants no loan and never derives permission from a destination type.
pub(crate) fn receiving_reference_access_refusal(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    value: ExpressionHandle,
    required: TypeReferenceHandle,
) -> Option<ReferenceAccessRefusal> {
    use language_semantics::ReferenceAccess;
    let expected_access = declared_reference_access(program, required);
    let supplied_access = supplied_reference_access(program, value);
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(value)
        && borrow.access.is_exclusive()
        && let Err(root) = explicit_borrow_source_access(
            program,
            machine,
            state,
            prior_statements,
            borrow.target,
            borrow.access,
        )
    {
        return Some(ReferenceAccessRefusal::ExclusiveBorrowSourceRequired {
            root,
            access: borrow.access,
        });
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
    program: &TypedTrees,
    required: TypeReferenceHandle,
) -> bool {
    declared_reference_access(program, required).is_some()
}

/// The reference access one argument actually supplies: an explicit `&`,
/// `&mut`, or `&write` borrow expression, or a resolved call result carrying
/// its declared access. Shared by the statement/transition path
/// (`validate_call_arguments_handles`) and the value-position path
/// (`validate_value_call_argument_classes`).
fn supplied_reference_access(
    program: &TypedTrees,
    argument: ExpressionHandle,
) -> Option<language_semantics::ReferenceAccess> {
    match program.expression_table.expression(argument) {
        ExpressionNode::Borrow(borrow) => Some(borrow.access),
        ExpressionNode::Call(call) => {
            // A resolved result carries its declared reference access;
            // passing it onward is not a new borrow of a binding slot.
            crate::machine_calls::calls::resolved_call_result_type(program, call)
                .and_then(|reference| declared_reference_access(program, reference))
        }
        _ => None,
    }
}

/// Forward the selected reference permission, not the mutability of its slot.
/// An owned aggregate can carry an exclusive reference without a mutable local
/// binding. Every enclosing reference still limits access: reading that same
/// leaf through a shared aggregate cannot recover its exclusive permission.
fn value_forwards_mutable_reference(
    program: &TypedTrees,
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

fn explicit_borrow_source_access(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    source: ExpressionHandle,
    access: language_semantics::ReferenceAccess,
) -> Result<(), symbols::SymbolHandle> {
    let Some((root, binding_type, owned_root_is_writable)) =
        source_place_root(program, machine, state, prior_statements, source)
    else {
        return Err(symbols::SymbolHandle::invalid());
    };
    if crate::value_custody::expression_types::place_permits_reference_borrow(
        program,
        source,
        root,
        binding_type,
        owned_root_is_writable,
        access,
    ) {
        Ok(())
    } else {
        Err(root)
    }
}

fn source_place_root(
    program: &TypedTrees,
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
    let ExpressionNode::Name(path) = program.expression_table.expression(root) else {
        return None;
    };
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
        .machine_owned_data(current_machine)
        .iter()
        .find(|owned| owned.symbol == path.symbol)
        .map(|owned| (path.symbol, owned.type_reference, true))
}

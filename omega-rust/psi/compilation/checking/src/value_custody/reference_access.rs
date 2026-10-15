//! Receiving a reference value cannot manufacture the source's access.
//!
//! Calls, local initialization, returns and field construction share this judgment at the exact
//! receiving leaf. A mutable slot or byte-compatible shape is not authority
//! over its referent. Declared reference forwarding preserves every enclosing
//! access, while lifetime, exclusivity, element/domain correspondence and
//! storage admission remain independent checks at their existing owners.
//!
//! A loan's access is spelled where the loan is formed: `&mut place` or
//! `&write place`. A bare window `coll[a..b]` carries no spelling of its own,
//! so it is the shared loan the `[..]` adaptation forms (`Slice::range` takes
//! `&[T]`) at every receiving leaf -- a local initializer, a call or state
//! argument, a returned value or a constructed field -- and mutable lending is
//! spelled `&mut coll[a..b]`.
//! This is `bare-range-reference-access` in the expression specification:
//! an expected `&mut [T]` type never inserts the mutable borrow, including on
//! a local declaration, and the bare window at a mutable destination is
//! refused by name rather than as a generic access shortfall.

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
    /// A bare window `coll[a..b]` at a mutable destination: the window is
    /// the shared view `bare-range-reference-access` forms, and the mutable
    /// subrange must be spelled `&mut coll[a..b]`.
    BareRangeFormsSharedView,
}

/// The shared access rule for every receiving reference leaf; it grants no
/// loan and never derives permission from a destination type. A local
/// initializer, a call or state argument, return and constructed field all receive
/// under the same judgment: the declared `&mut` of the destination is never
/// the loan's spelling.
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
        .as_ref()
        .map(|borrow| borrow.access)
        .or_else(|| declared_call_reference_access(program, value))
        .or_else(|| {
            value_forwarded_exclusive_reference_access(
                program,
                machine,
                state,
                prior_statements,
                value,
            )
        });
    if let Some(borrow) = &completed_borrow
        && borrow.access.is_exclusive()
        && let Some(refusal) = borrow_source_access_refusal(
            program,
            machine,
            state,
            prior_statements,
            borrow.source,
            borrow.access,
        )
    {
        return Some(refusal);
    }
    // Attenuation must be explicit where a new write-only loan is formed.
    // An ordinary call has already produced its declared loan; receiving that
    // descriptor neither forms another borrow nor observes its referent.
    if expected_access == Some(ReferenceAccess::WriteOnly)
        && supplied_access != Some(ReferenceAccess::WriteOnly)
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
        && completed_borrow
            .as_ref()
            .is_some_and(|borrow| borrow.bare_window)
    {
        return Some(ReferenceAccessRefusal::BareRangeFormsSharedView);
    }
    if expected_access == Some(ReferenceAccess::Mutable)
        && !matches!(supplied_access, Some(ReferenceAccess::Mutable))
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

/// Report the same receiving obligation at storage and return destinations.
/// The caller supplies only diagnostic ownership, never reference permission.
/// Returning a value forwards the access it already has; neither return syntax
/// nor the declared result inserts a borrow. Loan lifetime and result-region
/// correspondence remain with their independent custody owners.
pub(crate) fn report_reference_access_conflict(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    value: ExpressionHandle,
    required: TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> bool {
    if !is_reference_destination(program, required) {
        return false;
    }
    let Some(refusal) = receiving_reference_access_refusal(
        program,
        machine,
        state,
        prior_statements,
        value,
        required,
    ) else {
        return false;
    };
    let reason = match refusal {
        ReferenceAccessRefusal::ExplicitWriteOnlyRequired => {
            "requires explicit write-only attenuation; pass `&write ...`"
        }
        ReferenceAccessRefusal::WriteOnlyMayNotRead => {
            "may read, but the source supplies only write-only authority"
        }
        ReferenceAccessRefusal::ExclusiveBorrowSourceRequired { access, .. }
            if access == language_semantics::ReferenceAccess::WriteOnly =>
        {
            "requires exclusive source access; shared or immutable storage cannot supply it"
        }
        ReferenceAccessRefusal::ExclusiveBorrowSourceRequired { .. }
        | ReferenceAccessRefusal::MutableSourceRequired => {
            "requires mutable source access; shared or immutable storage cannot supply it"
        }
        ReferenceAccessRefusal::NamedPlaceRequired { .. } => {
            "requires a named source place; a temporary cannot lend exclusive access"
        }
        ReferenceAccessRefusal::BareRangeFormsSharedView => {
            "receives a bare range that forms a shared view; `bare-range-reference-access` spells the mutable subrange `&mut collection[start..end]`"
        }
    };
    diagnostics.push(diagnostics::Diagnostic::error(format!("{owner} {reason}")));
    true
}

/// A completed borrowing operation: its exact storage source and the access
/// spelled at its formation.
struct CompletedBorrow {
    source: ExpressionHandle,
    access: language_semantics::ReferenceAccess,
    /// A bare window `coll[a..b]` with no borrow spelling of its own, which
    /// `bare-range-reference-access` makes a shared view wherever received.
    bare_window: bool,
}

/// A completed borrowing operation retains its exact storage source and the
/// access spelled at its formation. The call-shape owner excludes declared
/// machines with the same spelling; the receiver's actual carrier distinguishes
/// the compiler collection operation. Window geometry retains its selected
/// builtin meaning. Neither a receiving declaration, parameter or field type
/// nor a slot's mutability can supply access, and this query establishes no
/// extent or loan.
fn completed_reference_borrow_source(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
) -> Option<CompletedBorrow> {
    use language_semantics::ReferenceAccess;

    // Ordinary borrows and representation views retain the same formation
    // coordinates. In particular, `reference as &mut Other` lends the
    // reference's backing without an outer Borrow node. Reuse the formation
    // owner so spelling does not lose access or let the view type create it;
    // the source permission and representation judgment remain independent.
    if let Some(formation) = super::recasts::reference_formation_source(program, value) {
        return Some(CompletedBorrow {
            source: formation.source,
            access: formation.access,
            bare_window: false,
        });
    }
    match program.expression_table.expression(value) {
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
            // `coll[a..b]` has no borrow spelling of its own, so nobody at
            // the formation spelled one: the window is the shared loan the
            // `[..]` adaptation forms, at a local declaration exactly as at a
            // call or constructed field, and `&mut coll[a..b]` lends mutably.
            // The collection's own declared access never widens a bare
            // window: a `&mut [T]` parameter forwards as `coll`, not as a
            // window of itself.
            Some(CompletedBorrow {
                source: indexed.collection,
                access: ReferenceAccess::Shared,
                bare_window: true,
            })
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
    crate::machine_calls::calls::call_result_type(program, argument)
        .and_then(|reference| declared_reference_access(program, reference))
}

/// Forward the selected reference permission, not the mutability of its slot.
/// An owned aggregate can carry an exclusive reference without a mutable local
/// binding. Every enclosing reference still limits access: reading that same
/// leaf through a shared aggregate cannot recover its exclusive permission.
fn value_forwarded_exclusive_reference_access(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    argument: ExpressionHandle,
) -> Option<language_semantics::ReferenceAccess> {
    let Some((root, binding_type, _)) = source_place_root(
        program,
        current_machine,
        current_state,
        prior_statements,
        argument,
    ) else {
        return None;
    };
    crate::value_custody::expression_types::place_forwarded_exclusive_reference_access(
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
    source: ExpressionHandle,
    access: language_semantics::ReferenceAccess,
) -> Option<ReferenceAccessRefusal> {
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

/// Assignment permission follows the access route independently of the
/// binding's mutability. A range writes backing; an ordinary selection writes
/// its slot. The existing root check owns whether that slot may be rebound.
pub(crate) fn assignment_place_permits_write(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    target: ExpressionHandle,
) -> bool {
    let Some((root, root_type, writable)) =
        source_place_root(program, machine, state, prior_statements, target)
    else {
        return false;
    };
    if let Some(indexed) = program.expression_table.indexed_node(target)
        && program.expression_table.indexed_on_range(indexed)
    {
        return crate::value_custody::expression_types::place_permits_reference_borrow(
            program,
            target,
            root,
            root_type,
            writable,
            language_semantics::ReferenceAccess::WriteOnly,
        );
    }
    crate::value_custody::expression_types::place_permits_storage_write(
        program, target, root, root_type,
    )
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
    let [_] = program.expression_table.name_path_members(path.members) else {
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

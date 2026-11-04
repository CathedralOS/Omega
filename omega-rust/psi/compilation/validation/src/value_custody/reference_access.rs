//! Receiving a reference value cannot manufacture the source's access.
//!
//! Calls, local initialization, returns and field construction share this judgment at the exact
//! receiving leaf. A mutable slot or byte-compatible shape is not authority
//! over its referent. Declared reference forwarding preserves every enclosing
//! access, while lifetime, exclusivity, element/domain correspondence and
//! storage admission remain independent checks at their existing owners.
//! An owned aggregate rvalue forwards a reference leaf the same way a named
//! binding does: `helper(..).window` lends the field's declared access, and a
//! shared shell around that aggregate still cannot recover it.
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
use crate::frozen_program::expression_table;
use facts::PlaceRoot;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

pub(crate) enum ReferenceAccessRefusal {
    ExplicitWriteOnlyRequired,
    WriteOnlyMayNotRead,
    MutableSourceRequired,
    ExclusiveBorrowSourceRequired {
        root: PlaceRoot,
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
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> Option<ReferenceAccessRefusal> {
    use language_semantics::ReferenceAccess;
    // Selection completes one of the retained arm values. Every possible
    // result must supply the same receiving permission; the destination's
    // access and the selected subject cannot supply it for an arm. Pattern
    // effects, coverage and each arm's loans keep their ordinary formation
    // judgments. Reuse those exact result handles rather than deriving one
    // access from a representative arm or flattening the source expression.
    if let ExpressionNode::Match(dispatch) = expression_table(program).expression(value) {
        return expression_table(program)
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
                    required_substitutions,
                )
            });
    }
    let (applied_required, _) = super::expression_types::substituted_required_reference(
        program,
        required,
        required_substitutions,
    );
    let expected_access = declared_reference_access(program, applied_required);
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
        })
        .or_else(|| {
            crate::value_custody::expression_types::owned_rvalue_exclusive_reference_access(
                program, value,
            )
        });
    if let Some(borrow) = &completed_borrow
        && let Some(refusal) = borrow_source_access_refusal(
            program,
            machine,
            state,
            prior_statements,
            borrow.source,
            borrow.access,
            borrow.interpretation
                == Some(super::recasts::ReferenceFormationInterpretation::Declared)
                && super::expression_types::borrow_selects_reference_storage(
                    program,
                    borrow.source,
                    required,
                    required_substitutions,
                ),
        )
    {
        return Some(refusal);
    }
    // Attenuation must be explicit where a new write-only loan is formed.
    // An ordinary call has already produced its declared loan; receiving that
    // descriptor neither forms another borrow nor observes its referent.
    if expected_access == Some(ReferenceAccess::WriteOnly)
        && supplied_access != Some(ReferenceAccess::WriteOnly)
        && !matches!(expression_table(program).expression(value),
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
            .is_some_and(|borrow| borrow.interpretation.is_none())
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
        &[],
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
        ReferenceAccessRefusal::ExclusiveBorrowSourceRequired {
            access: language_semantics::ReferenceAccess::WriteOnly,
            ..
        } => "requires exclusive source access; shared or immutable storage cannot supply it",
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
    /// None is the implicit shared borrow of a bare range; a declared borrow
    /// may instead select exact descriptor storage through its receiving type.
    interpretation: Option<super::recasts::ReferenceFormationInterpretation>,
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
            interpretation: Some(formation.interpretation),
        });
    }
    match expression_table(program).expression(value) {
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
                interpretation: None,
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
    let (root, binding_type, _) = source_place_root(
        program,
        current_machine,
        current_state,
        prior_statements,
        argument,
    )?;
    crate::value_custody::expression_types::place_forwarded_exclusive_reference_access(
        program,
        argument,
        root,
        binding_type,
    )
}

/// Every borrowing operation lends its source place's own access. A
/// representation view grants neither readability nor writability; shared
/// formation must reject a write-only source just as mutable formation does.
fn borrow_source_access_refusal(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    source: ExpressionHandle,
    access: language_semantics::ReferenceAccess,
    lends_storage: bool,
) -> Option<ReferenceAccessRefusal> {
    let Some((root, binding_type, owned_root_is_writable)) =
        source_place_root(program, machine, state, prior_statements, source)
    else {
        // Shared borrows may lend a computed value's temporary storage; its
        // producer and lifetime checks own that case. Exclusive loans still
        // require a named writable place.
        return access
            .is_exclusive()
            .then_some(ReferenceAccessRefusal::NamedPlaceRequired { access });
    };
    let permitted = if lends_storage {
        // Replacing a saved pointer needs its slot's permission, independently
        // of whether the old pointer could write its pointee. A denied slot
        // cannot fall back to the stronger access of that old pointer.
        super::expression_types::place_storage_borrow_access(
            program,
            source,
            root,
            binding_type,
            access,
        )
        .is_some_and(|storage| {
            !access.is_exclusive()
                || storage == super::expression_types::StorageWriteAccess::BorrowedStorage
                || reference_slot_is_writable(
                    program,
                    machine,
                    state,
                    prior_statements,
                    source,
                    root,
                )
        })
    } else {
        super::expression_types::place_permits_reference_borrow(
            program,
            source,
            root,
            binding_type,
            owned_root_is_writable,
            access,
        )
    };
    (!permitted).then_some(if access == language_semantics::ReferenceAccess::Shared {
        ReferenceAccessRefusal::WriteOnlyMayNotRead
    } else {
        ReferenceAccessRefusal::ExclusiveBorrowSourceRequired { root, access }
    })
}

fn reference_slot_is_writable(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    source: ExpressionHandle,
    root: PlaceRoot,
) -> bool {
    let Some(place) = crate::canonical_place_from_expression(program, source) else {
        return false;
    };
    // Owned record contents keep their ordinary initialization/storage rule.
    // A bare descriptor binding additionally needs replacement permission.
    if !place.segments.is_empty() {
        return true;
    }
    let Some(symbol) = root.as_symbol() else {
        return false;
    };
    prior_statements.iter().any(|statement| {
        statement
            .as_local_data()
            .is_some_and(|local| local.symbol == symbol && local.is_mutable)
    }) || state.is_some_and(|state| {
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == symbol && parameter.is_mutable)
    }) || program
        .machine_owned_data_by_symbol(machine, symbol)
        .is_some()
}

/// Assignment permission follows the typed access route. A range or incoming
/// exclusive reference parameter writes its backing under reference permission.
/// Other writes use the selected endpoint's access, independently of the owned
/// root's mutability; descriptor replacement still needs writable slot storage.
pub(crate) fn assignment_storage_write_access(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    target: ExpressionHandle,
) -> Option<super::expression_types::StorageWriteAccess> {
    let (root, root_type, writable) =
        source_place_root(program, machine, state, prior_statements, target)?;
    let writes_parameter_referent = state.is_some_and(|state| {
        crate::value_custody::places::assignment_targets_parameter_referent(program, state, target)
    });
    let writes_range_backing = expression_table(program)
        .indexed_node(target)
        .is_some_and(|indexed| expression_table(program).indexed_on_range(indexed));
    if writes_parameter_referent || writes_range_backing {
        return crate::value_custody::expression_types::place_permits_reference_borrow(
            program,
            target,
            root,
            root_type,
            writable,
            language_semantics::ReferenceAccess::WriteOnly,
        )
        .then_some(super::expression_types::StorageWriteAccess::BorrowedStorage);
    }
    crate::value_custody::expression_types::place_storage_write_access(
        program, target, root, root_type,
    )
}

fn source_place_root(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&State>,
    prior_statements: &[typed_trees::statement::StatementNode],
    argument: ExpressionHandle,
) -> Option<(PlaceRoot, TypeReferenceHandle, bool)> {
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
        let receiver = match expression_table(program).expression(root) {
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            ExpressionNode::Name(_) => break,
            ExpressionNode::SavedSubject(saved) => {
                let source = program.saved_value_source(saved)?;
                let reference = if let Some(state) = current_state {
                    super::expression_types::expression_result_type_reference(
                        program,
                        current_machine,
                        state,
                        source,
                    )?
                } else {
                    crate::value_custody::places::declared_place_type_raw(
                        program,
                        current_machine,
                        current_state,
                        source,
                    )?
                };
                let anchor = expression_table(program)
                    .value_capture(saved.capture)
                    .value_anchor;
                if !expression_table(program).expression_is_valid(anchor) {
                    return None;
                }
                // The match owns completed subject storage. Its declared
                // reference shells still attenuate every projected borrow;
                // exact supplying loans are checked independently in flow.
                return Some((PlaceRoot::Expression(anchor), reference, true));
            }
            _ => return None,
        };
        root = receiver;
    }
    let path = expression_table(program).name_path_node(root)?;
    let [_] = expression_table(program).name_path_members(path.members) else {
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
        return Some((
            PlaceRoot::Symbol(path.symbol),
            binding_type,
            owned_root_is_writable,
        ));
    }
    program
        .machine_owned_data_by_symbol(current_machine, path.symbol)
        .map(|owned| (PlaceRoot::Symbol(path.symbol), owned.type_reference, true))
}

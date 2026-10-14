//! The `let` a structural result initializes, recorded on the result's
//! binding plan where planning pairs the result with its home. Lowering then
//! names the local, its declared type and its binding's custody from the plan
//! instead of re-reading the authored statement.
//!
//! The custody classifies the type the binding holds. A declared view
//! (`let v: &[u8] = coll[a..b]`) holds its declaration; an element-declared
//! view (`let v: u8 = coll[a..b]`) holds the initializer's own view result.
//! Exclusive descriptors retain their declared mutable or write-only access.
//! Treating a returned write-only descriptor as owned would lose its lending
//! contract; treating it as mutable would manufacture observation authority.

use checked_trees::{CheckedInitializedLocal, CheckedStructuralAccess};
use typed_trees::data::DataMember;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(in crate::execution::terminal_unit) fn initialized_local(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    local: &typed_trees::statement::TableLocalData,
) -> CheckedInitializedLocal {
    let bound = bound_type(program, machine, state, local);
    let access = if is_shared_borrow(program, bound) {
        CheckedStructuralAccess::SharedBorrow
    } else if let Some(shell) = validation::reference_shell(program, bound)
        && shell.access.is_exclusive()
    {
        shell.access.into()
    } else {
        CheckedStructuralAccess::Owned
    };
    CheckedInitializedLocal {
        symbol: local.symbol,
        declared_type: local.type_reference,
        access,
    }
}

fn bound_type(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    local: &typed_trees::statement::TableLocalData,
) -> TypeReferenceHandle {
    if validation::reference_shell(program, local.type_reference).is_some()
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
    {
        return local.type_reference;
    }
    validation::expression_result_type_reference(program, machine, state, local.initial_value)
        .filter(|result| {
            result.is_valid() && validation::slice_view_shell(program, *result, &[]).is_some()
        })
        .unwrap_or(local.type_reference)
}

/// A shared borrow observes its referent without moving it: a named record,
/// a slice view, or a primitive with linear owned contents. Constraint shells
/// qualify the carrier without changing its custody (`&[u8] in Utf8` is a
/// shared view). A sum referent and a `&mut` carrier are not shared-borrow
/// bindings.
fn is_shared_borrow(
    program: &dyn validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    if validation::slice_view_shell(program, reference, &[])
        .is_some_and(|view| view.access == language_semantics::ReferenceAccess::Shared)
    {
        return true;
    }
    let Some(shell) = validation::reference_shell(program, reference) else {
        return false;
    };
    if shell.access != language_semantics::ReferenceAccess::Shared {
        return false;
    }
    if program.primitive_type_reference(shell.referee).is_some() {
        return validation::has_contents(
            program,
            shell.referee,
            &[],
            validation::ContentsRequirement::LinearOwned,
        );
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(shell.referee)
    else {
        return false;
    };
    program
        .data_definition_by_symbol(*symbol)
        .is_some_and(|record| {
            !program
                .data_members(record)
                .iter()
                .any(|member| matches!(member, DataMember::Variant(_)))
        })
}

//! Exact sealed declaration and ordering-case custody for portable fences.
//!
//! An ordinary authored machine with the same name has its own semantics.
//! Only the retained operator from the sealed toolchain file can produce the
//! fence event. The selected ordering still needs a live case proof at its call.

use crate::CheckingInput;
use language_core::atomic::{ATOMIC_CORE_SOURCE, FenceOrdering};
use symbols::SymbolHandle;
use typed_trees::operator::OperatorDefinition;

pub struct AtomicFenceContract {
    pub declaration: SymbolHandle,
    pub ordering_type: typed_trees::types::TypeReferenceHandle,
    pub cases: [(SymbolHandle, FenceOrdering); 3],
}

fn sealed_identity(program: &dyn CheckingInput, symbol: SymbolHandle, identity: &str) -> bool {
    program
        .normalized_hermetic_symbol_identity(symbol)
        .ok()
        .as_deref()
        == Some(identity)
        && program
            .symbol_toolchain_relative_path(symbol)
            .is_some_and(|path| path == std::path::Path::new(ATOMIC_CORE_SOURCE))
}

/// Declaration admission joins the complete signature and sealed enum roster.
/// The returned case handles, rather than their source spellings, select the
/// ordering at each use.
pub fn exact_toolchain_atomic_fence_contract(
    program: &dyn CheckingInput,
    operator: &OperatorDefinition,
) -> Option<AtomicFenceContract> {
    if !sealed_identity(program, operator.symbol, "toolchain::Atomic::fence")
        || !operator.is_public
        || operator.is_boundary
        || operator.spelling.is_some()
        || !operator.lifetime_parameters.is_empty()
        || !program.operator_type_parameters(operator).is_empty()
        || !program.operator_contracts(operator).is_empty()
        || operator.return_type.is_valid()
    {
        return None;
    }
    let [parameter] = program.operator_parameters(operator) else {
        return None;
    };
    if parameter.is_const || parameter.is_mutable || parameter.relevance.is_erased() {
        return None;
    }
    let (data, symbol) = super::intrinsic_boundaries::checked_shape_data_declaration(
        program,
        parameter.type_reference,
        "FenceOrdering",
    )?;
    if !sealed_identity(program, symbol, "toolchain::FenceOrdering")
        || data.properties.multiplicity != language_semantics::Multiplicity::Unrestricted
    {
        return None;
    }
    let members = program.data_members(data);
    if members.len() != 3 {
        return None;
    }
    let mut cases = [(SymbolHandle::invalid(), FenceOrdering::Receive); 3];
    for (index, (member, (name, ordering))) in members
        .iter()
        .zip([
            ("Receive", FenceOrdering::Receive),
            ("Publish", FenceOrdering::Publish),
            ("ReceivePublish", FenceOrdering::ReceivePublish),
        ])
        .enumerate()
    {
        let typed_trees::data::DataMember::Variant(case) = member else {
            return None;
        };
        if case.name.as_str() != name || !case.payload.is_empty() || !case.where_facts.is_empty() {
            return None;
        }
        cases[index] = (case.symbol, ordering);
    }
    Some(AtomicFenceContract {
        declaration: operator.symbol,
        ordering_type: parameter.type_reference,
        cases,
    })
}

pub(crate) fn validate_atomic_fence_declarations(
    program: &dyn CheckingInput,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    for operator in program.operators() {
        if sealed_identity(program, operator.symbol, "toolchain::Atomic::fence")
            && exact_toolchain_atomic_fence_contract(program, operator).is_none()
        {
            diagnostics.push(diagnostics::Diagnostic::error(
                "sealed Atomic::fence must take exactly the sealed copy FenceOrdering with Receive, Publish and ReceivePublish cases and return Unit"));
        }
    }
}

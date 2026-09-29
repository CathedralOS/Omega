//! Constructing an owned bounded byte local from a quoted literal.
//!
//! `let text: [u8; 8] in Utf8 = "150";` constructs a bounded carrier. A
//! domain-qualified fixed byte array owns its declared capacity and a live
//! length (wiki/spec/language/domains.md, "Byte containers and encoding
//! domains"): construction copies the literal's octets and sets the live
//! length to their count, 3 here. It proves that count fits the capacity and
//! neither truncates nor pads; unused capacity is not live content. A raw
//! `[u8; N]` keeps its exact-length rule and never reaches this producer,
//! because its carrier is a fixed array rather than a bounded owner.
//!
//! The plan reuses the literal establishment a borrowed `&[u8]` local already
//! uses. The binding's declared carrier, not another operation kind, tells
//! lowering to copy the octets into activation-owned backing instead of
//! lending immutable image storage. Every declared domain must be a plain
//! byte-predicate domain that the literal's own octets satisfy, which is the
//! same judgment a bounded field store from a literal makes; Terminal replays
//! the predicates from the octets rather than trusting this plan.
//!
//! Boundary: stage 05 still emits only borrowed-view literal establishments
//! and refuses this carrier, so the local does not yet reach native code.
use super::{CheckedStructuralAccess, ExpressionNode, Multiplicity, TypedTrees};
use crate::execution::terminal_unit::types::{
    byte_sequence_carrier, structural_access_for_type_reference,
};
use checked_trees::CheckedByteSequenceCarrier;

/// The literal octets an immutable owned bounded byte local is constructed
/// from, or `None` when the local is not that shape. The caller owns the
/// binding ordinal and publishes the ordinary literal establishment.
pub(super) fn literal_octets(
    program: &TypedTrees,
    local: &typed_trees::statement::TableLocalData,
) -> Option<Vec<u8>> {
    if local.is_mutable
        || program.type_multiplicity(local.type_reference) != Multiplicity::Unrestricted
        || structural_access_for_type_reference(program, local.type_reference)
            != Some(CheckedStructuralAccess::Owned)
    {
        return None;
    }
    let CheckedByteSequenceCarrier::BoundedOwned { capacity } =
        byte_sequence_carrier(program, local.type_reference, &[])?
    else {
        return None;
    };
    let ExpressionNode::String(bytes) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    if u64::try_from(bytes.len()).ok()? > capacity {
        return None;
    }
    let domains =
        crate::facts::field_domain::domain_constraint_symbols(program, local.type_reference);
    let plain_literal_domain = |symbol| {
        program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
            .is_some_and(|domain| {
                domain.establishment_routes.is_empty()
                    && domain.semantic_roles == Default::default()
            })
            && crate::facts::field_domain::string_literal_expression_grants_domain(
                program,
                local.initial_value,
                symbol,
            )
    };
    domains
        .into_iter()
        .all(plain_literal_domain)
        .then(|| bytes.to_vec())
}

#[cfg(test)]
mod tests;

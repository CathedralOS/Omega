//! Compiler-recognized BYTE-SEQUENCE classifier predicates -- the reusable
//! building blocks a domain selects by spelling one as its
//! `when <predicate>(self)` classifier (`domain [u8]::Utf8 when
//! valid_utf8(self)`). Moved here from the checker's `field_domain.rs`
//! (2026-07-16) so the RUNTIME decode boundary shares ONE vocabulary with
//! the compile-time proof machinery: wire decode brings UNTRUSTED bytes
//! where no compile-time proof exists, and the decoder must evaluate the
//! same predicate the checker proves elsewhere (`holds_for`). The ENUM
//! itself lives in `omega_core::byte_predicates` (dependency-free, so the
//! instruction kinds can carry predicate MASKS); this module owns the
//! TREE-WALKING resolution from domain declarations.

use crate::TypedTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use omega_core::symbols::SymbolHandle;

pub use omega_core::byte_predicates::ByteSequencePredicate;

/// If `domain_symbol`'s declared classifier is a recognized comptime
/// byte-predicate call applied to `self` (e.g. `when valid_utf8(self)`), return
/// that primitive. Any other classifier shape (a `self.field` comparison, a
/// `self in Type::Case` subset, an unknown call, or no classifier at all) is
/// not recognized.
pub fn domain_classifier_byte_predicate(
    program: &TypedTrees,
    domain_symbol: SymbolHandle,
) -> Option<ByteSequencePredicate> {
    let domain = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)?;
    if !domain.classifier.is_valid() {
        return None;
    }

    let ExpressionNode::Call(call) = program.expression_table.expression(domain.classifier) else {
        return None;
    };
    // A free-function predicate over `self`: no receiver, exactly one argument,
    // and that argument is the bare `self` subject the classifier scrutinizes.
    if call.receiver.is_valid() {
        return None;
    }
    let predicate = ByteSequencePredicate::from_name(call.target.as_str())?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    let [argument] = arguments else {
        return None;
    };
    if !expression_is_self_reference(program, *argument) {
        return None;
    }
    Some(predicate)
}

/// The DOMAIN constraints declared on a type reference (walking Reference and
/// Constrained wrappers), resolved by NAME against the program's domain
/// definitions: `(domain name, recognized predicate)` per constraint. An inner
/// `None` predicate = a declared domain whose classifier is NOT a recognized
/// byte predicate -- the decode boundary must refuse LOUDLY rather than skip
/// validation silently.
pub fn type_reference_domain_predicates(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<(String, Option<ByteSequencePredicate>)> {
    let mut predicates = Vec::new();
    let mut handle = type_reference;
    loop {
        if !handle.is_valid() {
            return predicates;
        }
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => handle = *referee,
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    let TypeConstraintNode::Domain(name) = constraint else {
                        continue;
                    };
                    // A domain's stored name is carrier-qualified
                    // (`[u8]::Utf8`) while the constraint spells the bare
                    // segment (`Utf8`): match the LAST `::` segment, the
                    // same rule as the checker's `resolve_domain_symbol`.
                    let predicate = program
                        .domain_definitions()
                        .iter()
                        .find(|domain| {
                            let full = domain.name.as_str();
                            full.rsplit("::").next().unwrap_or(full) == name.as_str()
                        })
                        .and_then(|domain| {
                            domain_classifier_byte_predicate(program, domain.symbol)
                        });
                    predicates.push((name.as_str().to_owned(), predicate));
                }
                handle = *base_type;
            }
            _ => return predicates,
        }
    }
}

/// Whether `expression` is the bare classifier subject `self` -- a single-member
/// name path spelled `self`. The classifier predicate must apply to `self` (the
/// value being classified), not to some unrelated place.
fn expression_is_self_reference(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    let members = program.expression_table.name_path_members(path.members);
    matches!(members, [member] if member.as_str() == "self")
}

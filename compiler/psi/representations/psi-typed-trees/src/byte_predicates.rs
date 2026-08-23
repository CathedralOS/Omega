//! Compiler-recognized BYTE-SEQUENCE predicate facts -- the reusable
//! building blocks a domain selects by spelling one as an ordinary fact
//! (`domain [u8]::Utf8 { valid_utf8(self); }`). Moved here from the checker's `field_domain.rs`
//! (2026-07-16) so the RUNTIME decode boundary shares ONE vocabulary with
//! the compile-time proof machinery: wire decode brings UNTRUSTED bytes
//! where no compile-time proof exists, and the decoder must evaluate the
//! same predicate the checker proves elsewhere (`holds_for`). The ENUM
//! itself lives in `psi_language_semantics::byte_predicates` (dependency-free, so the
//! instruction kinds can carry predicate MASKS); this module owns the
//! TREE-WALKING resolution from domain declarations.

use crate::TypedTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use psi_symbols::SymbolHandle;

pub use psi_language_semantics::byte_predicates::ByteSequencePredicate;

/// If `domain_symbol`'s sole fact is a recognized comptime byte-predicate call
/// applied to `self` (e.g. `valid_utf8(self);`), return that primitive. Domains
/// with additional facts require general proof evaluation and are not reduced
/// to a single byte predicate.
pub fn domain_byte_predicate(
    program: &TypedTrees,
    domain_symbol: SymbolHandle,
) -> Option<ByteSequencePredicate> {
    let domain = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)?;
    let [crate::domain::ProofFact::Expression(expression)] =
        program.proof_facts.span_or_empty(domain.facts)
    else {
        return None;
    };

    let ExpressionNode::Call(call) = program.expression_table.expression(*expression) else {
        return None;
    };
    // A free-function predicate over `self`: no receiver, exactly one argument,
    // and that argument is the bare `self` subject the fact scrutinizes.
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
/// `None` predicate means the domain is not exactly one recognized byte fact --
/// the decode boundary must refuse LOUDLY rather than skip validation silently.
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
                    let TypeConstraintNode::Domain(domain_constraint) = constraint else {
                        continue;
                    };
                    // The carrier-aware typed normalization pass already
                    // selected the declaration; never repeat a global lookup
                    // from the authored short name here.
                    let predicate = domain_constraint
                        .symbol
                        .is_valid()
                        .then(|| domain_byte_predicate(program, domain_constraint.symbol))
                        .flatten();
                    predicates.push((domain_constraint.name.as_str().to_owned(), predicate));
                }
                handle = *base_type;
            }
            _ => return predicates,
        }
    }
}

/// Whether `expression` is the bare domain subject `self` -- a single-member
/// name path spelled `self`. The predicate must apply to the domain value, not
/// to some unrelated place.
fn expression_is_self_reference(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    let members = program.expression_table.name_path_members(path.members);
    matches!(members, [member] if member.as_str() == "self")
}

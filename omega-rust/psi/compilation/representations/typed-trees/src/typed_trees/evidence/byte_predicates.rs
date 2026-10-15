//! Compiler-recognized BYTE-SEQUENCE predicate facts -- the reusable
//! building blocks a domain selects by spelling one as an ordinary fact
//! (`domain [u8]::Utf8(bytes) requires valid_utf8(bytes);`). Moved here from the checker's `field_domain.rs`
//! (2026-07-16) so the RUNTIME decode boundary shares ONE vocabulary with
//! the compile-time proof machinery: wire decode brings UNTRUSTED bytes
//! where no compile-time proof exists, and the decoder must evaluate the
//! same predicate the checker proves elsewhere (`holds_for`). The ENUM
//! itself lives in `language_semantics::byte_predicates` (dependency-free, so the
//! instruction kinds can carry predicate MASKS); this module owns the
//! TREE-WALKING resolution from domain declarations.

use crate::TypedTrees;
use crate::expression::ExpressionHandle;
use crate::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

pub use language_semantics::byte_predicates::ByteSequencePredicate;

/// If `expression` is a `pred(operand)` byte-predicate call — the reserved
/// unary boolean probe over a byte-carrier operand — return the named
/// primitive and its sole operand. A resolved declaration always wins over
/// the compiler vocabulary (the same precedence `checked_call_intrinsic`
/// enforces), receiver calls cannot select the free probes, and the probe is
/// unary: any other arity is a malformed application, not the predicate.
pub fn byte_predicate_call(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(ByteSequencePredicate, ExpressionHandle)> {
    let call = program.expression_table.call_node(expression)?;
    if call.target_symbol.is_valid() || call.receiver.is_valid() {
        return None;
    }
    let predicate = ByteSequencePredicate::from_name(call.target.as_str())?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    let [argument] = arguments else {
        return None;
    };
    Some((predicate, *argument))
}

/// If `domain_symbol`'s sole fact is a recognized comptime byte-predicate call
/// applied to its declared subject, return that primitive. Domains
/// with additional facts require general proof evaluation and are not reduced
/// to a single byte predicate. The same is true of the surrounding theory:
/// a transparent alias expands to constituent requirements, index binders or
/// instance arguments carry an identity byte extraction cannot substitute
/// into, and `established by` routes restrict who may mint membership at all
/// (`domain_requires_provenance` enforces the same list for writes) -- so any
/// of them keeps the domain a general proof obligation no byte check closes.
pub fn domain_byte_predicate(
    program: &TypedTrees,
    domain_symbol: SymbolHandle,
) -> Option<ByteSequencePredicate> {
    let domain = program.domain_definition(domain_symbol)?;
    if domain.alias.is_some()
        || !domain.index_arguments.is_empty()
        || !domain.establishment_routes.is_empty()
        || !crate::domain::index_parameters(program, domain).is_empty()
    {
        return None;
    }
    let [crate::domain::ProofFact::Expression(expression)] =
        program.proof_facts.span_or_empty(domain.facts)
    else {
        return None;
    };
    let (predicate, argument) = byte_predicate_call(program, *expression)?;
    crate::domain::domain_subject_reference(program, domain, argument).then_some(predicate)
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
        handle = program
            .type_reference_table
            .dereferenced_type_reference(handle);
        match program.type_reference_table.type_reference(handle) {
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

/// The exact octets of a quoted byte literal or a fixed array of byte-valued
/// integer literals. Contextual fixed-array landing changes syntax, not the
/// bytes available to predicate proof. No runtime value or cast supplies facts.
pub fn literal_bytes(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<std::borrow::Cow<'_, [u8]>> {
    match program.expression_table.expression(expression) {
        crate::expression::ExpressionNode::String(bytes) => Some(std::borrow::Cow::Borrowed(bytes)),
        crate::expression::ExpressionNode::ArrayLiteral(elements) => {
            let bytes = program
                .expression_table
                .expression_handles(*elements)
                .iter()
                .map(|element| {
                    let crate::expression::ExpressionNode::Integer(integer) =
                        program.expression_table.expression(*element)
                    else {
                        return None;
                    };
                    u8::try_from(integer.value_u64()?).ok()
                })
                .collect::<Option<Vec<_>>>()?;
            Some(std::borrow::Cow::Owned(bytes))
        }
        _ => None,
    }
}

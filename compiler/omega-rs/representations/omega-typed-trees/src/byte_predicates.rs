//! Compiler-recognized BYTE-SEQUENCE classifier predicates -- the reusable
//! building blocks a domain selects by spelling one as its
//! `when <predicate>(self)` classifier (`domain [u8]::Utf8 when
//! valid_utf8(self)`). Moved here from the checker's `field_domain.rs`
//! (2026-07-16) so the RUNTIME decode boundary shares ONE vocabulary with
//! the compile-time proof machinery: wire decode brings UNTRUSTED bytes
//! where no compile-time proof exists, and the decoder must evaluate the
//! same predicate the checker proves elsewhere (`holds_for`).

use crate::TypedTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use omega_core::symbols::SymbolHandle;

/// A compiler-recognized comptime byte-predicate primitive over a byte
/// sequence. These are reusable building blocks (like `+`/`==`), NOT
/// domain-specific.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteSequencePredicate {
    /// `valid_utf8(self)`: the bytes are well-formed UTF-8.
    ValidUtf8,
    /// `no_nul(self)`: no byte is `0x00`.
    NoNul,
    /// `ascii_only(self)`: every byte is < 128.
    AsciiOnly,
    /// `non_empty(self)`: the sequence has at least one byte. Notably does NOT
    /// hold for the empty/ZII value -- the means to exercise an empty-violating
    /// domain (see the checker's `domain_admits_empty_byte_sequence`).
    NonEmpty,
}

impl ByteSequencePredicate {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "valid_utf8" => Some(Self::ValidUtf8),
            "no_nul" => Some(Self::NoNul),
            "ascii_only" => Some(Self::AsciiOnly),
            "non_empty" => Some(Self::NonEmpty),
            _ => None,
        }
    }

    /// Evaluate the predicate over raw bytes: the comptime literal check AND
    /// the runtime decode-boundary validator share this one definition.
    pub fn holds_for(self, bytes: &[u8]) -> bool {
        match self {
            Self::ValidUtf8 => std::str::from_utf8(bytes).is_ok(),
            Self::NoNul => !bytes.contains(&0),
            Self::AsciiOnly => bytes.iter().all(|byte| *byte < 128),
            Self::NonEmpty => !bytes.is_empty(),
        }
    }

    /// Whether `predicate(a) && predicate(b)` implies `predicate(a ++ b)`: the
    /// classifier is preserved under byte-sequence concatenation. All four
    /// recognized predicates are concat-preserving -- concatenating two
    /// valid-UTF-8 / nul-free / ASCII-only / non-empty sequences yields one of
    /// the same kind (UTF-8 sequences are self-delimiting, so a complete valid
    /// sequence followed by another is valid). A future predicate that is NOT
    /// concat-preserving (a fixed-length or parse-shaped one) must return
    /// `false` here so the concat-domain law does not admit it.
    pub fn is_concat_preserving(self) -> bool {
        match self {
            Self::ValidUtf8 | Self::NoNul | Self::AsciiOnly | Self::NonEmpty => true,
        }
    }

    /// Whether `predicate(x)` implies `predicate(x[a..b])` for EVERY contiguous
    /// subslice: the classifier is preserved under subslicing. True only for
    /// PER-BYTE character-class predicates -- `no_nul`/`ascii_only` classify each
    /// byte independently, so any subset of the bytes still satisfies them.
    /// `valid_utf8` is NOT subslice-preserving (a subslice can cut a multi-byte
    /// scalar) and `non_empty` is NOT (a `x[a..a]` subslice is empty). A future
    /// per-byte predicate would return `true`; any sequence-shaped one, `false`.
    pub fn is_subslice_preserving(self) -> bool {
        match self {
            Self::NoNul | Self::AsciiOnly => true,
            Self::ValidUtf8 | Self::NonEmpty => false,
        }
    }
}

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

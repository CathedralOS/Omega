//! ch8 construction-grant primitives: a string literal whose compile-time bytes
//! satisfy a domain's declared classifier predicate grants that domain WITHOUT a
//! validating boundary call. Shared by the call-argument requires discharge
//! (`calls.rs`) and the domain-field WRITE-enforcement discharge (`writes.rs`):
//! both must accept a literal flowing into a `&[u8] in Utf8` (or any
//! classifier-backed) target. The policy of which bytes are in a domain lives in
//! the DOMAIN declaration's `when` clause; this module only provides the reusable
//! comptime byte-predicate primitives and evaluates them per-literal. A domain
//! with no classifier (or an unrecognized/non-comptime one) grants nothing, so
//! the literal must flow through a runtime validator instead -- correct, not a
//! regression. There is NO hardcoded domain name here.

use omega_core::symbols::SymbolHandle;

/// Whether the string literal `expression`'s compile-time bytes satisfy
/// `domain_symbol`'s declared comptime byte-predicate classifier. `false` when
/// `expression` is not a string literal, or the domain has no recognized
/// comptime classifier.
pub(super) fn string_literal_expression_grants_domain(
    program: &omega_typed_trees::TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let omega_typed_trees::expression::ExpressionNode::String(literal) =
        program.expression_table.expression(expression)
    else {
        return false;
    };
    let Some(predicate) = domain_classifier_byte_predicate(program, domain_symbol) else {
        return false;
    };
    predicate.holds_for(literal.as_bytes())
}

/// A compiler-recognized comptime byte-predicate primitive over a byte sequence.
/// These are reusable building blocks (like `+`/`==`), NOT domain-specific: a
/// domain selects one by spelling it as its `when <predicate>(self)` classifier.
#[derive(Clone, Copy)]
enum ByteSequencePredicate {
    /// `valid_utf8(self)`: the bytes are well-formed UTF-8.
    ValidUtf8,
    /// `no_nul(self)`: no byte is `0x00`.
    NoNul,
    /// `ascii_only(self)`: every byte is < 128.
    AsciiOnly,
}

impl ByteSequencePredicate {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "valid_utf8" => Some(Self::ValidUtf8),
            "no_nul" => Some(Self::NoNul),
            "ascii_only" => Some(Self::AsciiOnly),
            _ => None,
        }
    }

    fn holds_for(self, bytes: &[u8]) -> bool {
        match self {
            Self::ValidUtf8 => std::str::from_utf8(bytes).is_ok(),
            Self::NoNul => !bytes.contains(&0),
            Self::AsciiOnly => bytes.iter().all(|byte| *byte < 128),
        }
    }
}

/// If `domain_symbol`'s declared classifier is a recognized comptime
/// byte-predicate call applied to `self` (e.g. `when valid_utf8(self)`), return
/// that primitive. Any other classifier shape (a `self.field` comparison, a
/// `self in Type::Case` subset, an unknown call, or no classifier at all) is not
/// a comptime byte-predicate, so no grant is implied.
fn domain_classifier_byte_predicate(
    program: &omega_typed_trees::TypedTrees,
    domain_symbol: SymbolHandle,
) -> Option<ByteSequencePredicate> {
    let domain = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)?;
    if !domain.classifier.is_valid() {
        return None;
    }

    let omega_typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(domain.classifier)
    else {
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

/// Whether `expression` is the bare classifier subject `self` -- a single-member
/// name path spelled `self`. The classifier predicate must apply to `self` (the
/// value being classified), not to some unrelated place.
fn expression_is_self_reference(
    program: &omega_typed_trees::TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let omega_typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(expression)
    else {
        return false;
    };
    let members = program.expression_table.name_path_members(path.members);
    matches!(members, [member] if member.as_str() == "self")
}

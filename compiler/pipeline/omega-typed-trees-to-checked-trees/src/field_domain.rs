//! Shared resolution of an encoding-DOMAIN refinement declared on a
//! machine-attached-data field (`out: &[u8] in Utf8`), used by both the
//! write-enforcement check (`checks::contracts::writes`) and the flow-stage
//! re-establishment of the field-domain invariant after a write
//! (`flow::transfers`). #66.
//!
//! This neutral crate-root module also owns the comptime byte-predicate
//! machinery (`ByteSequencePredicate`, `domain_classifier_byte_predicate`,
//! `string_literal_expression_grants_domain`) so it is reachable from BOTH the
//! checker (`checks/`, construction-grant discharge) and the fact-producer
//! (`semantic/`). The policy of which bytes are in a domain lives in the DOMAIN
//! declaration's `when` clause; this module only provides the reusable comptime
//! byte-predicate primitives and evaluates them per-literal. A domain with no
//! classifier (or an unrecognized/non-comptime one) grants nothing. There is NO
//! hardcoded domain name here.

use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionNode;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// The declared encoding-domain symbol of a `self.field` target, resolved
/// through the machine's ATTACHED DATA (`self` is not itself a place type).
/// `None` for any non-domained or non-`self.field` target.
pub(crate) fn target_field_domain_symbol(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    target: omega_typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let type_reference = attached_data_field_type(program, machine, target)?;
    let domain_name = domain_constraint_name(program, type_reference)?;
    resolve_domain_symbol(program, &domain_name)
}

/// The machine whose attached data owns the place a `self.field` target refers to.
pub(crate) fn machine_by_symbol<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
) -> Option<&'program Machine> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
}

/// Resolve a `self.field` target (`Member(Name(self), field)` or
/// `Name ["self", field]`) to the field's DECLARED type reference (constraints
/// intact) via the machine's attached data. Mirrors omega-proof
/// `obligations::attached_data_field_type` (#63).
pub(crate) fn attached_data_field_type(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let field_name = match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let ExpressionNode::Name(receiver) =
                program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            match program.expression_table.name_path_members(receiver.members) {
                [segment] if segment.as_str() == "self" => member.member.as_str().to_owned(),
                _ => return None,
            }
        }
        ExpressionNode::Name(path) => {
            match program.expression_table.name_path_members(path.members) {
                [receiver, field] if receiver.as_str() == "self" => field.as_str().to_owned(),
                _ => return None,
            }
        }
        _ => return None,
    };

    let attached = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == field_name =>
            {
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            _ => None,
        })
}

/// The short domain name (`Utf8`) declared on a type reference, looking through a
/// leading reference (`&[u8] in Utf8`).
pub(crate) fn domain_constraint_name(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<String> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => domain_constraint_name(program, *referee),
        TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .find_map(|constraint| match constraint {
                TypeConstraintNode::Domain(name) => Some(name.as_str().to_owned()),
                _ => None,
            }),
        _ => None,
    }
}

/// Resolve a short domain name (`Utf8`) to its declared domain symbol, matching
/// the trailing path segment of a domain definition's full name (`[u8]::Utf8`).
pub(crate) fn resolve_domain_symbol(
    program: &omega_typed_trees::TypedTrees,
    wanted: &str,
) -> Option<SymbolHandle> {
    program.domain_definitions().iter().find_map(|domain| {
        let full = domain.name.as_str();
        (full.rsplit("::").next().unwrap_or(full) == wanted).then_some(domain.symbol)
    })
}

// --- comptime byte-predicate machinery (moved here from
// `checks::contracts::grants` so the `semantic` fact-producer can reuse it) ---

/// Whether the string literal `expression`'s compile-time bytes satisfy
/// `domain_symbol`'s declared comptime byte-predicate classifier. `false` when
/// `expression` is not a string literal, or the domain has no recognized
/// comptime classifier.
pub(crate) fn string_literal_expression_grants_domain(
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

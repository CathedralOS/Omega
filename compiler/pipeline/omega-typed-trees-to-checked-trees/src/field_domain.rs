//! Shared resolution of an encoding-DOMAIN refinement declared on a
//! machine-attached-data field (`out: &[u8] in Utf8`), used by both the
//! write-enforcement check (`checks::contracts::writes`) and the flow-stage
//! re-establishment of the field-domain invariant after a write
//! (`flow::transfers`). #66.

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

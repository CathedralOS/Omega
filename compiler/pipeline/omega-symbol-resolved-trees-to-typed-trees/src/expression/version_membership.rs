//! Version match arm lowering (frozen decision 14, chapter 21 "Version
//! Matching").
//!
//! The parser desugars a version pattern arm (`Counter::v1(old) ->` /
//! `Counter(current) ->`) into a MARKER membership over the subject:
//! `subject in Counter::v1::<marker>` (the marker is
//! `omega_core::versioning::VERSION_ARM_MARKER`). This module resolves the
//! marker into the executable era tag compare `subject.era == <era index>`
//! against the builtin `Versioned<Counter>` container the symbol-resolved
//! lowering synthesized.
//!
//! Era numbering rides frozen decision 10's wire-era assignment: declared
//! `version vN` blocks count up from era 0 in declaration order; the CURRENT
//! shape's era is the number of declared blocks.
//!
//! Legality: version arms are legal ONLY on `Versioned<T>` subjects. The
//! subject's declared type is resolved best-effort at the RESOLVED level
//! (state parameters, data fields, machine-owned data); a determinably-plain
//! subject is rejected with the chapter-21 suggestion. A subject whose type
//! cannot be determined here (e.g. an inferred local) is lowered
//! optimistically against the arm's named container.

use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_core::versioning;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

/// Whether a membership domain path is a parser-synthesized version arm
/// marker (`Type::<marker>` or `Type::vN::<marker>`).
pub(crate) fn is_version_arm_domain(
    source: &resolved::expression::ExpressionTable,
    domain: HandleSpan<resolved::name::DiagnosticName>,
) -> bool {
    source
        .name_path_members(domain)
        .last()
        .is_some_and(|member| member.as_str() == versioning::VERSION_ARM_MARKER)
}

/// Lower a version arm marker membership into the era tag compare. `subject`
/// is the RESOLVED subject expression (for declared-type lookup); `value` is
/// its already-lowered TYPED counterpart.
pub(super) fn lower_version_membership_expression(
    program: &resolved::SymbolResolvedTrees,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    subject: resolved::expression::ExpressionHandle,
    value: typed::expression::ExpressionHandle,
    domain: HandleSpan<resolved::name::DiagnosticName>,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    let members = source.name_path_members(domain);
    let (type_name, version_name) = match members {
        [type_name, marker] if marker.as_str() == versioning::VERSION_ARM_MARKER => {
            (type_name.as_str(), None)
        }
        [type_name, version, marker] if marker.as_str() == versioning::VERSION_ARM_MARKER => {
            (type_name.as_str(), Some(version.as_str()))
        }
        _ => {
            return Err(Diagnostic::error(
                "malformed version match arm: expected `Type::vN(binding)` or `Type(binding)`",
            ));
        }
    };

    if !program
        .data_definitions
        .iter()
        .any(|definition| definition.name.as_str() == type_name)
    {
        return Err(Diagnostic::error(format!(
            "version match arm names unknown data type `{type_name}`"
        )));
    }

    // The declared eras of `type_name`, in declaration order (the historical
    // shapes are pushed immediately after their parent definition).
    let declared_versions = program
        .data_definitions
        .iter()
        .filter_map(|definition| {
            versioning::split_version_shape_name(definition.name.as_str())
                .filter(|(data_name, _)| *data_name == type_name)
                .map(|(_, version)| version)
        })
        .collect::<Vec<_>>();

    if declared_versions.is_empty() {
        return Err(Diagnostic::error(format!(
            "data `{type_name}` declares no `version vN` blocks, so `Versioned<{type_name}>` has no eras to match"
        )));
    }

    // Decision-10 era assignment: vN = zero-based declaration index; the
    // current shape = the number of declared blocks.
    let era = match version_name {
        Some(version) => match declared_versions
            .iter()
            .position(|declared| *declared == version)
        {
            Some(index) => index as i64,
            None => {
                return Err(Diagnostic::error(format!(
                    "data `{type_name}` declares no version `{version}`; declared eras: {}, current",
                    declared_versions.join(", ")
                )));
            }
        },
        None => declared_versions.len() as i64,
    };

    // Best-effort subject legality: a subject with a DETERMINABLE plain type
    // is rejected here with the chapter-21 suggestion; an undeterminable
    // subject lowers optimistically (its members then resolve, or fail,
    // against the container shape the arm names).
    let container_name = versioning::versioned_container_name(type_name);
    if let Some(subject_type) = subject_declared_type_name(program, source, subject) {
        if subject_type != container_name {
            if versioning::versioned_container_payload(subject_type).is_some() {
                return Err(Diagnostic::error(format!(
                    "version match arm `{type_name}{}` does not match the subject's container `{subject_type}`",
                    version_name
                        .map(|version| format!("::{version}"))
                        .unwrap_or_default()
                )));
            }
            return Err(Diagnostic::error(format!(
                "version match arms require a `Versioned<{type_name}>` subject: `{subject_type}` is a plain value and carries no era tag -- receive it through the boundary container `Versioned<{type_name}>`"
            )));
        }
    }

    let era_member = target.insert(typed::expression::ExpressionNode::Member(
        typed::expression::TableMemberExpression {
            receiver: value,
            member_symbol: container_era_field_symbol(program, &container_name),
            member: typed::name::Identifier::generated(versioning::VERSIONED_ERA_FIELD),
            case_variant: None,
        },
    ));
    let era_literal = target.insert(typed::expression::ExpressionNode::Integer(era));

    Ok(target.insert(typed::expression::ExpressionNode::Binary(
        typed::expression::TableBinaryExpression {
            left: era_member,
            operator: typed::expression::BinaryOperator::Equal,
            right: era_literal,
        },
    )))
}

/// The synthesized container's `era` field symbol, when the symbol table has
/// it (it always should; an invalid handle degrades to the by-name member
/// resolution every nested field read already uses).
fn container_era_field_symbol(
    program: &resolved::SymbolResolvedTrees,
    container_name: &str,
) -> SymbolHandle {
    program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == container_name)
        .and_then(|definition| {
            program
                .data_members(definition.members)
                .iter()
                .find_map(|member| match member {
                    resolved::data::DataMember::Field(field)
                        if field.name.as_str() == versioning::VERSIONED_ERA_FIELD =>
                    {
                        Some(field.symbol)
                    }
                    _ => None,
                })
        })
        .unwrap_or_else(SymbolHandle::invalid)
}

/// Reject a source-level write INTO a `Versioned<T>` container (frozen
/// decision 14: constructed only at boundaries; `era` is read-only). Walks
/// the assignment target's member spine and errors when any link's receiver
/// has a determinable container type.
pub(crate) fn versioned_interior_write_error(
    program: &resolved::SymbolResolvedTrees,
    source: &resolved::expression::ExpressionTable,
    target: resolved::expression::ExpressionHandle,
) -> Option<Diagnostic> {
    match source.expression(target) {
        resolved::expression::ExpressionNode::Member(member) => {
            if let Some(receiver_type) =
                subject_declared_type_name(program, source, member.receiver)
                && versioning::versioned_container_payload(receiver_type).is_some()
            {
                let member_name = member.member.as_str();
                if member_name == versioning::VERSIONED_ERA_FIELD {
                    return Some(Diagnostic::error(format!(
                        "`era` of `{receiver_type}` is read-only: the era tag is set only by the boundary that constructs the container"
                    )));
                }
                return Some(Diagnostic::error(format!(
                    "cannot write into `{receiver_type}`: a `Versioned<T>` container is constructed only at boundaries"
                )));
            }
            versioned_interior_write_error(program, source, member.receiver)
        }
        resolved::expression::ExpressionNode::Indexed(indexed) => {
            versioned_interior_write_error(program, source, indexed.collection)
        }
        resolved::expression::ExpressionNode::Mutable(inner) => {
            versioned_interior_write_error(program, source, *inner)
        }
        _ => None,
    }
}

/// The DECLARED type name of a subject expression, when it is resolvable from
/// symbol-stamped places alone: a state parameter, a data field reached
/// through a member access, or machine-owned data. Returns `None` for
/// inferred locals and computed subjects.
pub(crate) fn subject_declared_type_name<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    source: &resolved::expression::ExpressionTable,
    subject: resolved::expression::ExpressionHandle,
) -> Option<&'program str> {
    match source.expression(subject) {
        resolved::expression::ExpressionNode::Mutable(inner) => {
            subject_declared_type_name(program, source, *inner)
        }
        resolved::expression::ExpressionNode::Member(member) => {
            symbol_declared_type_name(program, member.member_symbol)
        }
        resolved::expression::ExpressionNode::Name(path) => {
            symbol_declared_type_name(program, path.symbol)
        }
        _ => None,
    }
}

fn symbol_declared_type_name(
    program: &resolved::SymbolResolvedTrees,
    symbol: SymbolHandle,
) -> Option<&str> {
    if !symbol.is_valid() {
        return None;
    }

    for definition in &program.data_definitions {
        for member in program.data_members(definition.members) {
            if let resolved::data::DataMember::Field(field) = member
                && field.symbol == symbol
            {
                return named_type_name(program, &field.type_reference);
            }
        }
    }

    for machine in &program.machines {
        for owned in program.machine_owned_data(machine.owned_data) {
            if owned.symbol == symbol {
                return named_type_name(program, &owned.type_reference);
            }
        }
        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            for parameter in program.state_parameters(state.parameters) {
                if parameter.symbol == symbol {
                    return named_type_name(program, &parameter.type_reference);
                }
            }
        }
    }

    // `self.field` resolves to the MACHINE-scope copy of the attached data's
    // field symbol, not the data definition's own field symbol: follow the
    // symbol's parent machine back to the attached data definition and match
    // the field by name.
    let symbol_entry = program.symbols.get(symbol);
    if symbol_entry.kind == omega_core::symbols::SymbolKind::Field {
        let parent = symbol_entry.parent;
        if !parent.is_valid() {
            return None;
        }
        let field_name = program.symbols.name(symbol).to_owned();
        let data_name = match program.symbols.get(parent).kind {
            omega_core::symbols::SymbolKind::Machine => program
                .machines
                .iter()
                .find(|machine| machine.symbol == parent)
                .and_then(|machine| machine.attached_data.as_ref())
                .map(|attached| attached.as_str().to_owned())?,
            omega_core::symbols::SymbolKind::Data => program.symbols.name(parent).to_owned(),
            _ => return None,
        };
        return data_field_type_name(program, &data_name, &field_name);
    }

    None
}

fn data_field_type_name<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    data_name: &str,
    field_name: &str,
) -> Option<&'program str> {
    let definition = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == data_name)?;
    program
        .data_members(definition.members)
        .iter()
        .find_map(|member| match member {
            resolved::data::DataMember::Field(field) if field.name.as_str() == field_name => {
                named_type_name(program, &field.type_reference)
            }
            _ => None,
        })
}

/// The plain NAMED type a reference bottoms out at, looking through `&`/`&mut`
/// and bracket constraints. `None` for slices, arrays, generics, traits.
fn named_type_name<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    type_reference: &'program resolved::types::TypeReference,
) -> Option<&'program str> {
    match type_reference {
        resolved::types::TypeReference::Named { name, .. } => Some(name.as_str()),
        resolved::types::TypeReference::Reference(reference) => named_type_name(
            program,
            program.child_type_reference(reference.storage.referee),
        ),
        resolved::types::TypeReference::Constrained(constrained) => named_type_name(
            program,
            program.child_type_reference(constrained.storage.base_type),
        ),
        _ => None,
    }
}

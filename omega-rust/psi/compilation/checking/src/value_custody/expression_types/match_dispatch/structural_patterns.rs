//! Structural patterns select declarations, not eligible source arrangements.
//! Subject evaluation and custody belong to the ordinary value/flow owners.
//! This module checks carrier/case correspondence, field drift and coverage;
//! recognizing a case never grants permission to read or move its payload.

use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::DataMember;
use typed_trees::expression::{
    BinaryOperator, ExpressionNode, MatchFieldAction, MatchPattern, TableBinaryExpression,
    TableMatchExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

pub fn structural_match_subject_type(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    dispatch: &TableMatchExpression,
) -> Option<TypeReferenceHandle> {
    let reference = super::declared_value_type(program, machine, state, dispatch.subject)?;
    crate::value_custody::places::data_definition_for_type(program, reference)?;
    Some(reference)
}

/// Zero denotes a record pattern; a nonzero handle denotes one exact case.
/// Absence means that the pattern does not belong to this subject's carrier.
pub fn structural_match_pattern_case(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    dispatch: &TableMatchExpression,
    pattern: MatchPattern,
) -> Option<SymbolHandle> {
    let reference = structural_match_subject_type(program, machine, state, dispatch)?;
    let data = crate::value_custody::places::data_definition_for_type(program, reference)?;
    let head = match pattern {
        MatchPattern::Value(value) => value,
        MatchPattern::Destructure(pattern) => pattern.head,
        MatchPattern::Wildcard => return Some(SymbolHandle::invalid()),
    };
    if let Some(name) = program.expression_table.name_path_node(head) {
        if name.symbol.is_valid() && program.symbols.get(name.symbol).kind == SymbolKind::Variant {
            return crate::applied_case_membership_variant(
                program,
                machine,
                Some(state),
                head,
                &TableBinaryExpression {
                    left: dispatch.subject,
                    operator: BinaryOperator::CaseMembership,
                    right: head,
                },
            );
        }
        // A record head names the subject's exact declaration. Case payloads
        // cannot be extracted under their sum owner without selecting a case.
        if matches!(pattern, MatchPattern::Destructure(_))
            && name.symbol == data.symbol
            && !program
                .data_members(data)
                .iter()
                .any(|member| matches!(member, DataMember::Variant(_)))
        {
            return Some(SymbolHandle::invalid());
        }
    }
    // Preserve the fieldless literal classifier accepted before payload
    // patterns, without imposing copyability or a source-location roster.
    if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(head)
        && literal.type_symbol == data.symbol
        && program
            .expression_table
            .struct_fields(literal.fields)
            .is_empty()
        && let Some(case) = literal.case_symbol
        && program.data_members(data).iter().any(|member| {
            member
                .as_variant()
                .is_some_and(|variant| variant.symbol == case)
        })
    {
        return Some(case);
    }
    None
}

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    dispatch: &TableMatchExpression,
    pattern: MatchPattern,
    source_span: source::SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(SymbolHandle, bool)> {
    let Some(case) = structural_match_pattern_case(program, machine, state, dispatch, pattern)
    else {
        diagnostics.push(
            Diagnostic::error(
                "match pattern does not select a record or case of its subject's exact carrier",
            )
            .with_source_span(source_span),
        );
        return None;
    };
    let MatchPattern::Destructure(pattern) = pattern else {
        return Some((case, true));
    };
    let reference = structural_match_subject_type(program, machine, state, dispatch)?;
    let data = crate::value_custody::places::data_definition_for_type(program, reference)?;
    let mut declared = program
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    if case.is_valid() {
        let variant = program
            .data_members(data)
            .iter()
            .filter_map(DataMember::as_variant)
            .find(|variant| variant.symbol == case)?;
        declared.extend(program.data_payload_fields(variant));
    }
    let fields = program.expression_table.match_fields(pattern.fields);
    let mut total = !pattern.guard.is_valid();
    for (ordinal, field) in fields.iter().enumerate() {
        let exact = declared.iter().find(|declared| {
            declared.symbol == field.field_symbol && declared.name.as_str() == field.member.as_str()
        });
        if exact.is_none()
            || fields[..ordinal]
                .iter()
                .any(|prior| prior.field_symbol == field.field_symbol)
        {
            diagnostics.push(
                Diagnostic::error(format!(
                    "match field `{}` is missing, duplicated, or belongs to another record/case",
                    field.member
                ))
                .with_source_span(source_span),
            );
        }
        if let MatchFieldAction::Equal(value) = field.action {
            total = false;
            if let Some(declared) = exact {
                let carrier = crate::value_custody::places::assignment_value_type(
                    program,
                    declared.type_reference,
                );
                if !super::super::argument_matches_carrier(
                    program,
                    machine.symbol,
                    value,
                    carrier,
                    &[],
                ) {
                    diagnostics.push(Diagnostic::error(format!("match equality operand for field `{}` is incompatible with the field's declared carrier", field.member)).with_source_span(source_span));
                }
                crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_range(
                    program,
                    carrier,
                    value,
                    "match field equality",
                    diagnostics,
                );
            }
        }
    }
    if !pattern.has_rest
        && declared.iter().any(|declared| {
            !fields
                .iter()
                .any(|field| field.field_symbol == declared.symbol)
        })
    {
        diagnostics.push(
            Diagnostic::error(
                "match structural pattern must mention every declared field or explicitly use `..`",
            )
            .with_source_span(source_span),
        );
    }
    if pattern.guard.is_valid() {
        let guard_type = super::declared_value_type(program, machine, state, pattern.guard)
            .and_then(|reference| {
                crate::value_custody::places::unwrapped_type_reference(program, reference)
            })
            .and_then(|reference| program.primitive_type_reference(reference));
        let boolean = guard_type == Some(typed_trees::types::PrimitiveType::Bool)
            || super::super::value_classification::value_class(
                program,
                Some(machine),
                Some(state),
                pattern.guard,
            ) == Some(super::super::ValueClass::Boolean);
        if !boolean {
            diagnostics.push(
                Diagnostic::error("match structural pattern guard must produce bool")
                    .with_source_span(source_span),
            );
        }
    }
    Some((case, total))
}

pub fn structural_match_cases_are_covered(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    cases: &[SymbolHandle],
) -> bool {
    let Some(data) = crate::value_custody::places::data_definition_for_type(program, reference)
    else {
        return false;
    };
    let mut variants = program
        .data_members(data)
        .iter()
        .filter_map(DataMember::as_variant)
        .peekable();
    if variants.peek().is_none() {
        return cases.contains(&SymbolHandle::invalid());
    }
    variants.all(|variant| cases.contains(&variant.symbol))
}

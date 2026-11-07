//! Structural patterns select declarations, not eligible source arrangements.
//! Subject evaluation and custody belong to the ordinary value/flow owners.
//! This module checks carrier/case correspondence, field drift and coverage;
//! recognizing a case never grants permission to read or move its payload.

use crate::frozen_program::expression_table;
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::DataMember;
use typed_trees::data::DataVariant;
use typed_trees::expression::{
    ExpressionNode, MatchFieldAction, MatchPattern, TableMatchExpression,
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
    structural_match_subject_type_for_subject(program, machine, state, dispatch.subject)
}

pub(crate) fn structural_match_subject_type_for_subject(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    subject: typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let reference = super::declared_value_type(program, machine, state, subject)?;
    crate::value_custody::places::data_definition_for_type(program, reference)?;
    Some(reference)
}

/// Zero denotes an untagged record or domain predicate; a nonzero handle denotes one exact case.
/// Absence means that the pattern does not belong to this subject's carrier.
pub fn structural_match_pattern_case(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    dispatch: &TableMatchExpression,
    pattern: MatchPattern,
) -> Option<SymbolHandle> {
    structural_match_pattern_case_for_subject(program, machine, state, dispatch.subject, pattern)
}

pub(crate) fn structural_match_pattern_case_for_subject(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    subject: typed_trees::expression::ExpressionHandle,
    pattern: MatchPattern,
) -> Option<SymbolHandle> {
    program.pattern_component(subject)?;
    let reference = structural_match_subject_type_for_subject(program, machine, state, subject)?;
    let data = crate::value_custody::places::data_definition_for_type(program, reference)?;
    let head = match pattern {
        MatchPattern::Value(value) => value,
        MatchPattern::Destructure(pattern) => pattern.head,
        MatchPattern::Wildcard => return Some(SymbolHandle::invalid()),
    };
    if let Some(name) = expression_table(program).name_path_node(head) {
        if name.symbol.is_valid() && program.symbols.get(name.symbol).kind == SymbolKind::Variant {
            // A case pattern selects a declaration, not a Boolean operation.
            // Authenticate the retained path and translate that selector only
            // through this receiver's complete semantic application.
            let owner = crate::exact_case_reference_owner(program, head)?;
            return program
                .data_constructor_case(
                    program
                        .type_reference_table
                        .peeled_type_reference(reference),
                    owner.symbol,
                    name.symbol,
                )
                .map(|variant| variant.symbol);
        }
        // A record head names the subject's exact declaration. Case payloads
        // cannot be extracted under their sum owner without selecting a case.
        if matches!(pattern, MatchPattern::Destructure(_))
            && name.symbol == data.symbol
            && program.pattern_component(subject) == Some(language_core::PatternComponent::Fields)
        {
            return Some(SymbolHandle::invalid());
        }
        // Domain patterns carry a normalized executable predicate and no
        // payload binding. They select this exact carrier without selecting a
        // case; successful predicate checking supplies the branch facts.
        if matches!(pattern, MatchPattern::Destructure(_))
            && let Some(domain) = program.domain_definition(name.symbol)
            && let Some(subject) =
                crate::value_custody::places::unwrapped_type_reference(program, reference)
            && let Some(target) =
                crate::value_custody::places::unwrapped_type_reference(program, domain.target_type)
            && crate::value_custody::type_references::type_references_match(
                program, subject, target,
            )
        {
            return Some(SymbolHandle::invalid());
        }
    }
    // Preserve the fieldless literal classifier accepted before payload
    // patterns, without imposing copyability or a source-location roster.
    if let ExpressionNode::StructLiteral(literal) = expression_table(program).expression(head)
        && expression_table(program)
            .struct_fields(literal.fields)
            .is_empty()
        && let Some(case) = literal.case_symbol
        && let Some(selected) = program.data_constructor_case(
            program
                .type_reference_table
                .peeled_type_reference(reference),
            literal.type_symbol,
            case,
        )
        && selected.payload.is_empty()
    {
        return Some(selected.symbol);
    }

    None
}

/// Only an unfiltered record/case pattern covers its complete carrier. A
/// domain's predicate, like a field equality or explicit guard, may fail.
/// Every coverage consumer uses this judgment rather than interpreting an
/// empty field list as proof that a domain accepts the whole record.
pub fn structural_match_pattern_is_total(
    program: &dyn crate::CheckingInput,
    pattern: MatchPattern,
) -> bool {
    match pattern {
        MatchPattern::Value(_) => true,
        MatchPattern::Wildcard => false,
        MatchPattern::Destructure(pattern) => {
            !pattern.guard.is_valid()
                && !expression_table(program)
                    .name_path_node(pattern.head)
                    .is_some_and(|head| program.domain_definition(head.symbol).is_some())
                && !expression_table(program)
                    .match_fields(pattern.fields)
                    .iter()
                    .any(|field| matches!(field.action, MatchFieldAction::Equal(_)))
        }
    }
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
    validate_for_subject(
        program,
        machine,
        state,
        dispatch.subject,
        pattern,
        source_span,
        diagnostics,
    )
}

/// The quotient data a pattern's subject carries, if any.
fn quotient_subject<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    subject: typed_trees::expression::ExpressionHandle,
) -> Option<&'program typed_trees::data::DataDefinition> {
    structural_match_subject_type_for_subject(program, machine, state, subject)
        .and_then(|reference| {
            crate::value_custody::places::data_definition_for_type(program, reference)
        })
        .filter(|data| data.quotient.is_some())
}

pub(crate) fn validate_for_subject(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    subject: typed_trees::expression::ExpressionHandle,
    pattern: MatchPattern,
    source_span: source::SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(SymbolHandle, bool)> {
    // A quotient's retained representatives are opaque (quotients.md: no
    // ordinary record destructuring or matching), so every arm but `_` would
    // observe one. Match expressions and transitions both validate arms here.
    if !matches!(pattern, MatchPattern::Wildcard)
        && let Some(data) = quotient_subject(program, machine, state, subject)
    {
        diagnostics.push(
            Diagnostic::error(format!(
                "destructure arm in machine `{}` state `{}` cannot pattern-match quotient `{}`: retained representatives are opaque and pattern matching requires a named lifted operation",
                machine.name, state.name, data.name,
            ))
            .with_source_span(source_span),
        );
        return None;
    }
    let Some(case) =
        structural_match_pattern_case_for_subject(program, machine, state, subject, pattern)
    else {
        let head = match pattern {
            MatchPattern::Value(head) => head,
            MatchPattern::Destructure(pattern) => pattern.head,
            MatchPattern::Wildcard => Default::default(),
        };
        let selected = expression_table(program).name_path_node(head).map(|path| {
            (
                expression_table(program)
                    .name_path_members(path.members)
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
                path.symbol,
            )
        });
        let actual = super::declared_value_type(program, machine, state, subject)
            .map(|reference| program.type_reference_table.type_reference(reference));
        diagnostics.push(
            Diagnostic::error(format!(
                "match pattern does not select a record or case of its subject's exact carrier in `{}::{}`: selected {selected:?}, subject {subject:?} {:?}, type {actual:?}; parameters {:?}; locals {:?}",
                machine.name, state.name,
                expression_table(program).expression(subject),
                program.state_parameters(state).iter().map(|parameter| (&parameter.name, parameter.symbol, parameter.type_reference)).collect::<Vec<_>>(),
                program.statement_table.statements(state.statement_nodes).iter().filter_map(|statement| statement.as_local_data()).map(|local| (&local.name, local.symbol, local.type_reference, local.initial_value)).collect::<Vec<_>>(),
            ))
            .with_source_span(source_span),
        );
        return None;
    };
    let MatchPattern::Destructure(pattern) = pattern else {
        return Some((case, true));
    };
    let reference = structural_match_subject_type_for_subject(program, machine, state, subject)?;
    if pattern.application_type.is_valid() {
        let matches = crate::value_custody::places::unwrapped_type_reference(program, reference)
            .zip(crate::value_custody::places::unwrapped_type_reference(
                program,
                pattern.application_type,
            ))
            .is_some_and(|(subject, application)| {
                crate::value_custody::type_references::type_references_match(
                    program,
                    subject,
                    application,
                )
            });
        if !matches {
            diagnostics.push(
                Diagnostic::error(
                    "pattern type application differs from its subject's exact carrier",
                )
                .with_source_span(source_span),
            );
            return None;
        }
    }
    let data = crate::value_custody::places::data_definition_for_type(program, reference)?;
    let declared = if case.is_valid() {
        let variant = program
            .data_members(data)
            .iter()
            .filter_map(DataMember::as_variant)
            .find(|variant| variant.symbol == case)?;
        program
            .data_payload_fields(variant)
            .iter()
            .collect::<Vec<_>>()
    } else {
        program
            .data_members(data)
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let has_cases = program
        .data_members(data)
        .iter()
        .any(|member| member.as_variant().is_some());
    let fields = expression_table(program).match_fields(pattern.fields);
    let total = structural_match_pattern_is_total(program, MatchPattern::Destructure(pattern));
    let selected_field_symbol = |field_symbol: SymbolHandle| {
        // An exact prepared selector already belongs to this declaration.
        // Translate a template selector only through the complete receiver;
        // representation applications retain their semantic constructor view.
        if declared
            .iter()
            .any(|declared| declared.symbol == field_symbol)
        {
            return Some(field_symbol);
        }
        program
            .data_constructor_field(
                program
                    .type_reference_table
                    .peeled_type_reference(reference),
                data.symbol,
                field_symbol,
            )
            .map(|selected| selected.symbol)
    };
    for (ordinal, field) in fields.iter().enumerate() {
        if field.component == language_core::PatternComponent::Case {
            if case.is_valid()
                || !has_cases
                || program.pattern_component(subject)
                    != Some(language_core::PatternComponent::Fields)
                || fields[..ordinal]
                    .iter()
                    .any(|prior| prior.component == language_core::PatternComponent::Case)
                || matches!(field.action, MatchFieldAction::Equal(_))
            {
                diagnostics.push(Diagnostic::error("case component is missing, duplicated, or selected outside its outer data pattern").with_source_span(source_span));
            }
            continue;
        }
        let selected = selected_field_symbol(field.field_symbol);
        let exact = selected.and_then(|selected| {
            declared.iter().find(|declared| {
                declared.symbol == selected && declared.name.as_str() == field.member.as_str()
            })
        });
        if exact.is_none()
            || fields[..ordinal]
                .iter()
                .filter(|prior| prior.component == language_core::PatternComponent::Fields)
                .any(|prior| {
                    selected_field_symbol(prior.field_symbol)
                        .zip(selected)
                        .is_some_and(|(prior, selected)| prior == selected)
                })
        {
            diagnostics.push(
                Diagnostic::error(format!(
                    "match field `{}` is missing, duplicated, or belongs to another record/case",
                    field.member
                ))
                .with_source_span(source_span),
            );
        }
        if let MatchFieldAction::Equal(value) = field.action
            && let Some(declared) = exact
        {
            let carrier = crate::value_custody::places::assignment_value_type(
                program,
                declared.type_reference,
            );
            if !super::super::argument_matches_carrier(program, machine.symbol, value, carrier, &[])
            {
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
    if !pattern.has_rest
        && (declared.iter().any(|declared| {
            !fields.iter().any(|field| {
                field.component == language_core::PatternComponent::Fields
                    && selected_field_symbol(field.field_symbol) == Some(declared.symbol)
            })
        }) || (!case.is_valid()
            && has_cases
            && !fields
                .iter()
                .any(|field| field.component == language_core::PatternComponent::Case)))
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
    expression: typed_trees::expression::ExpressionHandle,
    cases: &[SymbolHandle],
) -> bool {
    let Some(data) = crate::value_custody::places::data_definition_for_type(program, reference)
    else {
        return false;
    };
    if cases.contains(&SymbolHandle::invalid()) {
        return true;
    }
    let mut variants = program
        .data_members(data)
        .iter()
        .filter_map(DataMember::as_variant)
        .peekable();
    if variants.peek().is_none() {
        return cases.contains(&SymbolHandle::invalid());
    }
    variants
        .filter(|variant| {
            !variant_is_impossible(program, variant)
                && program
                    .admitted_match_cases(expression)
                    .is_none_or(|cases| cases.contains(&variant.symbol))
        })
        .all(|variant| cases.contains(&variant.symbol))
}

/// A variant carrying a literal-false `where` fact -- the `0`/`false` witness
/// generic synthesis leaves when an instance's `T == name` refutes, or an
/// authored `where false` -- can never be established, so it is not an
/// obligation the match must cover. Facts needing evaluation are not
/// contradictions and stay counted.
fn variant_is_impossible(program: &dyn crate::CheckingInput, variant: &DataVariant) -> bool {
    program
        .proof_facts
        .span_or_empty(variant.where_facts)
        .iter()
        .any(|fact| match fact.as_expression() {
            Some(expression) => match expression_table(program).expression(*expression) {
                ExpressionNode::Integer(literal) => literal.decimal_value_i128() == Some(0),
                ExpressionNode::Boolean(value) => !*value,
                _ => false,
            },
            None => false,
        })
}

/// Transition patterns share exact carrier/field validation with expression
/// matches. The already completed dispatch local remains the runtime subject.
pub(crate) fn validate_transition_patterns(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    transition: &typed_trees::statement::TableTransition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for retained in program
        .statement_table
        .transition_patterns(transition.patterns)
    {
        let is_case = expression_table(program)
            .name_path_node(retained.pattern.head)
            .is_some_and(|head| {
                head.symbol.is_valid()
                    && program.symbols.get(head.symbol).kind == SymbolKind::Variant
            });
        // A quotient has no cases, so a case-spelled arm on one does not even
        // resolve; it still observes the representative and must be refused.
        // Condition arms keep no head and never observe the representative.
        if retained.is_structural
            || is_case
            || (retained.pattern.head.is_valid()
                && quotient_subject(program, machine, state, retained.subject).is_some())
        {
            let pattern = if retained.is_structural {
                MatchPattern::Destructure(retained.pattern)
            } else {
                MatchPattern::Value(retained.pattern.head)
            };
            validate_for_subject(
                program,
                machine,
                state,
                retained.subject,
                pattern,
                transition.source_span,
                diagnostics,
            );
        }
    }
}

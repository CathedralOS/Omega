mod signatures;

use crate::symbols::TopLevelSymbols;
use crate::type_references::{
    TypeReferenceOwner, validate_type_reference_handle_with_type_parameters,
};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use signatures::{operator_name, operator_operand_key, operator_signature_key};

pub(super) fn validate_operator_declarations(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_duplicate_operator_signatures(program, "root", program.operators(), diagnostics);
    validate_spelling_overlap(program, program.operators(), diagnostics);
    for operator in program.operators() {
        validate_operator_types(program, symbols, operator, diagnostics);
    }

    for domain in program.domain_definitions() {
        validate_duplicate_operator_signatures(
            program,
            domain.name.as_str(),
            program.domain_operators(domain),
            diagnostics,
        );
        validate_spelling_overlap(program, program.domain_operators(domain), diagnostics);
        for operator in program.domain_operators(domain) {
            validate_operator_types(program, symbols, operator, diagnostics);
        }
    }
}

fn validate_operator_types(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    operator: &omega_typed_trees::operator::OperatorDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = operator_name(program, operator);
    let type_parameters = program.operator_type_parameters(operator);
    for parameter in program.operator_parameters(operator) {
        validate_type_reference_handle_with_type_parameters(
            program,
            parameter.type_reference,
            symbols,
            diagnostics,
            TypeReferenceOwner::OperatorParameter {
                operator: &name,
                parameter: parameter.name.as_str(),
                generic_depth: 0,
            },
            type_parameters,
            &operator.lifetime_parameters,
        );
    }
    if operator.return_type.is_valid() {
        validate_type_reference_handle_with_type_parameters(
            program,
            operator.return_type,
            symbols,
            diagnostics,
            TypeReferenceOwner::OperatorReturn {
                operator: &name,
                generic_depth: 0,
            },
            type_parameters,
            &operator.lifetime_parameters,
        );
    }
}

/// Frozen Wave 0 decision #3 ambiguity rule: the same spelling with overlapping
/// receiver/operand types is a compile error. Spelling is the first-level
/// resolution discriminator; within a spelling, the operand types must uniquely
/// pick a candidate, so two candidates sharing operand types are ambiguous.
fn validate_spelling_overlap(
    program: &TypedTrees,
    operators: &[omega_typed_trees::operator::OperatorDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (operator_index, operator) in operators.iter().enumerate() {
        let Some(spelling) = operator.spelling else {
            continue;
        };
        let operand_key = operator_operand_key(program, operator);

        let overlaps = operators[..operator_index].iter().any(|previous| {
            previous.spelling == Some(spelling)
                && operator_operand_key(program, previous) == operand_key
        });

        if overlaps {
            diagnostics.push(Diagnostic::error(format!(
                "operator spelling `{}` has overlapping operand types `({operand_key})`; \
                 the same spelling with overlapping receiver/operand types is ambiguous",
                spelling.symbol()
            )));
        }
    }
}

fn validate_duplicate_operator_signatures(
    program: &TypedTrees,
    owner: &str,
    operators: &[omega_typed_trees::operator::OperatorDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (operator_index, operator) in operators.iter().enumerate() {
        let signature = operator_signature_key(program, operator);
        let previous = operators[..operator_index]
            .iter()
            .filter(|previous| operator_signature_key(program, previous) == signature)
            .collect::<Vec<_>>();
        if previous.is_empty() {
            continue;
        }
        // Explicitly named requirements may differ by their normalized
        // dispatch-bearing result set. Fixed spellings never do: their
        // separate ambiguity rule remains operand-directed.
        if operator.is_boundary
            && operator.spelling.is_none()
            && previous
                .iter()
                .all(|item| item.is_boundary && item.spelling.is_none())
        {
            let identity = program.normalized_operator_overload_identity(operator);
            if previous.iter().any(|previous| {
                program
                    .normalized_operator_overload_identity(previous)
                    .result_dispatch()
                    == identity.result_dispatch()
            }) {
                let dispatch = if identity.result_dispatch().is_empty() {
                    "<empty>".to_owned()
                } else {
                    identity.result_dispatch().identity()
                };
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate named requirement overload `{}` with parameter signature `{}` and result dispatch set `{dispatch}`; predicate-only result refinements do not distinguish overloads",
                    identity.path(),
                    identity.parameters(),
                )));
            }
            continue;
        }
        let name = operator_name(program, operator);
        if owner == "root" {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate operator declaration `{name}`"
            )));
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "domain `{owner}` has duplicate operator `{name}`"
            )));
        }
    }
}

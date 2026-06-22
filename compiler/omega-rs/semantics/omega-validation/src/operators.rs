mod dispatch;
mod signatures;

use dispatch::validate_domain_operator_dispatch;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use signatures::{operator_name, operator_operand_key, operator_signature_key};

pub(super) fn validate_operator_declarations(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_duplicate_operator_signatures(program, "root", program.operators(), diagnostics);
    validate_spelling_overlap(program, program.operators(), diagnostics);

    for domain in program.domain_definitions() {
        validate_duplicate_operator_signatures(
            program,
            domain.name.as_str(),
            program.domain_operators(domain),
            diagnostics,
        );
        validate_spelling_overlap(program, program.domain_operators(domain), diagnostics);
    }

    // Chapter 8: competing domain-operator meanings for one spelling are a
    // compile error. This crosses domain boundaries (the per-domain
    // `validate_spelling_overlap` above only catches overlaps within a single
    // domain), so it runs once over the whole program.
    validate_domain_operator_dispatch(program, diagnostics);
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
        if operators[..operator_index]
            .iter()
            .any(|previous| operator_signature_key(program, previous) == signature)
        {
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
}

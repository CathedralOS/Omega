//! Operator declaration admission and shared immutable use queries.
//! Receiver typing and selection preserve the authored executable context;
//! typed-call mutation and checked proof/application extraction stay in the pipeline.

mod applications;
mod contracts;
pub(crate) mod namespaces;
mod receiver;
mod requirement_uses;
mod selection;
mod signatures;
mod uses;

pub use contracts::{
    build_operator_facts, derive_checked_operator_crash_contracts,
    derive_pre_flow_operator_selections,
};
pub use receiver::expression_type_reference_for_origin;
pub use requirement_uses::{collect_named_requirement_uses, token_bound_machine_call_target};
pub use selection::select_pending_domain_operator_meanings;
pub use uses::{
    build_operator_uses, checked_operator_candidate,
    collect_pre_specialization_operator_selections, indexed_operand_types,
};

pub(crate) use applications::retain_validated_boundary_operator_application;
pub(crate) use applications::validate_named_statement_operator_application;
pub use applications::{
    ValidatedBoundaryOperatorApplication, ValidatedBoundaryOperatorApplicationArgument,
    ValidatedBoundaryOperatorApplicationUseSite, canonical_closed_operator_realization_bytes,
    checked_operator_application_matches_realization, landed_integer_literal_type_reference,
    validate_closed_operator_application, validate_named_operator_application,
    validated_boundary_operator_application, validated_symbolic_boundary_operator_application,
};

use crate::declarations::symbols::TopLevelSymbols;
use crate::value_custody::type_references::{TypeReferenceOwner, validate_type_reference_handle};
use diagnostics::Diagnostic;
use signatures::{operator_name, operator_operand_key, operator_signature_key};

/// The spelled comparisons a bounds or contract read may treat as one
/// class. Arithmetic and bitwise operators carry no ordering judgment.
pub fn comparison_operator_spelling(
    operator: typed_trees::expression::BinaryOperator,
) -> Option<language_core::OperatorSpelling> {
    use typed_trees::expression::BinaryOperator;
    match operator {
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            typed_trees::operator::binary_operator_spelling(operator)
        }
        _ => None,
    }
}

pub(crate) fn validate_operator_declarations(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    operator: &typed_trees::operator::OperatorDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = operator_name(program, operator);
    for contract in program.operator_contracts(operator) {
        let typed_trees::signature::SignatureContractKind::Crashes { cause } = &contract.kind
        else {
            continue;
        };
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if !matches!(fact, typed_trees::domain::ProofFact::Expression(_)) {
                diagnostics.push(Diagnostic::error(format!(
                    "operator `{name}` `crashes {cause:?}` routes must be Boolean expressions; domain memberships and proposition applications are proof facts, not runtime-refinable crash routes",
                )));
            }
        }
    }
    let type_parameters = program.operator_type_parameters(operator);
    for parameter in program.operator_parameters(operator) {
        validate_type_reference_handle(
            program,
            parameter.type_reference,
            symbols,
            diagnostics,
            TypeReferenceOwner::OperatorParameter {
                operator: &name,
                parameter: parameter.name.as_str(),
                generic_depth: 0,
            },
            crate::value_custody::type_references::TypeParameterScope {
                operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                lexical_owner: operator.symbol,
                type_parameters,
                lifetime_parameters: &operator.lifetime_parameters,
            },
        );
    }
    if operator.return_type.is_valid() {
        validate_type_reference_handle(
            program,
            operator.return_type,
            symbols,
            diagnostics,
            TypeReferenceOwner::OperatorReturn {
                operator: &name,
                generic_depth: 0,
            },
            crate::value_custody::type_references::TypeParameterScope {
                operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                lexical_owner: operator.symbol,
                type_parameters,
                lifetime_parameters: &operator.lifetime_parameters,
            },
        );
    }
}

/// Settled ambiguity rule: the same fixed token with overlapping
/// receiver/operand types is a compile error. The token is the first-level
/// resolution discriminator; within it, operand types must uniquely select a
/// candidate. The parser accepts only the settled literal token in the
/// declaration head; the retired trailing `spelling` clause is rejected.
fn validate_spelling_overlap(
    program: &dyn crate::CheckingInput,
    operators: &[typed_trees::operator::OperatorDefinition],
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
                && !program
                    .symbols
                    .source_scopes_separate(previous.symbol, operator.symbol)
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
    program: &dyn crate::CheckingInput,
    owner: &str,
    operators: &[typed_trees::operator::OperatorDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (operator_index, operator) in operators.iter().enumerate() {
        let signature = operator_signature_key(program, operator);
        let previous = operators[..operator_index]
            .iter()
            .filter(|previous| {
                operator_signature_key(program, previous) == signature
                    && !program
                        .symbols
                        .source_scopes_separate(previous.symbol, operator.symbol)
            })
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
            let identity = program.operator_callable_identity(operator);
            let result_dispatch = program.callable_identity_result_dispatch(identity);
            if previous.iter().any(|previous| {
                program
                    .callable_identity_result_dispatch(program.operator_callable_identity(previous))
                    == result_dispatch
            }) {
                let dispatch = if program.callable_result_dispatch_is_empty(result_dispatch) {
                    "<empty>".to_owned()
                } else {
                    program.callable_result_dispatch_text(result_dispatch)
                };
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate named requirement overload `{}` with parameter signature `{}` and result dispatch set `{dispatch}`; predicate-only result refinements do not distinguish overloads",
                    program.callable_path_text(program.callable_identity_path(identity)),
                    program.callable_parameters_text(program.callable_identity_parameters(identity)),
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

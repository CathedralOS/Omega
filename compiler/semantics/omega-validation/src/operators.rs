use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;

pub(super) fn validate_operator_declarations(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_duplicate_operator_signatures(program, "root", program.operators(), diagnostics);

    for domain in program.domain_definitions() {
        validate_duplicate_operator_signatures(
            program,
            domain.name.as_str(),
            program.domain_operators(domain),
            diagnostics,
        );
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

fn operator_signature_key(
    program: &TypedTrees,
    operator: &omega_typed_trees::operator::OperatorDefinition,
) -> String {
    let parameter_types = program
        .operator_parameters(operator)
        .iter()
        .map(|parameter| program.display_type_reference(parameter.type_reference))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}({parameter_types})", operator_name(program, operator))
}

fn operator_name(
    program: &TypedTrees,
    operator: &omega_typed_trees::operator::OperatorDefinition,
) -> String {
    program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

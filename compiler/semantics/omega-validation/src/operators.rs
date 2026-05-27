use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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
    let type_parameter_indices = program
        .operator_type_parameters(operator)
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.symbol, index))
        .collect::<Vec<_>>();
    let parameter_types = program
        .operator_parameters(operator)
        .iter()
        .map(|parameter| {
            canonical_type_reference(program, parameter.type_reference, &type_parameter_indices)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}({parameter_types})", operator_name(program, operator))
}

fn canonical_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    type_parameter_indices: &[(SymbolHandle, usize)],
) -> String {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
        } => {
            let qualifier = if *is_mutable { "mut " } else { "" };
            format!(
                "&{qualifier}{}",
                canonical_type_reference(program, *referee, type_parameter_indices)
            )
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            format!(
                "{}[constraints]",
                canonical_type_reference(program, *base_type, type_parameter_indices)
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            format!(
                "[{}; {length}]",
                canonical_type_reference(program, *element_type, type_parameter_indices)
            )
        }
        TypeReferenceNode::Slice { element_type } => {
            format!(
                "[{}]",
                canonical_type_reference(program, *element_type, type_parameter_indices)
            )
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
        } => {
            let base =
                canonical_named_type(*base_symbol, base_name.as_str(), type_parameter_indices);
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| {
                    canonical_type_reference(program, *argument, type_parameter_indices)
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{base}<{arguments}>")
        }
        TypeReferenceNode::Named { symbol, name } => {
            canonical_named_type(*symbol, name.as_str(), type_parameter_indices)
        }
        TypeReferenceNode::Unit => "()".to_owned(),
    }
}

fn canonical_named_type(
    symbol: SymbolHandle,
    name: &str,
    type_parameter_indices: &[(SymbolHandle, usize)],
) -> String {
    type_parameter_indices
        .iter()
        .find_map(|(candidate, index)| (*candidate == symbol).then_some(*index))
        .map(|index| format!("${index}"))
        .unwrap_or_else(|| name.to_owned())
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

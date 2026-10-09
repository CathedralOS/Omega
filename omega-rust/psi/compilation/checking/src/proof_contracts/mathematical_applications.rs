//! Source custody for mathematical applications, shared by signature
//! elaboration, specification formation and fact substitution. A mathematical
//! definition owns a checked dependent telescope and transparent term; it is
//! never an executable machine, a runtime result occurrence or a purity claim.

mod expressions;
pub use expressions::{definition_roots, expression_nodes};

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::mathematical::{MathematicalDefinition, MathematicalType};
use typed_trees::types::TypeReferenceHandle;

pub fn selected_definition<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<&'program MathematicalDefinition> {
    call.target_symbol.is_valid().then_some(())?;
    program
        .mathematical_definitions()
        .iter()
        .find(|definition| definition.symbol == call.target_symbol)
}

pub fn parameter_type_reference(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    symbol.is_valid().then_some(())?;
    program
        .mathematical_definitions()
        .iter()
        .find_map(|definition| {
            let parameter = program
                .mathematical_parameters(definition.parameters)
                .iter()
                .find(|parameter| parameter.symbol == symbol)?;
            match program.mathematical_type(parameter.ty) {
                MathematicalType::Ordinary(reference) => Some(*reference),
                _ => None,
            }
        })
}

/// Ordinary leaf type custody for the calculus elaborator's free source
/// operands. Computation uses the same selected-result walk as other source
/// declarations; a destination or spelling supplies no type.
pub fn operand_type_reference(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let owner = crate::reserved_result_place(program, expression)
        .map(|place| place.machine_symbol)
        .or_else(|| match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) if path.symbol.is_valid() => {
                Some(program.symbols.get(path.symbol).parent)
            }
            _ => None,
        })
        .unwrap_or_else(|| program.symbols.root());
    crate::value_custody::expression_types::declaration_expression_result_type_reference(
        program,
        owner,
        &|expression| match program.expression_table.expression(expression) {
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                crate::proof_contracts::proof_embeddings::expression_type_reference(
                    program, expression,
                )
            }
            _ => None,
        },
        expression,
    )
}

/// Complete application result references are reusable by scalar consumers.
/// Partial applications remain mathematical function terms, not guessed
/// ordinary value types; their dependent formation is checked by the calculus.
pub fn result_type_reference(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<TypeReferenceHandle> {
    let definition = selected_definition(program, call)?;
    (call.arguments.len() == definition.parameters.len()).then_some(())?;
    match program.mathematical_type(definition.result) {
        MathematicalType::Ordinary(reference) => Some(*reference),
        _ => None,
    }
}

/// Source carrier obligations supplement calculus conversion: the bounded
/// scalar calculus embeds all fixed integers into Int, but an authored u64
/// formal does not thereby accept an i32 value or erase its policy. Dependent
/// and function arguments stay under the signature's full Pi application rule.
pub fn validate_source_application(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Result<(), &'static str> {
    let definition =
        selected_definition(program, call).ok_or("missing mathematical declaration")?;
    if call.receiver.is_valid()
        || call.static_machine_parameter.is_valid()
        || !call.selects_only_nominal_route()
        || !call.evidence_arguments.is_empty()
        || call.operational_acknowledgement.acknowledges_suspend
        || call.operational_acknowledgement.acknowledges_block
    {
        return Err(
            "a mathematical application cannot carry runtime receiver, effects or evidence payload",
        );
    }
    let mut domains = program
        .mathematical_parameters(definition.parameters)
        .iter()
        .map(|parameter| parameter.ty)
        .collect::<Vec<_>>();
    let mut result = definition.result;
    let mut visited = Vec::new();
    while result.is_valid() && !visited.contains(&result) {
        visited.push(result);
        let MathematicalType::Arrow {
            domain, codomain, ..
        } = program.mathematical_type(result)
        else {
            break;
        };
        domains.push(*domain);
        result = *codomain;
    }
    for (domain, argument) in domains
        .iter()
        .zip(program.expression_table.expression_handles(call.arguments))
    {
        let MathematicalType::Ordinary(reference) = program.mathematical_type(*domain) else {
            continue;
        };
        let Some(required) = program.primitive_type_reference(*reference) else {
            continue;
        };
        let corresponds = if let Some(actual) = operand_type_reference(program, *argument) {
            program.primitive_type_reference(actual) == Some(required)
                && program.type_reference_table.arithmetic_domain(actual)
                    == program.type_reference_table.arithmetic_domain(*reference)
        } else {
            // Unlanded literals form against the demanded carrier. A missing
            // typed value is not an anonymous literal or a width/policy guess.
            crate::value_custody::literals::has_anonymous_numeric_results(program, *argument)
                && crate::value_custody::expression_types::argument_matches_type_reference_handle(
                    program, *argument, *reference,
                )
        };
        if !corresponds {
            return Err("mathematical argument does not preserve its declared carrier and policy");
        }
    }
    Ok(())
}

/// Adapt the ordinary portion of a mathematical telescope for shared source
/// checking. These are source binder views, never generated machine states.
/// Dependent functions/types retain their full MathematicalType at the kernel.
pub fn ordinary_parameters(
    program: &dyn crate::CheckingInput,
    definition: &MathematicalDefinition,
) -> Vec<typed_trees::signature::StateParameter> {
    program
        .mathematical_parameters(definition.parameters)
        .iter()
        .filter_map(|parameter| {
            let MathematicalType::Ordinary(reference) = program.mathematical_type(parameter.ty)
            else {
                return None;
            };
            Some(typed_trees::signature::StateParameter {
                symbol: parameter.symbol,
                name: parameter.name.clone(),
                type_reference: *reference,
                ..Default::default()
            })
        })
        .collect()
}

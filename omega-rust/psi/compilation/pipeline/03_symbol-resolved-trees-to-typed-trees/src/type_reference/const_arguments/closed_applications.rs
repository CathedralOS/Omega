//! Close argument values independently of whether their declaration was typed
//! in this cohort or retained by an earlier package. The declaration's exact
//! telescope supplies the carrier; shared const admission checks formation and
//! complete value constraints before any source recipe is replaced.

use diagnostics::Diagnostic;
use language_semantics::const_value::DecodedCanonicalConstValue;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceNode;

pub(super) fn normalize(program: &mut TypedTrees, frontier: usize) -> Result<(), Diagnostic> {
    let applications = program
        .type_reference_table
        .type_reference_rows()
        .filter_map(|(reference, node)| {
            if reference.arena_index() as usize <= frontier {
                return None;
            }
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = node
            else {
                return None;
            };
            Some((reference, *base_symbol, *arguments))
        })
        .collect::<Vec<_>>();
    for (application, declaration, arguments) in applications {
        let parameters = if let Some(data) = program.unique_data_definition_by_symbol(declaration) {
            program.data_type_parameters(data).to_vec()
        } else if let Some(interface) = program.trait_definition_by_symbol(declaration) {
            program.trait_type_parameters(interface).to_vec()
        } else {
            continue;
        };
        let arguments = program
            .type_reference_table
            .type_reference_handles(arguments)
            .to_vec();
        if parameters.len() != arguments.len() {
            continue;
        }
        let substitutions = parameters
            .iter()
            .zip(&arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument))
            .collect::<Vec<_>>();
        let owner = program
            .type_reference_table
            .operand_lifetime_owner(application);
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if argument.arena_index() as usize <= frontier {
                continue;
            }
            let TypeParameterKind::Const { type_reference } = parameter.kind else {
                continue;
            };
            let TypeReferenceNode::ConstExpression(expression) =
                *program.type_reference_table.type_reference(argument)
            else {
                continue;
            };
            if program.pending_const_range_endpoints.contains(&expression) {
                continue;
            }
            let carrier = program
                .apply_callable_type(type_reference, &substitutions, &[], declaration)
                .map_err(Diagnostic::error)?;
            let value =
                validation::check_closed_const_expression(program, owner, expression, carrier)
                    .map_err(|errors| {
                        errors.into_iter().next().unwrap_or_else(|| {
                            Diagnostic::error(
                                "closed const argument admission failed without a diagnostic",
                            )
                        })
                    })?;
            let Some(value) = value else {
                continue;
            };
            let spelling = match value.decode_encoding() {
                Some(DecodedCanonicalConstValue::Integer { value, .. }) => value.to_string(),
                Some(DecodedCanonicalConstValue::Boolean(value)) => value.to_string(),
                Some(_) => value.atom(),
                None => {
                    return Err(Diagnostic::error(
                        "closed const argument has malformed canonical encoding",
                    ));
                }
            };
            program.type_reference_table.substitute_node(
                argument,
                TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::generated(spelling),
                },
            );
        }
    }
    Ok(())
}

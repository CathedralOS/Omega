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
        } else if program.symbols.get(declaration).kind == symbols::SymbolKind::Conformance {
            let mut declarations = program
                .conformances()
                .iter()
                .filter(|definition| definition.symbol == declaration);
            let Some(definition) = declarations.next() else {
                continue;
            };
            if declarations.next().is_some() {
                continue;
            }
            program.conformance_type_parameters(definition).to_vec()
        } else if program.symbols.get(declaration).kind == symbols::SymbolKind::TypeAlias {
            let mut aliases = program
                .type_aliases()
                .iter()
                .filter(|alias| alias.symbol == declaration);
            let Some(alias) = aliases.next() else {
                continue;
            };
            if aliases.next().is_some() {
                continue;
            }
            program
                .tables
                .data_type_parameters
                .span_or_empty(alias.type_parameters)
                .to_vec()
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
            // Each supplied recipe retains its own caller telescope. An
            // ordinary Generic has no lexical owner; its tuple supplier is
            // only the fallback for an argument without a more specific one.
            let argument_owner = program
                .type_reference_table
                .operand_lifetime_owner(argument);
            let owner = if argument_owner.is_valid() {
                argument_owner
            } else {
                owner
            };
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

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::{
        arithmetic::ArithmeticDomain,
        literals::{IntegerLanding, IntegerLiteral, LandedIntegerType},
    };
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{
        constant::ConstDeclaration,
        data::TypeParameter,
        expression::{BinaryOperator, Expression, ExpressionNode, NamePath, TableBinaryExpression},
        trait_definition::Conformance,
        type_aliases::TypeAliasDefinition,
    };

    #[test]
    fn closed_const_arguments_use_unique_headers_and_keep_the_supplier() {
        for (kind, header_count, argument_scope) in [
            (SymbolKind::TypeAlias, 0, false),
            (SymbolKind::TypeAlias, 1, false),
            (SymbolKind::TypeAlias, 2, false),
            (SymbolKind::TypeAlias, 1, true),
            (SymbolKind::Conformance, 0, true),
            (SymbolKind::Conformance, 1, true),
            (SymbolKind::Conformance, 2, true),
        ] {
            let mut builder = SymbolTableBuilder::default();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let children = builder.insert_children(
                root,
                symbols::builtin_type_symbols().into_iter().chain([
                    (kind, SymbolNameRef::Static("Alias")),
                    (SymbolKind::Machine, SymbolNameRef::Static("supplier")),
                    (SymbolKind::Const, SymbolNameRef::Static("ZERO")),
                ]),
            );
            let u64_symbol = children.handle_at(symbols::BuiltinTypeAtom::U64.ordinal() as u32);
            let alias = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
            let supplier = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
            let zero = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 2);
            let parameter = builder
                .insert_children(
                    alias,
                    [(SymbolKind::TypeParameter, SymbolNameRef::Static("N"))],
                )
                .start();
            let mut program = TypedTrees::default();
            program.symbols = builder.finish();
            let carrier = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: u64_symbol,
                    name: Identifier::generated("u64"),
                });
            let parameters = program.data_type_parameters.insert_many([TypeParameter {
                symbol: parameter,
                name: Identifier::generated("N"),
                kind: TypeParameterKind::Const {
                    type_reference: carrier,
                },
                ..Default::default()
            }]);
            for _ in 0..header_count {
                if kind == SymbolKind::Conformance {
                    program.push_conformance(Conformance {
                        symbol: alias,
                        type_parameters: parameters,
                        ..Default::default()
                    });
                } else {
                    program.push_type_alias_definition(TypeAliasDefinition {
                        symbol: alias,
                        type_parameters: parameters,
                        ..Default::default()
                    });
                }
            }
            let value = program.expression_table.insert(ExpressionNode::Integer(
                IntegerLiteral::from_value(1).with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::U64,
                    domain: ArithmeticDomain::Exact,
                }),
            ));
            let value =
                if argument_scope {
                    program.push_const_declaration(ConstDeclaration {
                        symbol: zero,
                        declared_type: carrier,
                        evaluated_value: Some(
                            language_core::constant_value::EvaluatedConstant::Encoded(
                                language_semantics::const_value::CanonicalConstValue::integer(
                                    "u64", 0,
                                )
                                .encoding,
                            ),
                        ),
                        ..Default::default()
                    });
                    let left = program.expression_table.insert_tree(&Expression::Name(
                        NamePath::resolved(vec![Identifier::generated("ZERO")], zero, zero),
                    ));
                    program
                        .expression_table
                        .insert(ExpressionNode::Binary(TableBinaryExpression {
                            left,
                            operator: BinaryOperator::Add,
                            right: value,
                        }))
                } else {
                    value
                };
            let argument = program
                .type_reference_table
                .insert(TypeReferenceNode::ConstExpression(value));
            let arguments = program
                .type_reference_table
                .insert_type_reference_handles([argument]);
            let application = program
                .type_reference_table
                .insert(TypeReferenceNode::Generic {
                    base_symbol: alias,
                    base_name: Identifier::generated("Alias"),
                    arguments,
                    lifetime_arguments: Vec::new(),
                });
            program
                .type_reference_table
                .retain_operand_lifetime_owner(
                    if argument_scope {
                        argument
                    } else {
                        application
                    },
                    supplier,
                )
                .unwrap();
            normalize(&mut program, 0).unwrap();
            if header_count == 1 {
                assert!(
                    matches!(program.type_reference_table.type_reference(argument), TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() && name.as_str() == "1")
                );
            } else {
                assert_eq!(
                    program.type_reference_table.type_reference(argument),
                    &TypeReferenceNode::ConstExpression(value)
                );
            }
            assert_eq!(
                program
                    .type_reference_table
                    .operand_lifetime_owner(if argument_scope {
                        argument
                    } else {
                        application
                    }),
                supplier
            );
            if header_count == 1 {
                let wrong_width = program.expression_table.insert(ExpressionNode::Integer(
                    IntegerLiteral::from_value(1).with_landing(IntegerLanding {
                        landed_type: LandedIntegerType::U8,
                        domain: ArithmeticDomain::Exact,
                    }),
                ));
                program
                    .type_reference_table
                    .substitute_node(argument, TypeReferenceNode::ConstExpression(wrong_width));
                assert!(normalize(&mut program, 0).is_err());
                assert_eq!(
                    program.type_reference_table.type_reference(argument),
                    &TypeReferenceNode::ConstExpression(wrong_width)
                );
            }
        }
    }
}

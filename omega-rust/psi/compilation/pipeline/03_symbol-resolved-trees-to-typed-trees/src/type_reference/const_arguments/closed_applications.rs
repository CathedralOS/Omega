//! Check constant arguments against their declaration's parameter types and
//! conditions before replacing a source computation. The same checks apply to
//! new declarations and those retained from an earlier package.

use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceNode;

enum ConstArgumentInput {
    Expression(typed_trees::expression::ExpressionHandle),
    Canonical(CanonicalConstValue),
}

pub(super) fn normalize(program: &mut TypedTrees, frontier: usize) -> Result<(), Diagnostic> {
    let mut applications = program
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
            Some((
                *base_symbol,
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .to_vec(),
                program
                    .type_reference_table
                    .operand_lifetime_owner(reference),
            ))
        })
        .collect::<Vec<_>>();
    // Applications declared on a type or machine need the same argument checks
    // as generic type nodes. Storing a computation in a declaration header
    // does not turn it into a completed constant.
    applications.extend(program.conformances().iter().map(|declaration| {
        (
            declaration.trait_symbol,
            program
                .type_reference_table
                .type_reference_handles(declaration.arguments)
                .to_vec(),
            declaration.symbol,
        )
    }));
    for machine in program.machines() {
        applications.extend(
            program
                .machine_trait_conformances(machine)
                .iter()
                .map(|edge| {
                    (
                        edge.symbol,
                        program
                            .type_reference_table
                            .type_reference_handles(edge.arguments)
                            .to_vec(),
                        machine.symbol,
                    )
                }),
        );
    }
    for declaration in program.traits() {
        applications.extend(
            program
                .trait_requirements(declaration)
                .iter()
                .map(|parent| {
                    (
                        parent.symbol,
                        program
                            .type_reference_table
                            .type_reference_handles(parent.arguments)
                            .to_vec(),
                        declaration.symbol,
                    )
                }),
        );
        if let Some(base) = &declaration.refines {
            applications.push((
                base.symbol,
                program
                    .type_reference_table
                    .type_reference_handles(base.arguments)
                    .to_vec(),
                declaration.symbol,
            ));
        }
    }
    let bounds = program
        .machines()
        .iter()
        .map(|item| (item.symbol, &item.conformance_bounds))
        .chain(
            program
                .traits()
                .iter()
                .map(|item| (item.symbol, &item.conformance_bounds)),
        )
        .chain(
            program
                .data_definitions()
                .iter()
                .map(|item| (item.symbol, &item.conformance_bounds)),
        )
        .chain(
            program
                .type_aliases()
                .iter()
                .map(|item| (item.symbol, &item.conformance_bounds)),
        );
    for (owner, bounds) in bounds {
        applications.extend(bounds.iter().map(|bound| {
            // Lifted bounds still name the declaration where their operands
            // were authored, not the nested machine retaining the same bound.
            let owner = if bound.binder.is_valid() {
                program.symbols.get(bound.binder).parent
            } else {
                owner
            };
            (bound.trait_symbol, bound.arguments.clone(), owner)
        }));
    }
    for (declaration, arguments, owner) in applications {
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
        if parameters.len() != arguments.len() {
            continue;
        }
        let substitutions = parameters
            .iter()
            .zip(&arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument))
            .collect::<Vec<_>>();
        for (parameter, argument) in parameters.iter().zip(arguments.iter().copied()) {
            if argument.arena_index() as usize <= frontier {
                continue;
            }
            // Fresh declaration arguments need their source scope even when
            // a receiving callback contract substitutes them later. Existing
            // copied supplier custody takes precedence over this header owner.
            if owner.is_valid()
                && !program
                    .type_reference_table
                    .operand_lifetime_owner(argument)
                    .is_valid()
            {
                program
                    .type_reference_table
                    .retain_operand_lifetime_owner(argument, owner)
                    .map_err(Diagnostic::error)?;
            }
            let TypeParameterKind::Const { type_reference } = parameter.kind else {
                continue;
            };
            let input = match program.type_reference_table.type_reference(argument) {
                TypeReferenceNode::ConstExpression(expression) => {
                    if program.pending_const_range_endpoints.contains(expression) {
                        continue;
                    }
                    // Caller parameters need that caller's bounds and local
                    // facts during checking. Keep the expression until then;
                    // it is not a completed constant to publish here.
                    if !typed_trees::domain::index_argument_is_closed(program, argument) {
                        continue;
                    }
                    ConstArgumentInput::Expression(*expression)
                }
                TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => {
                    let Some(value) = CanonicalConstValue::from_atom(name.as_str()) else {
                        continue;
                    };
                    if !matches!(
                        value.decode_encoding(),
                        Some(
                            DecodedCanonicalConstValue::Record { .. }
                                | DecodedCanonicalConstValue::Variant { .. }
                        )
                    ) {
                        continue;
                    }
                    ConstArgumentInput::Canonical(value)
                }
                _ => continue,
            };
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
            let value = match input {
                ConstArgumentInput::Expression(expression) => {
                    validation::check_closed_const_expression(program, owner, expression, carrier)
                        .map_err(|errors| {
                        errors.into_iter().next().unwrap_or_else(|| {
                            Diagnostic::error(
                                "closed const argument admission failed without a diagnostic",
                            )
                        })
                    })?
                }
                ConstArgumentInput::Canonical(mut value) => {
                    // Source atoms retain schema labels until their exact receiving
                    // telescope is available. Validate their existing carrier claim
                    // before publishing the same resolved identity as a completed
                    // expression. Payload equality alone cannot retarget a nominal
                    // value, and diagnostic display never determines an application.
                    validation::validate_closed_const_argument(
                        program,
                        program.symbols.name(declaration),
                        parameter,
                        argument,
                        &parameters,
                        &arguments,
                    )
                    .map_err(|errors| {
                        errors.into_iter().next().unwrap_or_else(|| {
                            Diagnostic::error(
                                "canonical const argument admission failed without a diagnostic",
                            )
                        })
                    })?;
                    value.type_name = program.normalized_type_identity_text(carrier);
                    Some(value)
                }
            };
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
            let mut program = TypedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            let carrier = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: u64_symbol,
                    name: Identifier::generated("u64"),
                });
            // Lowering a suffixed literal retains its landed builtin carrier
            // (`retain_builtin_type_reference`); the `u8` control below needs
            // that same row so its selected carrier is known, not open.
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: children.handle_at(symbols::BuiltinTypeAtom::U8.ordinal() as u32),
                    name: Identifier::generated("u8"),
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
                    // Every evaluated declaration retains its evidence root;
                    // closed queries follow it rather than the encoded value.
                    let materialized_initializer =
                        program.expression_table.insert(ExpressionNode::Integer(
                            IntegerLiteral::from_value(0).with_landing(IntegerLanding {
                                landed_type: LandedIntegerType::U64,
                                domain: ArithmeticDomain::Exact,
                            }),
                        ));
                    program.push_const_declaration(ConstDeclaration {
                        symbol: zero,
                        declared_type: carrier,
                        materialized_initializer,
                        evaluated_value: Some(
                            language_core::constant_value::EvaluatedConstant::Encoded(
                                language_semantics::const_value::CanonicalConstIdentity::integer(
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

//! Public declaration value encodings and validated initializers.

use crate::constant::initializer_normalization;
use crate::constant::requires_const_initializer_evaluation;
use crate::constant::substitution::semantic_const_name;
use diagnostics::Diagnostic;
use source::{SourceSpan, Span};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionHandle;
use symbols::SymbolKind;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ConstDefinition, DataMember, Item};

/// Even unused private constants owe their exact nominal destination. Check
/// resolved constructor and field identities after specialization, without
/// requiring a public/index encoding or duplicating scalar type checking.
pub(super) fn validate_nominal_destinations(
    program: &SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbol_resolved_trees::types::TypeReference;

    let mut pending = program
        .const_declarations
        .iter()
        .map(|declaration| (&declaration.declared_type, declaration.initializer))
        .collect::<Vec<_>>();
    while let Some((destination, expression)) = pending.pop() {
        let table = &program.tables.bodies.expressions;
        if let TypeReference::Constrained(constraint) = destination {
            pending.push((
                program.child_type_reference(constraint.base_type),
                expression,
            ));
            continue;
        }
        if let (TypeReference::FixedArray(array), ExpressionNode::ArrayLiteral(elements)) =
            (destination, table.expression(expression))
        {
            pending.extend(
                table
                    .expression_handles(*elements)
                    .iter()
                    .map(|element| (program.child_type_reference(array.element_type), *element)),
            );
            continue;
        }
        let expected = match destination {
            TypeReference::Named { symbol, .. } => *symbol,
            TypeReference::Generic(application) if application.arguments.is_empty() => {
                application.base_symbol
            }
            _ => continue,
        };
        let (actual, literal) = match table.expression(expression) {
            ExpressionNode::StructLiteral(literal) => (literal.type_symbol, Some(literal)),
            ExpressionNode::Name(path)
                if program.symbols.get(path.symbol).kind == SymbolKind::Variant =>
            {
                (program.symbols.get(path.symbol).parent, None)
            }
            _ => continue,
        };
        if !expected.is_valid() || !actual.is_valid() || expected != actual {
            return Err(Diagnostic::error(format!(
                "constant constructor selects a different nominal carrier than its declared destination `{}`",
                program.symbols.display_path(expected, "::"),
            )).with_source_span(table.source_span(expression)));
        }
        let Some(literal) = literal else {
            continue;
        };
        let Some(definition) = program
            .data_definitions
            .iter()
            .find(|definition| definition.symbol == expected)
        else {
            continue;
        };
        for field in table.struct_fields(literal.fields) {
            let declared = program
                .data_members(definition.members)
                .iter()
                .find_map(|member| match member {
                    DataMember::Field(declared) if declared.symbol == field.field_symbol => {
                        Some(declared)
                    }
                    DataMember::Variant(variant) if literal.case_symbol == Some(variant.symbol) => {
                        program
                            .data_payload_fields(variant.payload)
                            .iter()
                            .find(|declared| declared.symbol == field.field_symbol)
                    }
                    _ => None,
                });
            if let Some(declared) = declared {
                pending.push((&declared.type_reference, field.value));
            }
        }
    }
    Ok(())
}

/// Public declaration identity includes floating scalars with determined bits, independently
/// of the narrower structural values eligible for generic and domain indices.
pub(crate) fn public_declaration_value_encoding(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&crate::preparation::generic_data::constant_selection::ConstantSelection>,
) -> Result<String, String> {
    use numerics::literals::FloatLiteral;
    use syntax_trees::{expression::ExpressionNode, types::TypeReferenceNode};

    if let TypeReferenceNode::Named(carrier) = syntax
        .type_references
        .type_reference(definition.type_reference)
        && matches!(carrier.as_str(), "f32" | "f64")
    {
        validate_scalar_initializer(syntax, definition)?;
        let literal = match syntax.expressions.expression(definition.value) {
            ExpressionNode::Float(text) => FloatLiteral::parse(text.as_str()),
            ExpressionNode::Integer(integer) => integer
                .value_bignum()
                .and_then(|value| FloatLiteral::parse(&value.to_string())),
            _ => None,
        }
        .ok_or("floating declaration identity requires a scalar literal")?;
        // Read the declared format directly from exact source meaning. Going
        // through f64 for f32 would introduce a second rounding at midpoints.
        // Signed infinity has one exact encoding per format. Payloadless NaN
        // meaning cannot choose representation bits for a public declaration.
        return if carrier.as_str() == "f32" {
            let value = literal.value_f32();
            if value.is_nan() {
                return Err(
                    "floating declaration identity requires explicit NaN representation bits"
                        .to_owned(),
                );
            }
            Ok(format!("float:f32:{:08x}", value.to_bits()))
        } else {
            let value = literal.value_f64();
            if value.is_nan() {
                return Err(
                    "floating declaration identity requires explicit NaN representation bits"
                        .to_owned(),
                );
            }
            Ok(format!("float:f64:{:016x}", value.to_bits()))
        };
    }
    crate::preparation::generic_data::canonicalize_selected_declared_const_definition(
        syntax, definition, selection,
    )
    .map(|value| value.encoding)
}

pub(crate) fn validate_scalar_initializer(
    syntax: &SyntaxTrees,
    constant: &syntax_trees::item::ConstDefinition,
) -> Result<(), String> {
    use numerics::literals::{FloatFormat, FloatLiteral};
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::types::TypeReferenceNode;

    // Unused private declarations never reach substitution or public identity
    // checks. Their numeric and Boolean landing obligations still apply.
    // Floating values need no const-index encoding to check their carrier.
    let TypeReferenceNode::Named(carrier) = syntax
        .type_references
        .type_reference(constant.type_reference)
    else {
        return Ok(());
    };
    if !matches!(
        carrier.as_str(),
        "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "addr"
            | "f32"
            | "f64"
    ) {
        return Ok(());
    }
    let initializer = syntax.expressions.expression(constant.value);
    match carrier.as_str() {
        "f32" | "f64" => {
            let format = if carrier.as_str() == "f32" {
                FloatFormat::F32
            } else {
                FloatFormat::F64
            };
            let compatible = match initializer {
                ExpressionNode::Float(text) => {
                    FloatLiteral::parse(text.as_str()).is_some_and(|literal| {
                        literal.landing().is_none_or(|landing| landing == format)
                    })
                }
                ExpressionNode::Integer(literal) => {
                    literal.landing().is_none() && literal.value_bignum().is_some()
                }
                _ => false,
            };
            if compatible {
                Ok(())
            } else {
                Err(format!(
                    "initializer conflicts with declared floating carrier `{carrier}`"
                ))
            }
        }
        _ => crate::preparation::generic_data::canonicalize_declared_const_definition(
            syntax, constant,
        )
        .map(|_| ()),
    }
}

/// Declaration validity precedes all use-site substitution, including unused
/// private declarations. Namespace and case collisions are checked later against
/// exact symbols; lexical locals do not make a constant declaration ambiguous.
pub(crate) fn validate_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&crate::preparation::generic_data::constant_selection::ConstantSelection>,
) -> Result<(), Diagnostic> {
    // An unused private declaration never acquires a use-site proof obligation.
    // Its constrained destination is still mandatory; failure to encode a
    // public/index value must not swallow a false declaration-site predicate.
    if let syntax_trees::types::TypeReferenceNode::Constrained { constraints, .. } = syntax
        .type_references
        .type_reference(definition.type_reference)
        && syntax
            .type_references
            .constraints(*constraints)
            .iter()
            .any(|constraint| {
                matches!(
                    constraint,
                    syntax_trees::types::TypeConstraintNode::Domain(_)
                )
            })
    {
        crate::preparation::generic_data::canonicalize_selected_declared_const_definition(
            syntax, definition, selection,
        )
        .map_err(|reason| {
            Diagnostic::error(format!(
                "constrained const `{}` is invalid: {reason}",
                semantic_const_name(definition)
            ))
            .with_source_span(definition.name.source_span())
        })?;
    }
    validate_scalar_initializer(syntax, definition).map_err(|reason| {
        Diagnostic::error(format!(
            "scalar constant `{}` is invalid: {reason}",
            definition.name.as_str()
        ))
        .with_source_span(definition.name.source_span())
    })?;
    validate_literal_initializer(syntax, definition, definition.value)
}

pub(crate) fn retain_const_initializer(
    lowerer: &mut crate::resolution::lowerer::Lowerer,
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<ExpressionHandle, Diagnostic> {
    let pending = lowerer.const_resolution_mode
        == crate::resolution::lowerer::ConstResolutionMode::InitializerSelection
        && requires_const_initializer_evaluation(syntax, definition);
    if !pending && !has_scalar_initializer(syntax, definition) {
        if crate::preparation::module_normalization::module_literal_constant(syntax, definition) {
            // Unused private arrays still owe declaration shape and landing.
            crate::preparation::generic_data::canonicalize_declared_const_definition(
                syntax, definition,
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "array constant `{}` is invalid: {reason}",
                    semantic_const_name(definition)
                ))
                .with_source_span(definition.name.source_span())
            })?;
        } else {
            validate_literal_initializer(syntax, definition, definition.value)?;
        }
    }
    let initializer = crate::lowering::expression::lower_expression_into_table(
        lowerer,
        syntax,
        definition.value,
    )?;
    lowerer.pending_const_values.push(initializer);
    initializer_normalization::retain(lowerer, syntax, definition, initializer)?;
    Ok(initializer)
}

fn has_scalar_initializer(syntax: &SyntaxTrees, definition: &ConstDefinition) -> bool {
    use syntax_trees::expression::ExpressionNode;
    matches!(
        syntax.expressions.expression(definition.value),
        ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_)
    )
}

/// Select a constant through the ordinary source-scoped path before its value
/// is substituted. Initializer dependency discovery uses the same selection:
/// module body names need not yet carry their finalized constant ledger rows.
pub(crate) fn selected_expression_constant(
    program: &SymbolResolvedTrees,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) -> Option<(SourceSpan, symbols::SymbolHandle)> {
    use symbol_resolved_trees::expression::ExpressionNode;
    let table = &program.tables.bodies.expressions;
    let ExpressionNode::Name(path) = table.expression(expression) else {
        return None;
    };
    if path.is_self_value
        || matches!(
            program.symbols.get(path.head_symbol).kind,
            SymbolKind::Local
                | SymbolKind::Parameter
                | SymbolKind::MachineParameter
                | SymbolKind::TypeParameter
                | SymbolKind::ConformanceParameter
        )
    {
        return None;
    }
    let members = table.name_path_members(path.members);
    let first = members.first()?;
    let last = members.last()?;
    let reference = SourceSpan::new(
        first.source_span().source_id,
        Span::new(first.source_span().span.start, last.source_span().span.end),
    );
    let name = members
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let selected = program
        .symbols
        .find_top_level_by_name_and_kinds_from_source(&name, &[SymbolKind::Const], reference)?;
    Some((reference, selected))
}

/// v0 initializers are literals all the way down. Payloadless case names are
/// nullary structural literals. The parser already folds `-5` into a single
/// literal, so no operator node is legitimate here.
fn validate_literal_initializer(
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
    value: syntax_trees::expression::ExpressionHandle,
) -> Result<(), Diagnostic> {
    use syntax_trees::expression::ExpressionNode;
    match syntax_trees.expressions.expression(value) {
        ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_) => Ok(()),
        ExpressionNode::ArrayLiteral(values) => {
            for element in syntax_trees.expressions.expression_handles(*values) {
                validate_literal_initializer(syntax_trees, definition, *element)?;
            }
            Ok(())
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in syntax_trees.expressions.struct_fields(literal.fields) {
                validate_literal_initializer(syntax_trees, definition, field.value)?;
            }
            Ok(())
        }
        ExpressionNode::Name(path) => {
            let members = syntax_trees.expressions.identifier_path_members(*path);
            let [type_name, case_name] = members else {
                return invalid_literal_initializer(syntax_trees, definition, value);
            };
            let is_payloadless_case = syntax_trees.root_items().any(|item| {
                let Item::Data(data) = item else {
                    return false;
                };
                data.name.as_str() == type_name.as_str()
                    && syntax_trees
                        .items
                        .data_members(data.members)
                        .iter()
                        .any(|member| {
                            matches!(
                                member,
                                DataMember::Variant(variant)
                                    if variant.name.as_str() == case_name.as_str()
                                        && variant.payload.is_empty()
                            )
                        })
            });
            if is_payloadless_case {
                Ok(())
            } else {
                invalid_literal_initializer(syntax_trees, definition, value)
            }
        }
        _ => invalid_literal_initializer(syntax_trees, definition, value),
    }
}

fn invalid_literal_initializer(
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
    value: syntax_trees::expression::ExpressionHandle,
) -> Result<(), Diagnostic> {
    Err(Diagnostic::error(format!(
        "const `{}::{}` initializer must be a literal (a scalar, a payloadless \
         case, or a struct/array literal of literals) in const-v0; `{}` is not \
         -- richer const expressions arrive with build-time evaluation",
        definition.scope.as_str(),
        definition.name.as_str(),
        syntax_trees
            .expressions
            .expression(value)
            .display_name(&syntax_trees.expressions),
    )))
}

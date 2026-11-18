/* Public declaration value encodings and validated initializers. */

use crate::constants::initializer_normalization;
use crate::constants::substitution::semantic_const_name;
use crate::declarations::generic_data::{
    canonicalize_selected_declared_const_definition, const_destination_requires_proof,
};
use crate::lowering::expression::lower_expression_into_table;
use crate::tables::syntax_expressions;
use crate::tables::syntax_items;
use crate::tables::syntax_type_references;
use diagnostics::Diagnostic;
use language_core::constant_value::EvaluatedConstant;
use numerics::literals::FloatFormat;
use source::{SourceSpan, Span};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionHandle;
use symbols::JoinedPath;
use symbols::SymbolKind;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::{ConstDefinition, DataMember};
use syntax_trees::types::TypeConstraintNode;
use syntax_trees::types::TypeReferenceNode;

/*
    Even unused private constants owe their exact nominal destination. Check
    resolved constructor and field identities after specialization, without
    requiring a public/index encoding or duplicating scalar type checking.
*/
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
        let Some(definition) = program.data_definition_by_symbol(expected) else {
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

/*
    Keep evaluation separate from byte demand. Only the explicit internal semantic
    node may carry a payloadless NaN; source float spelling still follows the ordinary
    encoder. The retained authored root is independently replayed after typing.
*/
pub(crate) fn evaluated_declaration_value(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&crate::declarations::declaration_selection::ConstantSelection>,
) -> Result<EvaluatedConstant, String> {
    use language_core::constant_value::EvaluatedConstant;
    // A literal payload does not establish the selected declaration's
    // predicates. Keep it pending until its ordinary initializer is
    // checked and admitted for evaluation.
    if const_destination_requires_proof(syntax, definition, selection) {
        return Err("constant requires its checked initializer computation".into());
    }
    if let Some(normalization) = &definition.normalization {
        return Ok(normalization.result_value.clone());
    }
    if let ExpressionNode::UndeterminedNaN(format) = syntax.expressions.expression(definition.value)
    {
        let mut carrier = definition.type_reference;
        while let TypeReferenceNode::Constrained { base_type, .. } =
            syntax.type_references.type_reference(carrier)
        {
            carrier = *base_type;
        }
        let Some(name) = syntax.type_references.type_reference(carrier).as_named() else {
            return Err(
                "semantic NaN constant requires its exact declared floating carrier".into(),
            );
        };
        let actual = match name.as_str(syntax) {
            "f32" => FloatFormat::F32,
            "f64" => FloatFormat::F64,
            _ => {
                return Err(
                    "semantic NaN constant requires its exact declared floating carrier".into(),
                );
            }
        };
        if actual != *format {
            return Err("semantic NaN constant differs from its declared floating format".into());
        }
        return Ok(EvaluatedConstant::UndeterminedNaN(*format));
    }
    public_declaration_value_encoding(syntax, definition, selection).map(EvaluatedConstant::Encoded)
}

/*
    Public declaration identity includes floating components with determined bits, independently
    of the narrower structural values eligible for generic and domain indices.
*/
pub(crate) fn public_declaration_value_encoding(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&crate::declarations::declaration_selection::ConstantSelection>,
) -> Result<String, String> {
    canonicalize_selected_declared_const_definition(syntax, definition, selection)
        .map(|value| value.encoding)
}

pub(crate) fn validate_scalar_initializer(
    syntax: &SyntaxTrees,
    constant: &syntax_trees::item::ConstDefinition,
    selection: Option<&crate::declarations::declaration_selection::ConstantSelection>,
) -> Result<(), String> {
    use numerics::literals::FloatFormat;
    use syntax_trees::expression::ExpressionNode;

    let Some(carrier) = syntax_type_references(syntax)
        .type_reference(constant.type_reference)
        .as_named()
    else {
        return Ok(());
    };
    let selection = selection.ok_or("scalar initializer has no declaration selection context")?;
    let Some(atom) = selection.builtin_type(syntax, carrier) else {
        return Ok(());
    };
    let initializer = syntax.expressions.expression(constant.value);
    match atom {
        symbols::BuiltinTypeAtom::F32 | symbols::BuiltinTypeAtom::F64 => {
            let format = if atom == symbols::BuiltinTypeAtom::F32 {
                FloatFormat::F32
            } else {
                FloatFormat::F64
            };
            let compatible = match initializer {
                ExpressionNode::UndeterminedNaN(actual) => *actual == format,
                ExpressionNode::Float(text) => text.literal(syntax).is_some_and(|literal| {
                    literal.landing().is_none_or(|landing| landing == format)
                }),
                ExpressionNode::Integer(literal) => {
                    literal.landing().is_none() && literal.value_bignum().is_some()
                }
                _ => false,
            };
            if compatible {
                Ok(())
            } else {
                Err(format!(
                    "initializer conflicts with declared floating carrier `{carrier}`",
                    carrier = carrier.as_str(syntax)
                ))
            }
        }
        symbols::BuiltinTypeAtom::Integer(_)
        | symbols::BuiltinTypeAtom::Address
        | symbols::BuiltinTypeAtom::Bool => {
            canonicalize_selected_declared_const_definition(syntax, constant, Some(selection))
                .map(|_| ())
        }
        _ => Ok(()),
    }
}

/*
    Declaration validity precedes all use-site substitution, including unused
    private declarations. Namespace and case collisions are checked later against
    exact symbols; lexical locals do not make a constant declaration ambiguous.
*/
pub(crate) fn validate_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&crate::declarations::declaration_selection::ConstantSelection>,
) -> Result<(), Diagnostic> {
    if definition.normalization.is_some()
        || crate::constants::requires_const_initializer_evaluation(syntax, definition)
        || const_destination_requires_proof(syntax, definition, selection)
    {
        // Resolution retains the request and its declaration identities.
        // Only an accepted invocation can supply a computed value.
        return Ok(());
    }
    if definition.pending_target_observation {
        let supported = syntax_type_references(syntax)
            .type_reference(definition.type_reference)
            .as_named()
            .and_then(|name| selection.and_then(|selected| selected.builtin_type(syntax, name)))
            .is_some_and(|atom| {
                matches!(
                    atom,
                    symbols::BuiltinTypeAtom::Integer(_)
                        | symbols::BuiltinTypeAtom::Bool
                        | symbols::BuiltinTypeAtom::F32
                        | symbols::BuiltinTypeAtom::F64
                )
            });
        return if supported {
            Ok(())
        } else {
            Err(Diagnostic::error("target-dependent constant evaluation requires an unqualified builtin scalar destination")
            .with_source_span(definition.name.source_span()))
        };
    }
    if let TypeReferenceNode::Constrained { constraints, .. } =
        syntax_type_references(syntax).type_reference(definition.type_reference)
        && syntax_type_references(syntax)
            .constraints(*constraints)
            .iter()
            .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)))
    {
        canonicalize_selected_declared_const_definition(syntax, definition, selection).map_err(
            |reason| {
                Diagnostic::error(format!(
                    "constrained const `{}` is invalid: {reason}",
                    semantic_const_name(syntax, definition)
                ))
                .with_source_span(definition.name.source_span())
            },
        )?;
    }
    validate_scalar_initializer(syntax, definition, selection).map_err(|reason| {
        Diagnostic::error(format!(
            "scalar constant `{}` is invalid: {reason}",
            definition.name.as_str(syntax)
        ))
        .with_source_span(definition.name.source_span())
    })?;
    validate_literal_initializer(syntax, definition, definition.value)
}

pub(crate) fn retain_const_initializer(
    lowerer: &mut crate::lowering::lowerer::Lowerer,
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<ExpressionHandle, Diagnostic> {
    let pending =
        lowerer.const_resolution_mode != crate::lowering::lowerer::ConstResolutionMode::Complete;
    if !pending
        && definition.normalization.is_none()
        && !has_scalar_initializer(syntax, definition)
        && !crate::constants::requires_const_initializer_evaluation(syntax, definition)
        && !const_destination_requires_proof(
            syntax,
            definition,
            lowerer.constant_selection.as_ref(),
        )
    {
        if crate::declarations::module_normalization::module_literal_constant(
            syntax,
            definition,
            lowerer.constant_selection.as_ref(),
        ) {
            canonicalize_selected_declared_const_definition(
                syntax,
                definition,
                lowerer.constant_selection.as_ref(),
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "array constant `{}` is invalid: {reason}",
                    semantic_const_name(syntax, definition)
                ))
                .with_source_span(definition.name.source_span())
            })?;
        } else {
            if !definition.pending_target_observation {
                validate_literal_initializer(syntax, definition, definition.value)?;
            }
        }
    }
    let initializer = lower_expression_into_table(lowerer, syntax, definition.value)?;
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
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::String(_)
    )
}

/*
    Select a constant through the ordinary source-scoped path before its value
    is substituted. Initializer dependency discovery uses the same selection:
    module body names need not yet carry their finalized constant ledger rows.
*/
pub(crate) fn selected_expression_constant(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> Option<(SourceSpan, symbols::SymbolHandle)> {
    let table = &program.tables.bodies.expressions;
    let path = table.name_path_node(expression)?;
    if path.static_application.is_valid() {
        return None;
    }
    if matches!(
        program.symbols.get(path.head_symbol).kind,
        SymbolKind::Local
            | SymbolKind::Parameter
            | SymbolKind::MachineParameter
            | SymbolKind::TypeParameter
            | SymbolKind::ConformanceParameter
            | SymbolKind::Field
    ) {
        return None;
    }
    let members = table.name_path_members(path.members);
    let first = members.first()?;
    let last = members.last()?;
    let reference = SourceSpan::new(
        first.source_span().source_id,
        Span::new(first.source_span().span.start, last.source_span().span.end),
    );
    let name = members.joined_path();
    let selected = program
        .symbols
        .find_top_level_by_name_and_kinds_from_source(&name, &[SymbolKind::Const], reference)?;
    Some((reference, selected))
}

/*
    v0 initializers are literals all the way down. Payloadless case names are
    nullary structural literals. The parser already folds `-5` into a single
    literal, so no operator node is legitimate here.
*/
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
        | ExpressionNode::UndeterminedNaN(_)
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
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => {
            let members = syntax_trees
                .expressions
                .identifier_path_members(path.to_owned());
            let [type_name, case_name] = members else {
                return invalid_literal_initializer(syntax_trees, definition, value);
            };
            let is_payloadless_case = syntax_trees.root_items().any(|item| {
            let Some(data) = item.as_data() else {
                return false;
            };
            data.name.as_str(syntax_trees) == type_name.as_str(syntax_trees)
                && syntax_items(syntax_trees).data_members(data.members)
                    .iter()
                    .any(|member| {
                        matches!(
                            member,
                            DataMember::Variant(variant)
                                if variant.name.as_str(syntax_trees) == case_name.as_str(syntax_trees)
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
        definition.scope.as_str(syntax_trees),
        definition.name.as_str(syntax_trees),
        syntax_expressions(syntax_trees)
            .expression(value)
            .display_name(syntax_trees),
    )))
}

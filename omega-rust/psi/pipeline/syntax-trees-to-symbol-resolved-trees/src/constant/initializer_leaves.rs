//! Pending const initializer leaves and aggregate placeholders.

use crate::constant::requires_const_initializer_evaluation;
use source::{SourceSpan, Span};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ConstDefinition, DataMember};

/// One pending initializer leaf: the authored expression and the exact
/// declared carrier it must produce. `structured` leaves are aggregate-producing
/// expressions (ordinary calls, matches, or evaluated constant references at a
/// closed nominal or fixed-array destination) whose value arrives through the
/// checked interpreter rather than the scalar probe evaluator.
#[derive(Clone, Copy)]
pub struct PendingConstInitializerLeaf {
    pub expression: syntax_trees::expression::ExpressionHandle,
    pub destination: syntax_trees::types::TypeReferenceHandle,
    pub structured: bool,
}

pub(crate) fn pending_const_initializer_leaves(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
) -> Result<Vec<PendingConstInitializerLeaf>, String> {
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::types::{FixedArrayLength, TypeReferenceNode};

    fn collect(
        syntax: &SyntaxTrees,
        selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
        expression: syntax_trees::expression::ExpressionHandle,
        type_reference: syntax_trees::types::TypeReferenceHandle,
        leaves: &mut Vec<PendingConstInitializerLeaf>,
    ) -> Result<(), String> {
        match syntax.type_references.type_reference(type_reference) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => match syntax.expressions.expression(expression) {
                ExpressionNode::ArrayLiteral(elements) => {
                    let values = syntax.expressions.expression_handles(*elements);
                    if values.len() != elements.len() || values.len() != *length {
                        return Err(
                            "computed array constant has an incorrect element roster".to_owned()
                        );
                    }
                    for element in values {
                        collect(syntax, selection, *element, *element_type, leaves)?;
                    }
                    Ok(())
                }
                // Aggregate-producing leaves keep their authored expression.
                // The leaf probe's checked interpreter supplies the result and
                // admission checks it against this exact closed array carrier.
                ExpressionNode::Call(_) | ExpressionNode::Match(_) => {
                    leaves.push(PendingConstInitializerLeaf {
                        expression,
                        destination: type_reference,
                        structured: true,
                    });
                    Ok(())
                }
                ExpressionNode::Name(path)
                    if name_selects_const(syntax, selection, *path) =>
                {
                    leaves.push(PendingConstInitializerLeaf {
                        expression,
                        destination: type_reference,
                        structured: true,
                    });
                    Ok(())
                }
                _ => Err("computed array constant requires an array literal".to_owned()),
            },
            TypeReferenceNode::Named(name)
                if matches!(
                    selection.builtin_type(name),
                    Some(symbols::BuiltinTypeAtom::I8 | symbols::BuiltinTypeAtom::I16
                        | symbols::BuiltinTypeAtom::I32 | symbols::BuiltinTypeAtom::I64
                        | symbols::BuiltinTypeAtom::U8 | symbols::BuiltinTypeAtom::U16
                        | symbols::BuiltinTypeAtom::U32 | symbols::BuiltinTypeAtom::U64
                        | symbols::BuiltinTypeAtom::Bool)
                ) =>
            {
                if !matches!(
                    syntax.expressions.expression(expression),
                    ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
                ) {
                    leaves.push(PendingConstInitializerLeaf {
                        expression,
                        destination: type_reference,
                        structured: false,
                    });
                }
                Ok(())
            }
            TypeReferenceNode::Named(name) => {
                let ExpressionNode::StructLiteral(literal) = syntax.expressions.expression(expression) else {
                    if selection.builtin_type(name).is_none() {
                        match syntax.expressions.expression(expression) {
                            // A call or match whose result is the selected
                            // nominal carrier evaluates through the checked
                            // interpreter as one structured leaf.
                            ExpressionNode::Call(_) | ExpressionNode::Match(_) => {
                                require_closed_data(selection, syntax, name)?;
                                leaves.push(PendingConstInitializerLeaf {
                                    expression,
                                    destination: type_reference,
                                    structured: true,
                                });
                                return Ok(());
                            }
                            // An evaluated constant reference at a nominal
                            // destination is the same structured leaf; a bare
                            // case or other name stays with literal validation.
                            ExpressionNode::Name(path)
                                if name_selects_const(syntax, selection, *path) =>
                            {
                                require_closed_data(selection, syntax, name)?;
                                leaves.push(PendingConstInitializerLeaf {
                                    expression,
                                    destination: type_reference,
                                    structured: true,
                                });
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                    // Existing literal validation owns payloadless cases and
                    // noncomputed scalar formats; they introduce no probe leaf.
                    return if matches!(syntax.expressions.expression(expression),
                        ExpressionNode::Name(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_) | ExpressionNode::String(_)) {
                        Ok(())
                    } else {
                        Err("computed constant leaf requires an exact builtin integer or Boolean carrier".to_owned())
                    };
                };
                let declared = require_closed_data(selection, syntax, name)?;
                let (constructed, case) = selection.constructor(syntax, &literal.constructor_name)?;
                if !std::ptr::eq(declared, constructed) {
                    return Err("computed constant constructor differs from its declared nominal carrier".to_owned());
                }
                let members = syntax.items.data_members(declared.members);
                if members.len() != declared.members.len() {
                    return Err("computed constant data member span is stale".to_owned());
                }
                let fields = if let Some(case) = case {
                    let variant = members.iter().find_map(|member| match member {
                        DataMember::Variant(variant) if variant.name.as_str() == case.as_str() => Some(variant),
                        _ => None,
                    }).ok_or("computed constant case is absent from its selected owner")?;
                    let fields = syntax.items.data_payload_fields(variant.payload);
                    if fields.len() != variant.payload.len() {
                        return Err("computed constant payload span is stale".to_owned());
                    }
                    fields.iter().collect::<Vec<_>>()
                } else {
                    if members.iter().any(|member| matches!(member, DataMember::Variant(_))) {
                        return Err("computed constant sum must select its exact case".to_owned());
                    }
                    members.iter().filter_map(|member| match member {
                        DataMember::Field(field) => Some(field),
                        _ => None,
                    }).collect()
                };
                let authored = syntax.expressions.struct_fields(literal.fields);
                if authored.len() != literal.fields.len() || authored.len() != fields.len() {
                    return Err("computed constant field roster differs from its declaration".to_owned());
                }
                // Keep authored evaluation order, but select field types only
                // within the exact constructor's retained declaration.
                for field in authored {
                    if authored.iter().filter(|candidate| candidate.name.as_str() == field.name.as_str()).count() != 1 {
                        return Err(format!("duplicate computed constant field `{}`", field.name));
                    }
                    let declaration = fields.iter().find(|candidate| candidate.name.as_str() == field.name.as_str())
                        .ok_or_else(|| format!("unknown computed constant field `{}`", field.name))?;
                    collect(syntax, selection, field.value, declaration.type_reference, leaves)?;
                }
                Ok(())
            }
            _ => Err("computed constant requires a closed array, nominal literal, or builtin scalar carrier".to_owned()),
        }
    }

    let mut leaves = Vec::new();
    if !requires_const_initializer_evaluation(syntax, definition) {
        return Ok(leaves);
    }
    collect(
        syntax,
        selection,
        definition.value,
        definition.type_reference,
        &mut leaves,
    )?;
    Ok(leaves)
}

/// The selected declaration for a computed nominal constant must be closed:
/// generic or lifetime-carrying owners cannot produce one materialized value.
fn require_closed_data<'syntax>(
    selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
    syntax: &'syntax SyntaxTrees,
    name: &syntax_trees::identifier::Identifier,
) -> Result<&'syntax syntax_trees::item::DataDefinition, String> {
    let declared = selection.data(syntax, name)?;
    if !declared.type_parameters.is_empty() || !declared.lifetime_parameters.is_empty() {
        return Err("computed nominal constant requires a closed selected declaration".to_owned());
    }
    Ok(declared)
}

/// Whether one authored name path selects an exact constant declaration from
/// its own source scope, distinguishing an evaluated value reference from a
/// literal case or constructor spelling.
fn name_selects_const(
    syntax: &SyntaxTrees,
    selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
    path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
) -> bool {
    let members = syntax.expressions.identifier_path_members(path);
    let (Some(first), Some(last)) = (members.first(), members.last()) else {
        return false;
    };
    let reference = SourceSpan::new(
        first.source_span().source_id,
        Span::new(first.source_span().span.start, last.source_span().span.end),
    );
    let name = members
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    selection.selects_const(&syntax_trees::identifier::Identifier::new(name, reference))
}

/// Synthesize one closed zero literal standing in for a pending aggregate leaf
/// so the surrounding probe forest can type. The placeholder exists only for
/// the non-executing preparation/typing pass; it is replaced by the evaluated
/// literal before the owning declaration materializes and never becomes a
/// published value.
pub(crate) fn pending_aggregate_placeholder(
    syntax: &mut SyntaxTrees,
    selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
    type_reference: syntax_trees::types::TypeReferenceHandle,
    reference: SourceSpan,
) -> Result<syntax_trees::expression::ExpressionHandle, String> {
    use syntax_trees::expression::{ExpressionNode, TableStructLiteral, TableStructLiteralField};
    use syntax_trees::types::{FixedArrayLength, TypeReferenceNode};

    // The node is cloned so recursive placeholder construction may insert
    // into the same syntax forest while this carrier stays borrowed.
    let destination = syntax
        .type_references
        .type_reference(type_reference)
        .clone();
    let node = match &destination {
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let element =
                pending_aggregate_placeholder(syntax, selection, *element_type, reference)?;
            let elements = syntax
                .expressions
                .insert_expression_handles(vec![element; *length]);
            ExpressionNode::ArrayLiteral(elements)
        }
        TypeReferenceNode::Named(name) => match selection.builtin_type(name) {
            Some(symbols::BuiltinTypeAtom::Bool) => ExpressionNode::Boolean(false),
            Some(
                symbols::BuiltinTypeAtom::I8
                | symbols::BuiltinTypeAtom::I16
                | symbols::BuiltinTypeAtom::I32
                | symbols::BuiltinTypeAtom::I64
                | symbols::BuiltinTypeAtom::U8
                | symbols::BuiltinTypeAtom::U16
                | symbols::BuiltinTypeAtom::U32
                | symbols::BuiltinTypeAtom::U64
                | symbols::BuiltinTypeAtom::Address,
            ) => ExpressionNode::Integer(numerics::literals::IntegerLiteral::zero()),
            Some(_) => {
                return Err(
                    "aggregate initializer placeholder has no closed literal carrier".to_owned(),
                );
            }
            None => {
                let (constructor, fields) = {
                    let declared = require_closed_data(selection, syntax, name)?;
                    let members = syntax.items.data_members(declared.members);
                    if members.len() != declared.members.len() {
                        return Err(
                            "aggregate initializer placeholder has a stale data member span"
                                .to_owned(),
                        );
                    }
                    if let Some(variant) = members.iter().find_map(|member| match member {
                        DataMember::Variant(variant) => Some(variant),
                        _ => None,
                    }) {
                        let payload = syntax.items.data_payload_fields(variant.payload);
                        if payload.len() != variant.payload.len() {
                            return Err(
                                "aggregate initializer placeholder has a stale payload span"
                                    .to_owned(),
                            );
                        }
                        (
                            format!("{}::{}", name.as_str(), variant.name.as_str()),
                            payload
                                .iter()
                                .map(|field| (field.name.as_str().to_owned(), field.type_reference))
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        (
                            name.as_str().to_owned(),
                            members
                                .iter()
                                .filter_map(|member| match member {
                                    DataMember::Field(field) => {
                                        Some((field.name.as_str().to_owned(), field.type_reference))
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>(),
                        )
                    }
                };
                let mut literal_fields = Vec::with_capacity(fields.len());
                for (field_name, field_type) in fields {
                    let value =
                        pending_aggregate_placeholder(syntax, selection, field_type, reference)?;
                    literal_fields.push(TableStructLiteralField {
                        // Compiler-derived placeholder fields carry no authored
                        // selection token: distinct declared fields must not
                        // alias one source span as copies of a single authored
                        // field selection.
                        name: syntax_trees::identifier::Identifier::generated(field_name),
                        value,
                    });
                }
                let fields = syntax.expressions.insert_struct_fields(literal_fields);
                ExpressionNode::StructLiteral(TableStructLiteral {
                    constructor_name: syntax_trees::identifier::Identifier::new(
                        constructor,
                        name.source_span(),
                    ),
                    fields,
                })
            }
        },
        TypeReferenceNode::Constrained { base_type, .. } => {
            return pending_aggregate_placeholder(syntax, selection, *base_type, reference);
        }
        _ => {
            return Err(
                "aggregate initializer placeholder requires a closed nominal or array carrier"
                    .to_owned(),
            );
        }
    };
    let placeholder = syntax.expressions.insert(node);
    syntax.expressions.set_source_span(placeholder, reference);
    Ok(placeholder)
}

//! Discover authored initializer leaves without demanding unfinished geometry.
//!
//! Discovery precedes dependency scheduling. Array cardinality is checked by
//! completed value encoding, after the exact declared extent has evaluated;
//! neither the initializer roster nor a probe stand-in supplies that extent.

use crate::constant::requires_const_initializer_evaluation;
use crate::preparation::declaration_selection::{
    GenericApplicationSubstitution, has_deferred_const_argument, substituted_generic_arguments,
    type_mentions_parameters,
};
use source::{SourceSpan, Span};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::item::{ConstDefinition, DataDefinition, DataMember};
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// One pending initializer leaf: the authored expression and the exact
/// declared carrier it must produce. `structured` leaves are aggregate-producing
/// expressions (ordinary calls, matches, or evaluated constant references at a
/// closed nominal or fixed-array destination) whose value arrives through the
/// checked interpreter rather than the scalar probe evaluator.
#[derive(Clone, Copy)]
pub struct PendingConstInitializerLeaf {
    pub expression: syntax_trees::expression::ExpressionHandle,
    pub destination: TypeReferenceHandle,
    pub structured: bool,
}

/// Whether `type_reference` is a carrier a leaf probe machine can spell: every
/// part already closed, no enclosing parameter left in any argument or member
/// position, and no deferred const-expression argument whose layout stand-in
/// would drift the materialized constructor's instance name.
fn closed_leaf_carrier(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    substitution: &GenericApplicationSubstitution,
) -> bool {
    !type_mentions_parameters(syntax, type_reference, substitution)
        && !has_deferred_const_argument(syntax, type_reference)
}

/// The member carrier a closed application's bindings assign: a bare `T`
/// member type reselects the argument's existing handle. Composite member
/// types keep their authored handle — `Generic` and `FixedArray` arms resolve
/// their own arguments and const lengths against the same bindings — while
/// any other shape still spelling a parameter declines.
fn effective_member_type(
    syntax: &SyntaxTrees,
    member_type: TypeReferenceHandle,
    substitution: &GenericApplicationSubstitution,
) -> Result<TypeReferenceHandle, String> {
    match syntax.type_references.type_reference(member_type) {
        TypeReferenceNode::Named(name) if substitution.contains_key(name.as_str(syntax)) => {
            Ok(substitution[name.as_str(syntax)])
        }
        TypeReferenceNode::Generic { .. } | TypeReferenceNode::FixedArray { .. } => Ok(member_type),
        _ if type_mentions_parameters(syntax, member_type, substitution) => {
            Err("computed constant member type is not yet a closed structural type".to_owned())
        }
        _ => Ok(member_type),
    }
}

pub(crate) fn pending_const_initializer_leaves(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: &crate::preparation::declaration_selection::ConstantSelection,
) -> Result<Vec<PendingConstInitializerLeaf>, String> {
    use syntax_trees::expression::ExpressionNode;

    fn collect(
        syntax: &SyntaxTrees,
        selection: &crate::preparation::declaration_selection::ConstantSelection,
        expression: syntax_trees::expression::ExpressionHandle,
        type_reference: TypeReferenceHandle,
        substitution: &GenericApplicationSubstitution,
        leaves: &mut Vec<PendingConstInitializerLeaf>,
    ) -> Result<(), String> {
        // A carrier spelled as a bare parameter reselects the enclosing
        // application's already-closed argument handle.
        if let Some(name) = syntax
            .type_references
            .type_reference(type_reference)
            .as_named()
            && let Some(argument) = substitution.get(name.as_str(syntax))
        {
            return collect(
                syntax,
                selection,
                expression,
                *argument,
                substitution,
                leaves,
            );
        }
        // A rewritten `Named` instance spelling carries its authored
        // application as origin evidence; resolve through it so the
        // synthesized name never has to re-resolve as a header symbol.
        let origin = syntax
            .type_references
            .generic_application_origin(type_reference);
        if origin.is_valid() && origin != type_reference {
            return collect(syntax, selection, expression, origin, substitution, leaves);
        }
        match syntax.type_references.type_reference(type_reference) {
            TypeReferenceNode::DynamicTrait { .. }
                if crate::constant::direct_selector_literal(syntax, expression) => Ok(()),
            TypeReferenceNode::FixedArray { element_type, .. } => match syntax.expressions.expression(expression) {
                ExpressionNode::ArrayLiteral(elements) => {
                    let element_type =
                        effective_member_type(syntax, *element_type, substitution)?;
                    let values = syntax.expressions.expression_handles(*elements);
                    if values.len() != elements.len() {
                        return Err("computed array constant has a stale element roster".to_owned());
                    }
                    for element in values {
                        collect(
                            syntax,
                            selection,
                            *element,
                            element_type,
                            substitution,
                            leaves,
                        )?;
                    }
                    Ok(())
                }
                // Aggregate-producing leaves keep their authored expression.
                // The leaf probe's checked interpreter supplies the result and
                // admission checks it against this exact array carrier once
                // its declaration-owned geometry dependencies have completed.
                ExpressionNode::Call(_) | ExpressionNode::Match(_)
                    if closed_leaf_carrier(syntax, type_reference, substitution) =>
                {
                    leaves.push(PendingConstInitializerLeaf {
                        expression,
                        destination: type_reference,
                        structured: true,
                    });
                    Ok(())
                }
                ExpressionNode::Name(path)
                    if closed_leaf_carrier(syntax, type_reference, substitution)
                        && name_selects_const(syntax, selection, *path) =>
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
                    selection.builtin_type(syntax, name),
                    Some(symbols::BuiltinTypeAtom::I8 | symbols::BuiltinTypeAtom::I16
                        | symbols::BuiltinTypeAtom::I32 | symbols::BuiltinTypeAtom::I64
                        | symbols::BuiltinTypeAtom::U8 | symbols::BuiltinTypeAtom::U16
                        | symbols::BuiltinTypeAtom::U32 | symbols::BuiltinTypeAtom::U64
                        | symbols::BuiltinTypeAtom::Bool
                        | symbols::BuiltinTypeAtom::F32 | symbols::BuiltinTypeAtom::F64)
                ) =>
            {
                let materialized = match syntax.expressions.expression(expression) {
                    ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
                    // Decimal-to-integer landing still needs evaluation; a
                    // floating declaration's literal validation owns its suffix.
                    ExpressionNode::Float(_) | ExpressionNode::UndeterminedNaN(_) => matches!(selection.builtin_type(syntax, name),
                        Some(symbols::BuiltinTypeAtom::F32 | symbols::BuiltinTypeAtom::F64)),
                    _ => false,
                };
                if !materialized {
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
                    if selection.builtin_type(syntax, name).is_none() {
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
                // A nongeneric declaration's members never see an enclosing
                // application's parameters: a member spelling `T` there
                // selects whatever `T` resolves to in this declaration's own
                // scope.
                collect_literal_fields(
                    syntax,
                    selection,
                    literal,
                    declared,
                    &GenericApplicationSubstitution::new(),
                    leaves,
                )
            }
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                // A closed application `Base<args..>` stands in for its
                // synthesized instance: the selected template keeps exact
                // declaration custody while its parameter bindings map each
                // member carrier onto the authored argument handles.
                let declared = selection.data(syntax, base_name)?;
                let argument_handles =
                    syntax.type_references.type_reference_handles(*arguments);
                let parameters = syntax.items.type_parameters(declared.type_parameters);
                if !lifetime_arguments.is_empty()
                    || !declared.lifetime_parameters.is_empty()
                    || argument_handles.len() != arguments.len()
                    || parameters.len() != declared.type_parameters.len()
                    || parameters.len() != argument_handles.len()
                {
                    return Err(
                        "computed constant requires a closed generic application".to_owned(),
                    );
                }
                let next_substitution = substituted_generic_arguments(syntax, parameters, &argument_handles, substitution)?;
                match syntax.expressions.expression(expression) {
                    ExpressionNode::StructLiteral(literal) => collect_literal_fields(
                        syntax,
                        selection,
                        literal,
                        declared,
                        &next_substitution,
                        leaves,
                    ),
                    // A call, match, or evaluated constant at a closed
                    // application destination evaluates as one structured leaf
                    // whose probe return type spells the application itself.
                    ExpressionNode::Call(_) | ExpressionNode::Match(_)
                        if closed_leaf_carrier(syntax, type_reference, substitution) =>
                    {
                        leaves.push(PendingConstInitializerLeaf {
                            expression,
                            destination: type_reference,
                            structured: true,
                        });
                        Ok(())
                    }
                    ExpressionNode::Name(path)
                        if closed_leaf_carrier(syntax, type_reference, substitution)
                            && name_selects_const(syntax, selection, *path) =>
                    {
                        leaves.push(PendingConstInitializerLeaf {
                            expression,
                            destination: type_reference,
                            structured: true,
                        });
                        Ok(())
                    }
                    ExpressionNode::Call(_) | ExpressionNode::Match(_) => Err(
                        "computed constant call carrier is not a closed generic application"
                            .to_owned(),
                    ),
                    // Existing literal validation owns payloadless cases and
                    // noncomputed scalar formats; they introduce no probe leaf.
                    ExpressionNode::Name(_)
                    | ExpressionNode::Integer(_)
                    | ExpressionNode::Boolean(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::String(_) => Ok(()),
                    _ => Err(
                        "computed constant leaf requires an exact builtin integer or Boolean carrier"
                            .to_owned(),
                    ),
                }
            }
            // A constrained carrier keeps its declared constraints for
            // downstream discharge; the leaf itself evaluates at the base
            // carrier, matching canonicalization's `Constrained` recursion.
            TypeReferenceNode::Constrained { base_type, .. } => {
                collect(syntax, selection, expression, *base_type, substitution, leaves)
            }
            _ => Err(
                "computed constant requires a closed array, nominal literal, or builtin scalar carrier"
                    .to_owned(),
            ),
        }
    }

    fn collect_literal_fields(
        syntax: &SyntaxTrees,
        selection: &crate::preparation::declaration_selection::ConstantSelection,
        literal: &syntax_trees::expression::TableStructLiteral,
        declared: &DataDefinition,
        substitution: &GenericApplicationSubstitution,
        leaves: &mut Vec<PendingConstInitializerLeaf>,
    ) -> Result<(), String> {
        let (constructed, case) = selection.constructor(syntax, &literal.constructor_name)?;
        if !std::ptr::eq(declared, constructed) {
            return Err(
                "computed constant constructor differs from its declared nominal carrier"
                    .to_owned(),
            );
        }
        let members = syntax.items.data_members(declared.members);
        if members.len() != declared.members.len() {
            return Err("computed constant data member span is stale".to_owned());
        }
        let fields = if let Some(case) = case {
            let variant = members
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant)
                        if variant.name.as_str(syntax) == case.as_str(syntax) =>
                    {
                        Some(variant)
                    }
                    _ => None,
                })
                .ok_or("computed constant case is absent from its selected owner")?;
            let fields = syntax.items.data_payload_fields(variant.payload);
            if fields.len() != variant.payload.len() {
                return Err("computed constant payload span is stale".to_owned());
            }
            fields.iter().collect::<Vec<_>>()
        } else {
            if members
                .iter()
                .any(|member| matches!(member, DataMember::Variant(_)))
            {
                return Err("computed constant sum must select its exact case".to_owned());
            }
            members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field),
                    _ => None,
                })
                .collect()
        };
        let authored = syntax.expressions.struct_fields(literal.fields);
        if authored.len() != literal.fields.len() || authored.len() != fields.len() {
            return Err("computed constant field roster differs from its declaration".to_owned());
        }
        // Keep authored evaluation order, but select field types only
        // within the exact constructor's retained declaration; application
        // bindings map parameter carriers onto their argument handles.
        for field in authored {
            if authored
                .iter()
                .filter(|candidate| candidate.name.as_str(syntax) == field.name.as_str(syntax))
                .count()
                != 1
            {
                return Err(format!(
                    "duplicate computed constant field `{}`",
                    field.name.as_str(syntax)
                ));
            }
            let declaration = fields
                .iter()
                .find(|candidate| candidate.name.as_str(syntax) == field.name.as_str(syntax))
                .ok_or_else(|| {
                    format!(
                        "unknown computed constant field `{}`",
                        field.name.as_str(syntax)
                    )
                })?;
            let member_type =
                effective_member_type(syntax, declaration.type_reference, substitution)?;
            collect(
                syntax,
                selection,
                field.value,
                member_type,
                substitution,
                leaves,
            )?;
        }
        Ok(())
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
        &GenericApplicationSubstitution::new(),
        &mut leaves,
    )?;
    Ok(leaves)
}

/// The selected declaration for a computed nominal constant must be closed:
/// generic or lifetime-carrying owners cannot produce one materialized value.
fn require_closed_data<'syntax>(
    selection: &crate::preparation::declaration_selection::ConstantSelection,
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
    selection: &crate::preparation::declaration_selection::ConstantSelection,
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
    let name = members.joined_path_in(syntax);
    selection.selects_const(
        syntax,
        &syntax_trees::identifier::Identifier::new(name, reference),
    )
}

//! Pending const initializer leaves and aggregate placeholders.

use crate::constant::requires_const_initializer_evaluation;
use crate::preparation::generic_data::constant_selection::{
    GenericApplicationSubstitution, has_deferred_const_argument, resolved_generic_argument,
    type_mentions_parameters,
};
use source::{SourceSpan, Span};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ConstDefinition, DataDefinition, DataMember};
use syntax_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

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
        TypeReferenceNode::Named(name) if substitution.contains_key(name.as_str()) => {
            Ok(substitution[name.as_str()])
        }
        TypeReferenceNode::Generic { .. } | TypeReferenceNode::FixedArray { .. } => Ok(member_type),
        _ if type_mentions_parameters(syntax, member_type, substitution) => {
            Err("computed constant member type is not yet a closed structural type".to_owned())
        }
        _ => Ok(member_type),
    }
}

/// A fixed-array carrier length, closed by its literal or by the enclosing
/// application's binding for a declared const parameter.
fn resolved_fixed_length(
    syntax: &SyntaxTrees,
    length: &FixedArrayLength,
    substitution: &GenericApplicationSubstitution,
) -> Result<usize, String> {
    match length {
        FixedArrayLength::Literal(length) => Ok(*length),
        FixedArrayLength::ConstParameter(name) => {
            let Some(argument) = substitution.get(name.as_str()) else {
                return Err("computed array constant length is an open parameter".to_owned());
            };
            let TypeReferenceNode::Named(value) = syntax.type_references.type_reference(*argument)
            else {
                return Err("computed array constant length is not a closed literal".to_owned());
            };
            value
                .as_str()
                .parse::<usize>()
                .map_err(|_| "computed array constant length is not a closed literal".to_owned())
        }
        FixedArrayLength::ConstCall(_) => {
            Err("computed array constant length is not a closed literal".to_owned())
        }
    }
}

pub(crate) fn pending_const_initializer_leaves(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
) -> Result<Vec<PendingConstInitializerLeaf>, String> {
    use syntax_trees::expression::ExpressionNode;

    fn collect(
        syntax: &SyntaxTrees,
        selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
        expression: syntax_trees::expression::ExpressionHandle,
        type_reference: TypeReferenceHandle,
        substitution: &GenericApplicationSubstitution,
        leaves: &mut Vec<PendingConstInitializerLeaf>,
    ) -> Result<(), String> {
        // A carrier spelled as a bare parameter reselects the enclosing
        // application's already-closed argument handle.
        if let TypeReferenceNode::Named(name) =
            syntax.type_references.type_reference(type_reference)
            && let Some(argument) = substitution.get(name.as_str())
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
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => match syntax.expressions.expression(expression) {
                ExpressionNode::ArrayLiteral(elements) => {
                    let length = resolved_fixed_length(syntax, length, substitution)?;
                    let element_type =
                        effective_member_type(syntax, *element_type, substitution)?;
                    let values = syntax.expressions.expression_handles(*elements);
                    if values.len() != elements.len() || values.len() != length {
                        return Err(
                            "computed array constant has an incorrect element roster"
                                .to_owned(),
                        );
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
                // admission checks it against this exact closed array carrier;
                // a parameter-spelled length stays on the literal-only path.
                ExpressionNode::Call(_) | ExpressionNode::Match(_)
                    if matches!(length, FixedArrayLength::Literal(_))
                        && closed_leaf_carrier(syntax, type_reference, substitution) =>
                {
                    leaves.push(PendingConstInitializerLeaf {
                        expression,
                        destination: type_reference,
                        structured: true,
                    });
                    Ok(())
                }
                ExpressionNode::Name(path)
                    if matches!(length, FixedArrayLength::Literal(_))
                        && closed_leaf_carrier(syntax, type_reference, substitution)
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
                    selection.builtin_type(name),
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
                    ExpressionNode::Float(_) => matches!(selection.builtin_type(name),
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
                let mut next_substitution = GenericApplicationSubstitution::new();
                for (parameter, argument) in parameters.iter().zip(argument_handles.iter()) {
                    next_substitution.insert(
                        parameter.name.as_str().to_owned(),
                        resolved_generic_argument(syntax, *argument, substitution)?,
                    );
                }
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
        selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
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
                    DataMember::Variant(variant) if variant.name.as_str() == case.as_str() => {
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
                .filter(|candidate| candidate.name.as_str() == field.name.as_str())
                .count()
                != 1
            {
                return Err(format!(
                    "duplicate computed constant field `{}`",
                    field.name
                ));
            }
            let declaration = fields
                .iter()
                .find(|candidate| candidate.name.as_str() == field.name.as_str())
                .ok_or_else(|| format!("unknown computed constant field `{}`", field.name))?;
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
    type_reference: TypeReferenceHandle,
    reference: SourceSpan,
) -> Result<syntax_trees::expression::ExpressionHandle, String> {
    pending_aggregate_placeholder_at(
        syntax,
        selection,
        type_reference,
        &GenericApplicationSubstitution::new(),
        reference,
    )
}

fn pending_aggregate_placeholder_at(
    syntax: &mut SyntaxTrees,
    selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
    type_reference: TypeReferenceHandle,
    substitution: &GenericApplicationSubstitution,
    reference: SourceSpan,
) -> Result<syntax_trees::expression::ExpressionHandle, String> {
    use syntax_trees::expression::ExpressionNode;

    // A carrier spelled as a bare parameter reselects the enclosing
    // application's already-closed argument handle.
    if let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(type_reference)
        && let Some(argument) = substitution.get(name.as_str())
    {
        return pending_aggregate_placeholder_at(
            syntax,
            selection,
            *argument,
            substitution,
            reference,
        );
    }
    // A rewritten `Named` instance spelling resolves through its retained
    // authored application rather than the synthesized name.
    let origin = syntax
        .type_references
        .generic_application_origin(type_reference);
    if origin.is_valid() && origin != type_reference {
        return pending_aggregate_placeholder_at(
            syntax,
            selection,
            origin,
            substitution,
            reference,
        );
    }
    // The node is cloned so recursive placeholder construction may insert
    // into the same syntax forest while this carrier stays borrowed.
    let destination = syntax
        .type_references
        .type_reference(type_reference)
        .clone();
    let node = match &destination {
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let length = resolved_fixed_length(syntax, length, substitution)?;
            let element_type = effective_member_type(syntax, *element_type, substitution)?;
            let element = pending_aggregate_placeholder_at(
                syntax,
                selection,
                element_type,
                substitution,
                reference,
            )?;
            let elements = syntax
                .expressions
                .insert_expression_handles(vec![element; length]);
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
                let declared = require_closed_data(selection, syntax, name)?;
                // A nongeneric declaration's members never see an enclosing
                // application's parameters: a member spelling `T` there
                // selects whatever `T` resolves to in this declaration's own
                // scope. The member list is cloned so the declaration borrow
                // ends before recursive placeholder construction inserts into
                // the same forest.
                let members = syntax.items.data_members(declared.members).to_vec();
                let member_count = declared.members.len();
                nominal_placeholder_literal(
                    syntax,
                    selection,
                    &members,
                    member_count,
                    name.as_str(),
                    name.source_span(),
                    &GenericApplicationSubstitution::new(),
                    reference,
                )?
            }
        },
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            // A closed application's placeholder is built from the selected
            // template under its argument bindings; nested applications keep
            // their authored spelling and resolve one level at a time.
            let declared = selection.data(syntax, base_name)?;
            let argument_handles = syntax
                .type_references
                .type_reference_handles(*arguments)
                .to_vec();
            let parameters = syntax.items.type_parameters(declared.type_parameters);
            if !lifetime_arguments.is_empty()
                || !declared.lifetime_parameters.is_empty()
                || argument_handles.len() != arguments.len()
                || parameters.len() != declared.type_parameters.len()
                || parameters.len() != argument_handles.len()
            {
                return Err(
                    "aggregate initializer placeholder requires a closed generic application"
                        .to_owned(),
                );
            }
            let mut next_substitution = GenericApplicationSubstitution::new();
            for (parameter, argument) in parameters.iter().zip(argument_handles.iter()) {
                next_substitution.insert(
                    parameter.name.as_str().to_owned(),
                    resolved_generic_argument(syntax, *argument, substitution)?,
                );
            }
            // The member list is cloned so the declaration borrow ends before
            // recursive placeholder construction inserts into the same forest.
            let members = syntax.items.data_members(declared.members).to_vec();
            let member_count = declared.members.len();
            nominal_placeholder_literal(
                syntax,
                selection,
                &members,
                member_count,
                base_name.as_str(),
                base_name.source_span(),
                &next_substitution,
                reference,
            )?
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            return pending_aggregate_placeholder_at(
                syntax,
                selection,
                *base_type,
                substitution,
                reference,
            );
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

/// One zero literal for the selected closed declaration: a sum's first case or
/// a record's fields, each member carrier resolved through the enclosing
/// application's bindings. `members` is an owned clone so this call may insert
/// into the same forest.
fn nominal_placeholder_literal(
    syntax: &mut SyntaxTrees,
    selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
    members: &[DataMember],
    member_count: usize,
    constructor: &str,
    constructor_span: SourceSpan,
    substitution: &GenericApplicationSubstitution,
    reference: SourceSpan,
) -> Result<syntax_trees::expression::ExpressionNode, String> {
    use syntax_trees::expression::{ExpressionNode, TableStructLiteral, TableStructLiteralField};
    if members.len() != member_count {
        return Err("aggregate initializer placeholder has a stale data member span".to_owned());
    }
    let (constructor, fields) = if let Some(variant) =
        members.iter().find_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            _ => None,
        }) {
        let payload = syntax.items.data_payload_fields(variant.payload);
        if payload.len() != variant.payload.len() {
            return Err("aggregate initializer placeholder has a stale payload span".to_owned());
        }
        (
            format!("{constructor}::{}", variant.name.as_str()),
            payload
                .iter()
                .map(|field| (field.name.as_str().to_owned(), field.type_reference))
                .collect::<Vec<_>>(),
        )
    } else {
        (
            constructor.to_owned(),
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
    };
    let mut literal_fields = Vec::with_capacity(fields.len());
    for (field_name, field_type) in fields {
        let field_type = effective_member_type(syntax, field_type, substitution)?;
        let value = pending_aggregate_placeholder_at(
            syntax,
            selection,
            field_type,
            substitution,
            reference,
        )?;
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
    Ok(ExpressionNode::StructLiteral(TableStructLiteral {
        constructor_name: syntax_trees::identifier::Identifier::new(constructor, constructor_span),
        fields,
    }))
}

/*
    A const parameter's requirement type joins the existing nominal callable
    contract only after its owner resolves as a trait. The scoped static
    selection keeps arguments on that owner, independently of its member.
    This normalization runs in the lexical declared-type walk before constant
    discovery and full symbol assignment; no transient selection symbol escapes.
*/

use super::declaration_selection::ConstantSelection;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbols::SymbolKind;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionNode, StaticMachineArgument};
use syntax_trees::identifier::{Identifier, JoinedSyntaxPath};
use syntax_trees::item::MachineParameterContract;
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn nominal_contract(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection<'_>,
    reference: TypeReferenceHandle,
    bound: &[String],
) -> Result<Option<MachineParameterContract>, Diagnostic> {
    let (owner, member, arguments) = match syntax.type_references.type_reference(reference).clone()
    {
        TypeReferenceNode::Named(name) => {
            let Some((owner, member)) = split_member(syntax, &name) else {
                return Ok(None);
            };
            (owner, member, Vec::new())
        }
        TypeReferenceNode::Generic { base_name, .. } => {
            let Some((owner, _)) = split_member(syntax, &base_name) else {
                return Ok(None);
            };
            if is_trait_owner(syntax, selection, &owner, bound)? {
                return Err(Diagnostic::error(
                    "a nominal callable type applies the trait before `::requirement`, not the requirement member",
                ).with_source_span(base_name.source_span()));
            }
            return Ok(None);
        }
        TypeReferenceNode::StaticSelection(selected) => {
            let owner = joined_name(syntax, &selected.path)?;
            if !is_trait_owner(syntax, selection, &owner, bound)? {
                return Ok(None);
            }
            let Some(application) = selected.application else {
                return Err(
                    Diagnostic::error("a scoped callable type lost its trait application")
                        .with_source_span(owner.source_span()),
                );
            };
            if !application.lifetime_arguments.is_empty() {
                return Err(Diagnostic::error(
                    "a nominal callable requirement trait application takes no lifetime arguments",
                )
                .with_source_span(owner.source_span()));
            }
            if !application.member.is_valid() {
                return Err(Diagnostic::error(
                    "a nominal callable type must name one exact trait requirement",
                )
                .with_source_span(owner.source_span()));
            }
            let member = syntax
                .expressions
                .static_member(application.member)
                .ok_or_else(|| {
                    Diagnostic::error("a nominal callable type has an invalid requirement member")
                        .with_source_span(owner.source_span())
                })?;
            let [name] = member.path.as_ref() else {
                return Err(Diagnostic::error("a nominal callable type must name one exact trait requirement after its application")
                    .with_source_span(owner.source_span()));
            };
            if member.application.is_some() {
                return Err(Diagnostic::error("a nominal callable type applies the trait before `::requirement`, not the requirement member")
                    .with_source_span(name.source_span()));
            }
            (owner, name.clone(), application.arguments.into_vec())
        }
        _ => return Ok(None),
    };
    if !is_trait_owner(syntax, selection, &owner, bound)? {
        return Ok(None);
    }
    let arguments = arguments
        .iter()
        .map(|argument| argument_type(syntax, argument))
        .collect::<Result<Vec<_>, _>>()?;
    let trait_arguments = syntax
        .type_references
        .insert_type_reference_handles(arguments);
    let mut requirement = HandleSpan::empty();
    requirement.push_contiguous(syntax.items.append_identifier_path_member(owner));
    requirement.push_contiguous(syntax.items.append_identifier_path_member(member));
    Ok(Some(MachineParameterContract::Nominal {
        requirement,
        trait_arguments,
    }))
}

fn is_trait_owner(
    syntax: &SyntaxTrees,
    selection: &ConstantSelection<'_>,
    owner: &Identifier,
    bound: &[String],
) -> Result<bool, Diagnostic> {
    let head = owner.as_str(syntax).split("::").next().unwrap_or_default();
    if bound.iter().any(|name| name == head) {
        return Ok(false);
    }
    Ok(selection.type_declaration_kind(syntax, owner)? == Some(SymbolKind::Trait))
}

fn split_member(syntax: &SyntaxTrees, name: &Identifier) -> Option<(Identifier, Identifier)> {
    let (owner, member) = name.as_str(syntax).rsplit_once("::")?;
    let mut owner_span = name.source_span();
    owner_span.span.end = owner_span.span.start + owner.len();
    Some((
        Identifier::new(owner, owner_span),
        Identifier::new(member, name.trailing_span(member.len())),
    ))
}

fn joined_name(syntax: &SyntaxTrees, path: &[Identifier]) -> Result<Identifier, Diagnostic> {
    let Some(first) = path.first() else {
        return Err(Diagnostic::error(
            "a static type selection has no declaration path",
        ));
    };
    let mut span = first.source_span();
    for member in &path[1..] {
        if member.source_span().source_id != span.source_id {
            return Err(
                Diagnostic::error("a static type selection mixes supplying sources")
                    .with_source_span(span),
            );
        }
        span.span.end = member.source_span().span.end;
    }
    Ok(Identifier::new(path.joined_path_in(syntax), span))
}

fn argument_type(
    syntax: &mut SyntaxTrees,
    argument: &StaticMachineArgument,
) -> Result<TypeReferenceHandle, Diagnostic> {
    if argument.type_reference.is_valid() {
        return Ok(argument.type_reference);
    }
    if let Some(literal) = &argument.const_literal {
        let expression = syntax
            .expressions
            .insert(ExpressionNode::Integer(literal.clone()));
        return Ok(syntax
            .type_references
            .insert(TypeReferenceNode::ConstExpression(expression)));
    }
    if argument.evidence_projection.is_some()
        || argument
            .application
            .as_ref()
            .is_some_and(|application| application.member.is_valid())
    {
        return Ok(syntax
            .type_references
            .insert(TypeReferenceNode::StaticSelection(argument.clone())));
    }
    let name = joined_name(syntax, &argument.path)?;
    let reference = if let Some(application) = &argument.application {
        let arguments = application
            .arguments
            .iter()
            .map(|argument| argument_type(syntax, argument))
            .collect::<Result<Vec<_>, _>>()?;
        TypeReferenceNode::Generic {
            base_name: name,
            lifetime_arguments: application.lifetime_arguments.to_vec(),
            arguments: syntax
                .type_references
                .insert_type_reference_handles(arguments),
        }
    } else {
        TypeReferenceNode::Named(name)
    };
    Ok(syntax.type_references.insert(reference))
}

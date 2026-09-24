//! Resolve carrier-binder domain heads before substituting their carrier.
//!
//! `T in T::Marked` selects a family in the template author's scope. Once
//! `T` becomes an array or a foreign nominal type, the original carrier prefix
//! can no longer resolve there. Select the exact exposed generic-carrier family
//! while the owning telescope is available and retain its complete declaration
//! address with the authored use span. Original templates and their instances
//! then use the same ordinary domain lookup and authored-selection evidence;
//! the receiving package's imports cannot redirect either one.
//! Alias constituents use the same selection: their carrier prefix refers to
//! the alias's binder, not a top-level module with the same spelling. This
//! binds names only; typed alias application still checks subject compatibility
//! and the selected family's requirements before expanding its atoms.

use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{DataMember, Item, TypeParameterKind};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

use super::constant_selection::ConstantSelection;

pub(in crate::preparation) fn normalize(
    syntax: &mut SyntaxTrees,
    selection: Option<&ConstantSelection>,
) -> Result<(), Vec<Diagnostic>> {
    let mut pending = Vec::new();
    let mut alias_heads = Vec::new();
    for &declaration in syntax.root_item_handles() {
        let item = syntax.root_item(declaration);
        if let Item::Domain(domain) = item {
            let Some(alias) = &domain.alias else {
                continue;
            };
            let parameters = syntax.items.type_parameters(domain.type_parameters);
            for (ordinal, constituent) in alias.constituents.iter().enumerate() {
                let members = syntax.items.identifier_path_members(*constituent);
                if members.len() < 2 {
                    continue;
                }
                let Some(carrier) = members.first() else {
                    continue;
                };
                let Some(parameter) = parameters
                    .iter()
                    .find(|parameter| parameter.name.as_str() == carrier.as_str())
                else {
                    continue;
                };
                let [_, leaf] = members else {
                    // A lexical binder cannot become a same-spelled module
                    // just because another path component follows it.
                    return Err(vec![Diagnostic::error(format!(
                        "domain alias carrier type binder `{}` must be followed by one domain name",
                        carrier.as_str()
                    )).with_source_span(carrier.source_span())]);
                };
                if !matches!(parameter.kind, TypeParameterKind::Type)
                    || named_carrier(syntax, domain.target_type) != Some(carrier.as_str())
                {
                    return Err(vec![Diagnostic::error(format!(
                        "domain alias constituent `{}::{}` must name this alias's carrier type binder",
                        carrier.as_str(), leaf.as_str()
                    )).with_source_span(carrier.source_span())]);
                }
                alias_heads.push((
                    declaration,
                    ordinal,
                    leaf.as_str().to_owned(),
                    carrier.source_span(),
                ));
            }
            continue;
        }
        let Item::Data(definition) = item else {
            continue;
        };
        let parameters = syntax.items.type_parameters(definition.type_parameters);
        if parameters.is_empty() {
            continue;
        }
        let mut types = Vec::new();
        for member in syntax.items.data_members(definition.members) {
            match member {
                DataMember::Field(field) => types.push(field.type_reference),
                DataMember::Variant(variant) => types.extend(
                    syntax
                        .items
                        .data_payload_fields(variant.payload)
                        .iter()
                        .map(|field| field.type_reference),
                ),
                DataMember::Retired(_) => {}
            }
        }
        while let Some(site) = types.pop() {
            match syntax.type_references.type_reference(site) {
                TypeReferenceNode::Reference { referee, .. } => types.push(*referee),
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => types.push(*element_type),
                TypeReferenceNode::Generic { arguments, .. } => {
                    types.extend(syntax.type_references.type_reference_handles(*arguments))
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    types.push(*base_type);
                    for (ordinal, constraint) in syntax
                        .type_references
                        .constraints(*constraints)
                        .iter()
                        .enumerate()
                    {
                        let TypeConstraintNode::Domain(domain) = constraint else {
                            continue;
                        };
                        types.extend(
                            syntax
                                .type_references
                                .type_reference_handles(domain.arguments),
                        );
                        let Some((carrier, leaf)) = domain.name.as_str().rsplit_once("::") else {
                            continue;
                        };
                        // The head must bind the same lexical type parameter as
                        // this field's carrier. A module or top-level type with
                        // the same spelling supplies no substitute authority.
                        let Some(parameter) = parameters
                            .iter()
                            .find(|parameter| parameter.name.as_str() == carrier)
                        else {
                            continue;
                        };
                        if !matches!(parameter.kind, TypeParameterKind::Type)
                            || named_carrier(syntax, *base_type) != Some(carrier)
                        {
                            return Err(vec![Diagnostic::error(format!(
                                "domain qualification `{}` must name this field's carrier type binder",
                                domain.name.as_str()
                            )).with_source_span(domain.name.source_span())]);
                        }
                        pending.push((site, ordinal, leaf.to_owned(), domain.name.source_span()));
                    }
                }
                TypeReferenceNode::Named(_)
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::SelfType
                | TypeReferenceNode::Unit => {}
            }
        }
    }
    if pending.is_empty() && alias_heads.is_empty() {
        return Ok(());
    }
    let fallback;
    let selection = if let Some(selection) = selection {
        selection
    } else {
        fallback = ConstantSelection::new(syntax, None, Vec::new())?;
        &fallback
    };
    for (site, ordinal, leaf, reference) in pending {
        let Some(address) = selection.generic_carrier_domain_address(syntax, &leaf, reference)
        else {
            // The prefix already bound a lexical type parameter. Falling
            // through could instead select a same-spelled top-level module.
            return Err(vec![Diagnostic::error(format!(
                "carrier-qualified domain `{leaf}` does not select one exposed generic-carrier family"
            )).with_source_span(reference)]);
        };
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = syntax.type_references.type_reference(site).clone()
        else {
            continue;
        };
        let mut constraints = syntax.type_references.constraints(constraints).to_vec();
        let Some(TypeConstraintNode::Domain(domain)) = constraints.get_mut(ordinal) else {
            continue;
        };
        domain.name = Identifier::new(address, reference);
        let constraints = syntax.type_references.insert_constraints(constraints);
        syntax.type_references.replace_type_reference(
            site,
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            },
        );
    }
    for (declaration, ordinal, leaf, reference) in alias_heads {
        let Some(address) = selection.generic_carrier_domain_address(syntax, &leaf, reference)
        else {
            return Err(vec![Diagnostic::error(format!(
                "carrier-qualified domain `{leaf}` does not select one exposed generic-carrier family"
            )).with_source_span(reference)]);
        };
        let Item::Domain(mut domain) = syntax.root_item(declaration).clone() else {
            continue;
        };
        let Some(alias) = &mut domain.alias else {
            continue;
        };
        // One selected address is not another authored carrier-head path.
        // Retaining it as one identifier also makes repeated preparation
        // idempotent when the declaring module has a binder's spelling.
        alias.constituents[ordinal] = syntax
            .items
            .insert_identifier_path_members([Identifier::new(address, reference)]);
        syntax.items.replace_item(declaration, Item::Domain(domain));
    }
    Ok(())
}

fn named_carrier(syntax: &SyntaxTrees, mut carrier: TypeReferenceHandle) -> Option<&str> {
    loop {
        match syntax.type_references.type_reference(carrier) {
            TypeReferenceNode::Named(name) => return Some(name.as_str()),
            TypeReferenceNode::Constrained { base_type, .. } => carrier = *base_type,
            _ => return None,
        }
    }
}

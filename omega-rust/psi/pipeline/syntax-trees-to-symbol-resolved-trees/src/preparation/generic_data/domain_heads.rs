//! Resolve a generic field's carrier-qualified domain before substituting its carrier.
//!
//! `T in T::Marked` selects a family in the template author's scope. Once
//! `T` becomes an array or a foreign nominal type, the original carrier prefix
//! can no longer resolve there. Select the exact exposed generic-carrier family
//! while the owning telescope is available and retain its complete declaration
//! address with the authored use span. Original templates and their instances
//! then use the same ordinary domain lookup and authored-selection evidence;
//! the receiving package's imports cannot redirect either one.

use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{DataMember, Item, TypeParameterKind};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

use super::constant_selection::ConstantSelection;

pub(super) fn normalize(
    syntax: &mut SyntaxTrees,
    selection: Option<&ConstantSelection>,
) -> Result<(), Vec<Diagnostic>> {
    let mut pending = Vec::new();
    for item in syntax.root_items() {
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
    if pending.is_empty() {
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

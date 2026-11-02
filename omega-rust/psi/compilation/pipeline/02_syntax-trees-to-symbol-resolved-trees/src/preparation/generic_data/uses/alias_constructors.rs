//! Infer a qualified alias constructor from its complete expected application.
//! The alias is never a nominal carrier or owner of synthetic case symbols.

use super::super::{Instantiation, SyntaxTrees, TypeReferenceHandle};
use crate::preparation::declaration_selection::ConstantSelection;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{DataMember, ItemHandle};

pub(super) struct AliasConstructor {
    pub template: ItemHandle,
    pub case: Option<Identifier>,
    pub application: TypeReferenceHandle,
    pub carrier: Identifier,
}

pub(super) fn select(
    syntax: &mut SyntaxTrees,
    selection: Option<&ConstantSelection<'_>>,
    expected: TypeReferenceHandle,
    instance: &Instantiation,
    authored: &Identifier,
    payload_free: bool,
) -> Option<AliasConstructor> {
    let selection = selection?;
    if payload_free && selection.constructor_prefix_is_value(syntax, authored) {
        return None;
    }
    let (carrier, case) = if selection.type_alias_source(syntax, authored).is_some() {
        (authored.clone(), None)
    } else {
        let (prefix, case) = authored.as_str(syntax).rsplit_once("::")?;
        let mut span = authored.source_span();
        span.span.end = span.span.start + prefix.len();
        (
            Identifier::new(prefix, span),
            Some(Identifier::new(case, authored.trailing_span(case.len()))),
        )
    };
    let declaration = selection.type_alias_source(syntax, &carrier)?;
    // Missing or ambiguous inference is not permission to choose the expanded
    // template alone: erased arguments and all supplier coordinates still owe
    // their original alias's formation.
    let mut origins = syntax
        .type_references
        .type_alias_applications(expected)
        .filter(|origin| origin.declaration == declaration);
    origins.next()?;
    if origins.next().is_some() {
        return None;
    }
    drop(origins);
    let template = syntax.root_item(instance.template).as_data()?;
    if let Some(case) = &case {
        let mut variants = syntax
            .items
            .data_members(template.members)
            .iter()
            .filter_map(|member| {
                if let DataMember::Variant(variant) = member {
                    (variant.name.as_str(syntax) == case.as_str(syntax)).then_some(variant)
                } else {
                    None
                }
            });
        let variant = variants.next()?;
        if variants.next().is_some() || (payload_free && !variant.payload.is_empty()) {
            return None;
        }
    } else if payload_free {
        return None;
    }
    let lookup = selection.data_lookup_path_for_declaration(syntax, &template.name, &carrier)?;
    let lookup = lookup.rsplit_once("::").map_or_else(
        || instance.synthetic_name.clone(),
        |(prefix, _)| format!("{prefix}::{}", instance.synthetic_name),
    );
    let application =
        syntax
            .type_references
            .alias_use_at(expected, declaration, carrier.clone())?;
    Some(AliasConstructor {
        template: instance.template,
        case,
        application,
        carrier: Identifier::new(lookup, carrier.source_span()),
    })
}

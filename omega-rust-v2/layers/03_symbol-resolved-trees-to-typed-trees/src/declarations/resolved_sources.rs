use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_resolver::{NameRole, Symbol, SymbolResolvedTrees};

pub(crate) fn index_resolved_sources<'a>(resolved: &'a SymbolResolvedTrees<'_>) -> Vec<ResolvedSourceFile<'a>> {
    resolved
        .trees()
        .map(|tree| ResolvedSourceFile {
            tree,
            inferred_callbacks: Default::default(),
            nominal_callbacks: Vec::new(),
            universe_binders: Default::default(),
            parameters: Default::default(),
            hypotheses: Default::default(),
        })
        .collect()
}

/// Only the normalized binder type tells whether a lexical root can compete with a namespace.
pub(crate) fn check_static_qualifiers(
    resolved: &SymbolResolvedTrees<'_>,
    units: &[ResolvedSourceFile<'_>],
    types: &TypeCatalog<'_>,
) -> Result<(), TypeError> {
    for name in resolved
        .uses()
        .filter(|name| name.role == NameRole::CompetingStaticRoot)
    {
        if let Symbol::Binding { span, .. } = &name.target {
            let unit = &units[name.node.file];
            let carrier = unit
                .parameters
                .get(span)
                .and_then(|parameter| types.parameter_carrier(parameter));
            if unit.hypotheses.contains_key(span) || matches!(carrier, Some(ValueType::Evidence(_) | ValueType::Callable(_))) {
                return Err(TypeError {
                    span: name.span,
                    ..TypeError::at(unit.file, name.node.node, "ambiguous static root")
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn check_domain_visibility(
    resolved: &SymbolResolvedTrees<'_>,
    units: &[ResolvedSourceFile<'_>],
    types: &TypeCatalog<'_>,
) -> Result<(), TypeError> {
    for name in resolved.uses() {
        if name.role == NameRole::Qualifier && matches!(name.target, Symbol::DomainCandidates { .. }) {
            types
                .selected_declaration(&name.target)
                .map_err(|message| TypeError {
                    span: name.span,
                    ..TypeError::at(units[name.node.file].file, name.node.node, message)
                })?;
        }
    }
    for check in &resolved.carrier_visibility {
        if check
            .binders
            .iter()
            .any(|binder| types.binder_names_type(binder))
        {
            continue;
        }
        let unit = &units[check.site.file];
        let node = check.site.node;
        if let Some(name @ Symbol::DomainCandidates { .. }) = unit.symbol(node).or_else(|| unit.qualifier(node))
            && types
                .selected_declaration(name)
                .map_err(|message| TypeError::at(unit.file, node, message))?
                != &check.domain
        {
            continue;
        }
        let error = &check.error;
        return Err(TypeError {
            span: error.span,
            ..TypeError::at(unit.file, node, &error.message)
        });
    }
    Ok(())
}

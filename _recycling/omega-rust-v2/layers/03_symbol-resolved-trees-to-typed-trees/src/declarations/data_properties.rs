use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::{DataProperties, Multiplicity, TypeError, TypedStage};

pub(crate) fn read(
    unit: &ResolvedSourceFile<'_>,
    n: usize,
) -> Result<DataProperties, TypeError> {
    let f = unit.file;
    let mut properties = DataProperties::default();
    for &a in &f.nodes[n].children {
        let mut words = f.nodes[a].words();
        match words.next().map(|w| w.text) {
            Some(kind @ ("copy" | "linear")) if properties.multiplicity == Multiplicity::Affine && words.next().is_none() => {
                properties.multiplicity = if kind == "copy" { Multiplicity::Copy } else { Multiplicity::Linear };
            }
            _ => return Err(TypeError::at(f, a, "invalid or conflicting data attribute")),
        }
    }
    Ok(properties)
}

pub(crate) fn check(
    unit: &ResolvedSourceFile<'_>,
    node: usize,
    catalog: &DeclarationCatalog<'_>,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let id = unit.declaration(node)?;
    let properties = catalog.types.data_properties.get(id);
    if let Some(target) = catalog.types.aliases.get(id) {
        let target = catalog
            .types
            .normalize_at(target.clone(), unit, node)
            .map_err(|m| TypeError::at(unit.file, node, m))?;
        if properties.is_some_and(|p| p.multiplicity != Multiplicity::Affine && p.multiplicity != catalog.types.multiplicity(&target)) {
            return Err(TypeError::at(unit.file, node, "a transparent alias cannot change its target's multiplicity"));
        }
        output.type_alias_targets.insert(id.clone(), target);
        if let Some(properties) = properties {
            // Provenance on this alias, not a new nominal ownership identity.
            output.type_alias_properties.insert(id.clone(), properties.clone());
        }
        return Ok(());
    }
    let packaged = catalog.types.witness_count(id) != 0;
    let copy = properties.is_some_and(|p| p.multiplicity == Multiplicity::Copy);
    if packaged || copy {
        for (_, field) in catalog.stored_fields(id, None) {
            if packaged && !catalog.package_representation(&field.ty, Some(id)) {
                return Err(TypeError::for_site(&field.site, "field has witness-dependent representation"));
            }
            if copy && catalog.types.multiplicity(&field.ty) != Multiplicity::Copy {
                return Err(TypeError::for_site(&field.site, format!("copy data contains non-copy field {}", field.name())));
            }
        }
    }
    if let Some(properties) = properties {
        output.data_properties.push((id.clone(), properties.clone()));
    }
    Ok(())
}

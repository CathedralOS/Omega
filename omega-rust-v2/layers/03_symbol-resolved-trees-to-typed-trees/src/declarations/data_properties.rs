use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::{Carry, DataProperties, Multiplicity, TypeError, TypedStage};
use std::collections::HashMap;

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
            Some("carry") if properties.carry.is_none() => {
                let mut axes = HashMap::new();
                while let Some(axis) = words.next() {
                    let value = words
                        .next()
                        .ok_or_else(|| TypeError::at(f, a, "missing carry value"))?;
                    if axes.insert(axis.text, value.text).is_some() {
                        return Err(TypeError::at(f, a, "repeated carry axis"));
                    }
                }
                if axes.len() != 4 {
                    return Err(TypeError::at(f, a, "carry requires exactly four axes"));
                }
                let axis = |key, yes, no| match axes.get(key) {
                    Some(v) if *v == yes => Ok(true),
                    Some(v) if *v == no => Ok(false),
                    _ => Err(TypeError::at(f, a, format!("invalid carry axis {key}"))),
                };
                properties.carry = Some(Carry {
                    allows_suspension: axis("suspension", "allowed", "forbidden")?,
                    requires_same_cpu: axis("cpu", "same", "any")?,
                    requires_same_thread: axis("thread", "same", "any")?,
                    requires_stable_address: axis("address", "stable", "movable")?,
                });
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
        let Some(properties) = properties else { return Ok(()) };
        let target = catalog
            .types
            .normalize_at(target.clone(), unit, node)
            .map_err(|m| TypeError::at(unit.file, node, m))?;
        if properties.multiplicity != Multiplicity::Affine && properties.multiplicity != catalog.types.multiplicity(&target) {
            return Err(TypeError::at(unit.file, node, "a transparent alias cannot change its target's multiplicity"));
        }
        // Only a nominal declaration contributes a new ownership identity.
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

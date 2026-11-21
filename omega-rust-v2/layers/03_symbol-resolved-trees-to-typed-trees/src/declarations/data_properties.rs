use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::{Carry, DataProperties, Multiplicity, TypeError, TypedStage, ValueType};
use omega_resolved::DeclarationId;
use omega_syntax::NodeKind;
use std::collections::HashMap;

/// A value with no live part demands nothing on any axis.
const PORTABLE: Carry = Carry {
    allows_suspension: true,
    requires_same_cpu: false,
    requires_same_thread: false,
    requires_stable_address: false,
};

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
        let target = catalog
            .types
            .normalize_at(target.clone(), unit, node)
            .map_err(|m| TypeError::at(unit.file, node, m))?;
        if properties.is_some_and(|p| p.multiplicity != Multiplicity::Affine && p.multiplicity != catalog.types.multiplicity(&target)) {
            return Err(TypeError::at(unit.file, node, "a transparent alias cannot change its target's multiplicity"));
        }
        // The alias is its expansion, so a demand the expansion lacks would change
        // that type rather than assert something about it. Undecided targets keep
        // the authored axes for lowering, which refuses them.
        if let Some(authored) = properties.and_then(|p| p.carry.as_ref())
            && derived_carry(catalog, &target, &mut Vec::new()).is_some_and(|derived| adds_demand(authored, &derived))
        {
            let attribute = carry_attribute(unit, node).unwrap_or(node);
            return Err(TypeError::at(unit.file, attribute, "a transparent alias cannot change its target's carry"));
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

/// The carry policy that the declarations fix for every value of a normalized
/// type. Aggregates and closed nominal data take the most restrictive demand of
/// their parts and authored floor (`wiki/spec/resources/carry.md`); a sum
/// joins every case, which bounds each active payload. References, callables,
/// qualifications, binders, open applications and data without visible parts
/// return `None`: their policy depends on facts this query does not judge.
fn derived_carry(
    catalog: &DeclarationCatalog<'_>,
    ty: &ValueType,
    open: &mut Vec<DeclarationId>,
) -> Option<Carry> {
    match ty {
        ValueType::Bool | ValueType::Integer { .. } | ValueType::IndexedInteger { .. } | ValueType::Float32 | ValueType::Float64 => Some(PORTABLE),
        ValueType::Array(element, _) | ValueType::IndexedArray(element, _) => derived_carry(catalog, element, open),
        ValueType::Tuple(parts) => parts
            .iter()
            .try_fold(PORTABLE, |policy, part| Some(join(&policy, &derived_carry(catalog, part, open)?))),
        ValueType::Nominal(id) => {
            let id: &DeclarationId = id;
            let closed = catalog
                .types
                .generics
                .get(id)
                .is_none_or(|g| g.parameters.is_empty() && g.witness_count == 0);
            let visible = catalog.fields.contains_key(id) || catalog.cases.contains_key(id);
            if !closed || !visible || catalog.types.aliases.contains_key(id) || open.contains(id) {
                return None;
            }
            open.push(id.clone());
            let floor = catalog
                .types
                .data_properties
                .get(id)
                .and_then(|p| p.carry.clone())
                .unwrap_or(PORTABLE);
            let policy = catalog
                .stored_fields(id, None)
                .try_fold(floor, |policy, (_, field)| Some(join(&policy, &derived_carry(catalog, &field.ty, open)?)));
            open.pop();
            policy
        }
        _ => None,
    }
}

fn join(
    a: &Carry,
    b: &Carry,
) -> Carry {
    Carry {
        allows_suspension: a.allows_suspension && b.allows_suspension,
        requires_same_cpu: a.requires_same_cpu || b.requires_same_cpu,
        requires_same_thread: a.requires_same_thread || b.requires_same_thread,
        requires_stable_address: a.requires_stable_address || b.requires_stable_address,
    }
}

/// True when `authored` restricts an axis that `derived` leaves unrestricted.
fn adds_demand(
    authored: &Carry,
    derived: &Carry,
) -> bool {
    join(authored, derived) != *derived
}

fn carry_attribute(
    unit: &ResolvedSourceFile<'_>,
    node: usize,
) -> Option<usize> {
    let file = unit.file;
    let attributes = file.child(node, NodeKind::Attributes)?;
    file.nodes[attributes]
        .children
        .iter()
        .copied()
        .find(|&a| file.nodes[a].words().next().is_some_and(|w| w.text == "carry"))
}

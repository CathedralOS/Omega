use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::{Carry, DataProperties, Multiplicity, TypeError};
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

//! Storage demands of a canonical declaration ABI, without Source or execution.
//!
//! Owned values and inline private callback backings need extents. Ordinary
//! pointer transport does not demand the layout of its referent. Target names
//! select retained layout descriptions, never a provider or executable body.

use semantic_vocabulary::{CallingSignatureId, ScalarApplicationId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralTypeShape, TerminalModule};

pub fn calling_abi_extent_dependencies(
    module: &TerminalModule,
    declaration: CallingSignatureId,
    target: &str,
) -> Result<Vec<ScalarApplicationId>, String> {
    let storage = module
        .calling_abi_signature(declaration)
        .ok_or("calling extent query lost its declaration")?
        .storage
        .as_ref()
        .map_err(Clone::clone)?;
    let mut roots = Vec::new();
    roots.extend(storage.result);
    for parameter in &storage.parameters {
        if parameter.access == StructuralAccess::Owned {
            roots.push(parameter.structural_type);
        }
        if let Some(backing) = parameter.callback_backing
            && callback_storage_has_slots(module, target, backing)?
        {
            roots.push(backing);
        }
    }
    let declarations = module
        .structural_types
        .iter()
        .map(|row| (row.id, row))
        .collect();
    crate::inline_storage_extent_dependencies(roots, &declarations)
        .map(|applications| applications.into_iter().collect())
}

pub fn callback_storage_has_slots(
    module: &TerminalModule,
    target: &str,
    record: StructuralTypeId,
) -> Result<bool, String> {
    storage_has_slots(module, target, record, &mut Vec::new())
}

fn storage_has_slots(
    module: &TerminalModule,
    target: &str,
    record: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
) -> Result<bool, String> {
    if active.len() >= 128 || active.contains(&record) {
        return Err("canonical callback storage has a recursive inline record path".into());
    }
    let declaration = module
        .structural_type(record)
        .ok_or("callback backing root has no retained storage declaration")?;
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Ok(false);
    };
    let native = module
        .native_layouts
        .record(record)
        .ok_or("callback backing record lost its native layout association")?;
    if module
        .native_layouts
        .for_target(native, target)?
        .is_some_and(|row| !row.report.private_callback_demands.is_empty())
    {
        return Ok(true);
    }
    active.push(record);
    let result = (|| {
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            if let Some(child) = field.field_type.as_structural() {
                let child_declaration = module
                    .structural_type(child)
                    .ok_or("callback inline field has no retained storage declaration")?;
                if matches!(child_declaration.shape, StructuralTypeShape::Record { .. })
                    && storage_has_slots(module, target, child, active)?
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    })();
    active.pop();
    result
}

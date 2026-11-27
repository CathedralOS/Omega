use semantic_vocabulary::{CallingSignatureId, ScalarApplicationId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralTypeShape, TerminalModule};

pub use terminal_psi::layouts::record_storage::{foreign_opaque_carrier, foreign_record_storage};

pub(crate) fn foreign_storage_extent_dependencies(
    module: &TerminalModule,
    roots: impl IntoIterator<Item = StructuralTypeId>,
    target: &str,
) -> Result<Vec<ScalarApplicationId>, String> {
    fn visit(
        module: &TerminalModule,
        subject: StructuralTypeId,
        target: &str,
        active: &mut Vec<StructuralTypeId>,
        closed: &mut Vec<StructuralTypeId>,
        applications: &mut Vec<ScalarApplicationId>,
    ) -> Result<(), String> {
        if closed.contains(&subject) {
            return Ok(());
        }
        if active.contains(&subject) {
            return Err("recursive by-value Terminal data has no finite boundary shape".into());
        }
        let declaration = module
            .structural_type(subject)
            .ok_or("boundary value has no canonical storage declaration")?;
        active.push(subject);
        let mut children = Vec::new();
        if let Some((_, selected)) = foreign_opaque_carrier(module, declaration)? {
            children.push(selected.carrier);
        } else {
            match &declaration.shape {
                StructuralTypeShape::TypeParameter => {
                    return Err("a boundary value requires a specialized storage type".into());
                }
                StructuralTypeShape::Opaque => {
                    return Err(
                        "opaque boundary value has no selected storage representation".into(),
                    );
                }
                StructuralTypeShape::PrimitiveScalar(_)
                | StructuralTypeShape::AtomicCell(_)
                | StructuralTypeShape::Reference { .. }
                | StructuralTypeShape::ByteSequence(_)
                | StructuralTypeShape::ElementView { .. } => {}
                StructuralTypeShape::FixedArray { element, length } => {
                    if let terminal_psi::ArrayExtent::Application(application) = length {
                        applications.push(*application);
                    }
                    children.push(*element);
                }
                StructuralTypeShape::Record { fields, storage } => {
                    let selected = foreign_record_storage(module, subject, *storage, target)?;
                    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                        let storage = selected.field(field)?;
                        if storage
                            .as_ref()
                            .and_then(|storage| storage.stored_scalar())
                            .is_none()
                            && let terminal_psi::StructuralFieldType::Structural(child) =
                                field.field_type
                        {
                            children.push(child);
                        }
                    }
                }
                StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. } => {
                    return Err("case data is not yet classifiable as a boundary value; pass it by reference".into());
                }
            }
        }
        for child in children {
            visit(module, child, target, active, closed, applications)?;
        }
        active.pop();
        closed.push(subject);
        Ok(())
    }
    let mut applications = Vec::new();
    let mut active = Vec::new();
    let mut closed = Vec::new();
    for root in roots {
        visit(
            module,
            root,
            target,
            &mut active,
            &mut closed,
            &mut applications,
        )?;
    }
    applications.sort_unstable();
    applications.dedup();
    Ok(applications)
}

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
    foreign_storage_extent_dependencies(module, roots, target)
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
        .layouts.native_layouts
        .record(record)
        .ok_or("callback backing record lost its native layout association")?;
    if module
        .layouts.native_layouts
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

#[cfg(test)]
mod tests;

//! A handback checks availability of the exact lenders of an erased field.
//! A complete local origin carries an address even when the public binder
//! hides its type. Check each address against the original typed initializer.
use super::*;
use crate::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPosition, descriptor_location, descriptor_path,
};
use crate::descriptor_results::{DescriptorAnalysis, DescriptorCallOrigin};
use semantic_vocabulary::PlaceId;
use std::collections::BTreeSet;

pub(crate) fn local_mutable_field_homes(
    module: &TerminalModule,
    machine: &TerminalMachine,
    at: DescriptorPosition,
    references: &mut DescriptorAnalysis<'_>,
    source: PlaceId,
    path: &[StructuralPathSegment],
    checked: Option<&CheckedPackageSites>,
) -> Option<Vec<(PlaceId, Vec<StructuralPathSegment>)>> {
    let root = machine.structural_place_type(source)?;
    let member = fields::hidden_reference_field(module, root, path)?;
    if member.access != StructuralAccess::MutableBorrow
        || member
            .path
            .iter()
            .any(|step| step.runtime_index().is_some())
    {
        return None;
    }
    let types = &module.types.structural_types;
    let replayed;
    let checked = match checked {
        Some(checked) => checked,
        None => {
            replayed = check_package_sites(module).ok()?;
            &replayed
        }
    };
    let slot = descriptor_location(types, source, root, path, None)?;
    let homes = references.local_reference_homes(machine.id, at, &slot)?;
    let mut pending: BTreeSet<_> = homes.iter().collect();
    for block in &machine.blocks {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let OperationKind::EstablishRecord { fields } = &operation.kind else {
                continue;
            };
            if operation
                .result
                .structural()
                .is_none_or(|r| r.structural_type != member.record)
            {
                continue;
            }
            let concrete = checked
                .field_type(machine.id, operation.id, member.field)?
                .as_structural()?;
            let concrete_reference =
                crate::inline_descriptor_type(types.iter(), concrete, member.path)?;
            let arg = fields
                .iter()
                .find(|initializer| initializer.field == member.field)?
                .value
                .as_structural()?;
            let mut input_path = arg.path.clone();
            input_path.extend_from_slice(member.path);
            let input = descriptor_location(
                types,
                arg.place,
                machine.structural_place_type(arg.place)?,
                &input_path,
                None,
            )?;
            let cut = (block.id, u32::try_from(ordinal).ok()?);
            let Some(captures) = references.local_reference_homes(machine.id, cut, &input) else {
                continue;
            };
            for captured in &captures {
                if crate::reference_source_projection_compatible(
                    types.iter(),
                    machine.structural_place_type(captured.0)?,
                    &captured.1,
                    concrete_reference.id,
                ) {
                    pending.remove(captured);
                }
            }
            if pending.is_empty() {
                return Some(homes);
            }
        }
    }
    None
}

/// A conservative call effect can invalidate the hidden state reached through
/// its mutable package argument. Resolve the complete set of local lenders
/// captured by that field. Unknown captures keep the ordinary refusal.
pub fn local_call_write_origins(
    module: &TerminalModule,
    machine: &TerminalMachine,
    at: DescriptorPosition,
    references: &mut DescriptorAnalysis<'_>,
    location: &DescriptorLocation,
) -> Option<Vec<DescriptorCallOrigin>> {
    let operation = machine
        .blocks
        .iter()
        .find(|block| block.id == at.0)?
        .operations
        .get(usize::try_from(at.1).ok()?)?;
    let root = machine.structural_place_type(location.root)?;
    let path = descriptor_path(&module.types.structural_types, root, &location.path)?;
    let (StructuralPathSegment::Referent, carrier) = path.split_last()? else {
        return None;
    };
    let (_, package) = carrier.split_last()?;
    let written_argument = terminal_psi::call_structural_arguments(&operation.kind)
        .iter()
        .any(|arg| {
            arg.place == location.root
                && arg.access == StructuralAccess::MutableBorrow
                && (package.starts_with(&arg.path) || path == arg.path)
        });
    if !written_argument {
        return None;
    }
    local_mutable_field_homes(
        module,
        machine,
        at,
        references,
        location.root,
        carrier,
        None,
    )?
    .into_iter()
    .map(|(source, path)| {
        // A runtime index would need its original evaluation point to
        // identify the same cell after the call.
        (!path.iter().any(|segment| segment.runtime_index().is_some())).then(|| {
            DescriptorCallOrigin::Argument {
                source,
                projection: crate::borrowed_storage::Projection::new(&path),
            }
        })
    })
    .collect()
}

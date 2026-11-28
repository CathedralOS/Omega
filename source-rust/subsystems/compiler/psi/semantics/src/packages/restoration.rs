//! A handback checks availability of the exact lenders of an erased field.
//! A complete local origin carries an address even when the public binder
//! hides its type. Check each address against the original typed initializer.
use super::*;
use crate::descriptor_lifetimes::{DescriptorPosition, descriptor_location};
use crate::descriptor_results::DescriptorAnalysis;
use semantic_vocabulary::PlaceId;

pub(crate) fn local_restoration_homes(
    module: &TerminalModule,
    machine: &TerminalMachine,
    at: DescriptorPosition,
    references: &mut DescriptorAnalysis<'_>,
    source: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<Vec<(PlaceId, Vec<StructuralPathSegment>)>> {
    let (record, field, StructuralAccess::MutableBorrow) =
        fields::hidden_reference_field(module, machine, source, path)?
    else {
        return None;
    };
    let types = &module.types.structural_types;
    let root = machine.structural_place_type(source)?;
    let checked = check_package_sites(module).ok()?;
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
                .is_none_or(|r| r.structural_type != record)
            {
                continue;
            }
            let concrete = checked
                .field_type(machine.id, operation.id, field.id)?
                .as_structural()?;
            let arg = fields
                .iter()
                .find(|member| member.field == field.id)?
                .value
                .as_structural()?;
            let input = descriptor_location(
                types,
                arg.place,
                machine.structural_place_type(arg.place)?,
                &arg.path,
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
                    concrete,
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

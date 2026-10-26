//! Capture native scalar and discriminator geometry beside target lowering,
//! before its inputs are consumed. This is the same shape/address owner as native calls
//! and field accesses, not a second layout implementation in a runtime adapter.
//! Case-qualified paths retain each overlaid payload's declaration namespace.
//! Opaque regions retain their extent, not their bytes or permissions. An
//! adapter must exclude active opaque storage from scalar/tag replacement.

use super::structural_layout;
use abstract_operations::AbstractOperationPlan;
use runtime_abi::{NativeBorrowedStorageLayout, NativeStorageCaseTag, NativeStorageRepetition};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, ScalarType, StructuralCaseId, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

pub(super) fn capture(plan: &AbstractOperationPlan) -> Vec<NativeBorrowedStorageLayout> {
    let declarations: BTreeMap<_, _> = plan
        .structural_types
        .iter()
        .map(|row| (row.id, row))
        .collect();
    // Native value codecs need owned argument and result layouts even when no
    // unrelated boundary in this module borrows storage. Original allocation
    // geometry additionally needs the complete actual owned roots below.
    let mut requested: BTreeSet<_> = plan
        .boundary_machines
        .iter()
        .flat_map(|boundary| {
            boundary
                .structural_parameters
                .iter()
                .map(|parameter| parameter.structural_type)
                .chain(match &boundary.result {
                    terminal_psi::BoundaryMachineResult::Structural(result) => {
                        Some(result.structural_type)
                    }
                    _ => None,
                })
        })
        .collect();
    let needs_original_roots = plan
        .boundary_machines
        .iter()
        .flat_map(|boundary| &boundary.structural_parameters)
        .any(|parameter| {
            matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            )
        });
    if needs_original_roots {
        // A boundary may first borrow a nested field. Its formal supplies
        // only that field's view, never the containing allocation's extent,
        // alignment or field offsets. Retain layouts for the actual owned
        // allocations in this produced program before lowering consumes them.
        // Runtime custody still selects the original owner, and unsupported
        // layouts remain absent rather than fabricating a smaller home.
        for function in &plan.functions {
            requested.extend(
                function
                    .structural_parameters
                    .iter()
                    .chain(function.block_structural_parameters())
                    .filter(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned)
                    .map(|parameter| parameter.structural_type),
            );
            requested.extend(
                function
                    .operations
                    .iter()
                    .filter_map(|operation| operation.materialized_structural_result())
                    .map(|(_, result)| result.structural_type),
            );
        }
    }
    requested
        .into_iter()
        .filter_map(|root| capture_declared_type(root, &declarations))
        .collect()
}

pub(super) fn capture_type(
    root: StructuralTypeId,
    types: &[StructuralTypeDeclaration],
) -> Option<NativeBorrowedStorageLayout> {
    let declarations = types.iter().map(|row| (row.id, row)).collect();
    capture_declared_type(root, &declarations)
}

fn capture_declared_type(
    root: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<NativeBorrowedStorageLayout> {
    let shape = structural_layout::structural_shape(
        root,
        declarations,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
    .ok()?;
    let mut recipes = StorageRecipes {
        declarations,
        fields: Vec::new(),
        discriminators: Vec::new(),
        opaque_regions: Vec::new(),
    };
    recipes.collect(root, 0, None, &mut Vec::new(), &mut Vec::new())?;
    Some(NativeBorrowedStorageLayout::new(
        root,
        shape.byte_size,
        shape.alignment,
        recipes.fields,
        recipes.discriminators,
        recipes.opaque_regions,
    ))
}

type Field = (
    Vec<Path>,
    Vec<NativeStorageRepetition>,
    u32,
    ScalarType,
    ScalarType,
);
type Discriminator = (
    Vec<Path>,
    Vec<NativeStorageRepetition>,
    StructuralTypeId,
    u32,
    Vec<NativeStorageCaseTag>,
);
type OpaqueRegion = (Vec<Path>, Vec<NativeStorageRepetition>, u32, u32);

struct StorageRecipes<'declarations> {
    declarations:
        &'declarations BTreeMap<StructuralTypeId, &'declarations StructuralTypeDeclaration>,
    fields: Vec<Field>,
    discriminators: Vec<Discriminator>,
    opaque_regions: Vec<OpaqueRegion>,
}

impl StorageRecipes<'_> {
    fn collect(
        &mut self,
        root: StructuralTypeId,
        offset: u32,
        occurrence_stride: Option<u32>,
        path: &mut Vec<Path>,
        repetitions: &mut Vec<NativeStorageRepetition>,
    ) -> Option<()> {
        let declaration = self.declarations.get(&root).copied()?;
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => {
                self.fields
                    .push((path.clone(), repetitions.clone(), offset, *scalar, *scalar))
            }
            StructuralTypeShape::BindingCarrier
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. } => {
                let shape = structural_layout::structural_shape(
                    root,
                    self.declarations,
                    &mut BTreeMap::new(),
                    &mut BTreeSet::new(),
                )
                .ok()?;
                self.opaque_regions.push((
                    path.clone(),
                    repetitions.clone(),
                    offset,
                    shape.byte_size,
                ));
            }
            StructuralTypeShape::Record { fields, .. } => {
                self.collect_members(root, fields, None, offset, path, repetitions)?;
            }
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                // Conventional sum layout does not consume authored member
                // storage. A borrowed recipe cannot invent an independent
                // offset, integer codec or stride for those declarations.
                let common = match &declaration.shape {
                    StructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
                    _ => &[],
                };
                if common
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                    .any(|field| !field.relevance.is_erased() && field.storage.is_some())
                {
                    return None;
                }
                let layout = structural_layout::structural_sum_layout(
                    root,
                    self.declarations,
                    &mut BTreeMap::new(),
                    &mut BTreeSet::new(),
                )
                .ok()?;
                let mappings = cases
                    .iter()
                    .enumerate()
                    .map(|(ordinal, case)| {
                        Some(NativeStorageCaseTag {
                            case: case.id,
                            native_tag: i32::try_from(ordinal).ok()?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                self.discriminators.push((
                    path.clone(),
                    repetitions.clone(),
                    root,
                    offset.checked_add(layout.tag_byte_offset)?,
                    mappings,
                ));
                self.collect_members(root, common, None, offset, path, repetitions)?;
                for case in cases {
                    path.push(Path::Case(case.id));
                    self.collect_members(
                        root,
                        &case.fields,
                        Some(case.id),
                        offset,
                        path,
                        repetitions,
                    )?;
                    path.pop();
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                let (_, stride) = structural_layout::root_array_projection_metadata(
                    root,
                    self.declarations,
                    &mut BTreeMap::new(),
                    &mut BTreeSet::new(),
                )
                .ok()?;
                repetitions.push(NativeStorageRepetition {
                    path_position: path.len(),
                    length: *length,
                    byte_stride: occurrence_stride.or(stride)?,
                });
                path.push(Path::FixedIndex(0));
                self.collect(*element, offset, None, path, repetitions)?;
                path.pop();
                repetitions.pop();
            }
        }
        Some(())
    }

    fn collect_members(
        &mut self,
        root: StructuralTypeId,
        members: &[StructuralFieldDeclaration],
        case: Option<StructuralCaseId>,
        offset: u32,
        path: &mut Vec<Path>,
        repetitions: &mut Vec<NativeStorageRepetition>,
    ) -> Option<()> {
        for (position, member) in members
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .enumerate()
        {
            if matches!(member.field_type, StructuralFieldType::Erased { .. }) {
                continue;
            }
            path.push(Path::Field(member.id));
            // Atomic residents retain geometry without acquiring ordinary
            // scalar permission. Shared capture still leaves them uncovered.
            if let Some(scalar) = member.field_type.scalar_type() {
                let (displacement, _) = if let Some(case) = case {
                    structural_layout::static_field_geometry(
                        root,
                        &[StructuralPathSegment::Case(case)],
                        member.id,
                        scalar,
                        self.declarations,
                    )
                    .ok()?
                } else {
                    structural_layout::direct_scalar_field_geometry(
                        root,
                        member.id,
                        scalar,
                        self.declarations,
                    )
                    .ok()?
                };
                if matches!(
                    member.storage,
                    Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
                ) {
                    return None;
                }
                let stored = member
                    .storage
                    .as_ref()
                    .and_then(|storage| storage.stored_scalar())
                    .unwrap_or(scalar);
                self.fields.push((
                    path.clone(),
                    repetitions.clone(),
                    offset.checked_add(displacement)?,
                    scalar,
                    stored,
                ));
            } else if let Some(child) = member.field_type.as_structural() {
                let (_, _, displacement) = if let Some(case) = case {
                    structural_layout::case_projection(
                        root,
                        &[Path::Case(case), Path::Field(member.id)],
                        self.declarations,
                        &mut BTreeMap::new(),
                        &mut BTreeSet::new(),
                    )
                    .ok()?
                } else {
                    structural_layout::resolve_structural_projection_path(
                        root,
                        &[StructuralPathSegment::Field(member.identity.clone())],
                        self.declarations,
                        &mut BTreeMap::new(),
                        &mut BTreeSet::new(),
                    )
                    .ok()?
                };
                let stride = member
                    .storage
                    .as_ref()
                    .and_then(semantic_vocabulary::AuthoredFieldStorage::element_stride)
                    .map(u32::try_from)
                    .transpose()
                    .ok()?;
                self.collect(
                    child,
                    offset.checked_add(displacement)?,
                    stride,
                    path,
                    repetitions,
                )?;
            } else if let Some(carrier) = member.field_type.byte_sequence_carrier() {
                let (displacement, byte_size) =
                    self.inline_byte_geometry(root, member, position, case, carrier)?;
                self.opaque_regions.push((
                    path.clone(),
                    repetitions.clone(),
                    offset.checked_add(displacement)?,
                    byte_size,
                ));
            } else {
                // No byte-bearing member may disappear from the retained
                // geometry just because it has no portable scalar decoder.
                return None;
            }
            path.pop();
        }
        Some(())
    }

    fn inline_byte_geometry(
        &self,
        root: StructuralTypeId,
        member: &StructuralFieldDeclaration,
        position: usize,
        case: Option<StructuralCaseId>,
        carrier: terminal_psi::ByteSequenceCarrier,
    ) -> Option<(u32, u32)> {
        let declaration = self.declarations.get(&root)?;
        if let StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } =
            &declaration.shape
        {
            let layout = structural_layout::structural_sum_layout(
                root,
                self.declarations,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )
            .ok()?;
            let field = if let Some(case) = case {
                let case_position = terminal_psi::structural_case_position(cases, case)?;
                layout.cases.get(case_position)?.fields.get(position)?
            } else {
                layout.common_fields.get(position)?
            };
            return Some((field.byte_offset, field.shape.byte_size));
        }

        // Record byte fields have no standalone structural identity. Reuse
        // their native field-address owner, including authored record offsets,
        // and the same field carrier shape used by native layout.
        let member_path = [StructuralPathSegment::Field(member.identity.clone())];
        let displacement = match carrier {
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => {
                structural_layout::bounded_byte_field_geometry(
                    root,
                    &member_path,
                    self.declarations,
                    &mut BTreeMap::new(),
                    &mut BTreeSet::new(),
                )
                .ok()??
                .0
            }
            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
            | terminal_psi::ByteSequenceCarrier::BoundedView { .. } => {
                structural_layout::borrowed_view_field_offset(
                    root,
                    &member_path,
                    self.declarations,
                    &mut BTreeMap::new(),
                    &mut BTreeSet::new(),
                )
                .ok()??
            }
        };
        let shape = structural_layout::byte_sequence_shape(carrier, root).ok()?;
        Some((displacement, shape.byte_size))
    }
}

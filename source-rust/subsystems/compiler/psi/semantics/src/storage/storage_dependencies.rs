use crate::canonical_storage_dependencies;
use crate::projection_storage_dependencies;
use semantic_vocabulary::StructuralTypeId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralStorageUse, StructuralTypeDeclaration, TerminalMachine};

pub fn inline_storage_extent_dependencies(
    roots: impl IntoIterator<Item = StructuralTypeId>,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<BTreeSet<semantic_vocabulary::ScalarApplicationId>, String> {
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut applications = BTreeSet::new();
    while let Some(subject) = pending.pop() {
        if !visited.insert(subject) {
            continue;
        }
        let declaration = declarations
            .get(&subject)
            .ok_or_else(|| format!("inline storage has no declaration for {subject:?}"))?;
        if let terminal_psi::StructuralTypeShape::FixedArray { element, length } =
            declaration.shape
        {
            if let terminal_psi::ArrayExtent::Application(application) = length {
                applications.insert(application);
            }
            pending.push(element);
        }
        for field in declaration
            .shape
            .direct_fields()
            .into_iter()
            .flatten()
            .chain(
                declaration
                    .shape
                    .sum_cases()
                    .into_iter()
                    .flatten()
                    .flat_map(|case| &case.fields),
            )
            .filter(|field| !field.relevance.is_erased())
        {
            if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                pending.push(child);
            }
        }
    }
    Ok(applications)
}

/// A consumer may use fixed transport words for a particular root occurrence.
/// Its projected fields, referents and elements still need their own layouts,
/// even when the path reaches the same declared type again.
pub fn storage_operand_dependencies(
    machine: &TerminalMachine,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    storage: StructuralStorageUse<'_>,
    roots: &mut BTreeSet<StructuralTypeId>,
    include_root: impl Fn(semantic_vocabulary::PlaceId) -> bool,
) -> Result<(), String> {
    let place_type = |place| {
        machine
            .structural_place_type(place)
            .ok_or_else(|| format!("storage operand {place:?} has no declared carrier"))
    };
    match storage {
        StructuralStorageUse::Place(place) => {
            let root = place_type(place)?;
            if include_root(place) {
                roots.insert(root);
            }
        }
        StructuralStorageUse::Type(structural_type) => {
            roots.insert(structural_type);
        }
        StructuralStorageUse::Projection(projection) => {
            projection_storage_dependencies(
                declarations.values().copied(),
                place_type(projection.root)?,
                projection.path,
                roots,
                include_root(projection.root),
            )
            .ok_or_else(|| format!("malformed storage projection {projection:?}"))?;
        }
        StructuralStorageUse::Canonical { root, path } => {
            canonical_storage_dependencies(
                declarations.values().copied(),
                place_type(root)?,
                path,
                roots,
                include_root(root),
            )
            .ok_or_else(|| {
                format!("malformed declaration-ID storage projection at {root:?}: {path:?}")
            })?;
        }
        StructuralStorageUse::ObservedField { root, path, .. } => {
            let carrier = place_type(root)?;
            if include_root(root) {
                roots.insert(carrier);
            }
            let observed = match declarations.get(&carrier).map(|row| &row.shape) {
                Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) => {
                    *referent
                }
                _ => carrier,
            };
            canonical_storage_dependencies(
                declarations.values().copied(),
                observed,
                path,
                roots,
                observed != carrier || include_root(root),
            )
            .ok_or_else(|| {
                format!("malformed scalar-field carrier projection at {root:?}: {path:?}")
            })?;
        }
        StructuralStorageUse::Element { descriptor, path } => {
            let descriptor_type = projection_storage_dependencies(
                declarations.values().copied(),
                place_type(descriptor.root)?,
                descriptor.path,
                roots,
                include_root(descriptor.root),
            )
            .flatten()
            .ok_or_else(|| format!("element access has no descriptor at {descriptor:?}"))?;
            let Some(terminal_psi::StructuralTypeShape::ElementView { element, .. }) =
                declarations.get(&descriptor_type).map(|row| &row.shape)
            else {
                return Err(format!(
                    "element access carrier {descriptor_type:?} is not a view"
                ));
            };
            projection_storage_dependencies(
                declarations.values().copied(),
                *element,
                path,
                roots,
                true,
            )
            .ok_or_else(|| {
                format!(
                    "malformed element storage projection below {descriptor_type:?}: {path:?}"
                )
            })?;
        }
    }
    Ok(())
}

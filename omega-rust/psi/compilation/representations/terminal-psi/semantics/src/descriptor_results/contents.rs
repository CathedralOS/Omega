//! Typed queries for descriptor contents, separate from exact carrier addresses.
//!
//! A runtime selector denotes every possible element here. This analysis never
//! exports an activation's scalar IDs into another activation, nor grants write
//! or disjointness authority from an uncertain selection. Reads must account for
//! every selected slot; one unknown candidate makes the contents unknown.
use super::*;
use crate::descriptor_lifetimes::{
    DescriptorPathSegment, descriptor_location, descriptor_location_type, descriptor_path,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorContentsStep {
    Address(DescriptorPathSegment),
    AnyElement { element: StructuralTypeId },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorContentsQuery {
    pub root: PlaceId,
    pub steps: Vec<DescriptorContentsStep>,
}

/// Leaf-wise value transfer. Carrier custody still uses the exact flow graph;
/// these queries describe possible payload locations, not write authority.
pub struct DescriptorContentsTransfer {
    pub source: DescriptorContentsQuery,
    pub destination: DescriptorContentsQuery,
    pub removes_source: bool,
}

pub struct DescriptorContentsOperand<'a> {
    pub root: PlaceId,
    pub root_type: StructuralTypeId,
    pub path: &'a [StructuralPathSegment],
    pub field: Option<semantic_vocabulary::StructuralFieldId>,
}

pub fn descriptor_contents_transfers(
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    source: DescriptorContentsOperand<'_>,
    destination: DescriptorContentsOperand<'_>,
    value_type: StructuralTypeId,
    removes_source: bool,
) -> Option<Vec<DescriptorContentsTransfer>> {
    let slots = slot_kind.slots(types, source.root, value_type)?;
    if slots.is_empty() {
        return Some(Vec::new());
    }
    let source = descriptor_contents_query(
        types,
        source.root,
        source.root_type,
        source.path,
        source.field,
    )?;
    let destination = descriptor_contents_query(
        types,
        destination.root,
        destination.root_type,
        destination.path,
        destination.field,
    )?;
    let mut transfers = Vec::new();
    for slot in slots {
        let mut source = source.clone();
        let mut destination = destination.clone();
        let suffix = slot.path.into_iter().map(DescriptorContentsStep::Address);
        source.steps.extend(suffix.clone());
        destination.steps.extend(suffix);
        transfers.push(DescriptorContentsTransfer {
            source,
            destination,
            removes_source,
        });
    }
    Some(transfers)
}

impl DescriptorContentsQuery {
    pub fn from_location(location: &DescriptorLocation) -> Self {
        Self {
            root: location.root,
            steps: location
                .path
                .iter()
                .cloned()
                .map(DescriptorContentsStep::Address)
                .collect(),
        }
    }

    /// The exact carrier prefix observed while evaluating this query. Keeping
    /// that prefix live is conservative; it does not equate it with the result.
    pub fn observation(&self) -> DescriptorLocation {
        DescriptorLocation {
            root: self.root,
            path: self
                .steps
                .iter()
                .map_while(|step| match step {
                    DescriptorContentsStep::Address(segment) => Some(segment.clone()),
                    DescriptorContentsStep::AnyElement { .. } => None,
                })
                .collect(),
        }
    }

    /// Validate the full typed route, including inline field carriers that
    /// have no standalone structural declaration. Only collection crossings
    /// require a structural type; the final leaf uses its actual field identity.
    pub fn validate(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
    ) -> Option<()> {
        let mut tip = root_type;
        let mut path = Vec::new();
        for step in &self.steps {
            match step {
                DescriptorContentsStep::Address(segment) => path.push(segment.clone()),
                DescriptorContentsStep::AnyElement { element } => {
                    tip = descriptor_location_type(types, tip, &path)?;
                    let declaration = terminal_psi::find_structural_type(types, tip)?;
                    match declaration.shape {
                        StructuralTypeShape::FixedArray {
                            element: actual, ..
                        }
                        | StructuralTypeShape::ElementView {
                            element: actual, ..
                        } if actual == *element => {}
                        _ => return None,
                    }
                    tip = *element;
                    path.clear();
                }
            }
        }
        descriptor_path(types, tip, &path).map(|_| ())
    }

    /// Geometry consumers that cannot retain symbolic contents get an indirect
    /// enclosing region, never a fabricated exact payload address.
    pub fn conservative_projection(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
    ) -> Option<Projection> {
        let prefix = self.observation();
        let mut projection = Projection::new(&descriptor_path(types, root_type, &prefix.path)?);
        projection.exact = false;
        projection.indirect = true;
        Some(projection.without_captured_address())
    }
}

/// Resolve declaration-local field identities before erasing runtime selectors.
/// The executable operation still owns its bounds, case and access obligations.
pub fn descriptor_contents_query(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    final_field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<DescriptorContentsQuery> {
    let mut query = DescriptorContentsQuery {
        root,
        steps: Vec::new(),
    };
    let mut tip = root_type;
    let mut static_path = Vec::new();
    for segment in path {
        if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
            let prefix = descriptor_location(types, root, tip, &static_path, None)?;
            tip = descriptor_location_type(types, tip, &prefix.path)?;
            query
                .steps
                .extend(prefix.path.into_iter().map(DescriptorContentsStep::Address));
            let declaration = terminal_psi::find_structural_type(types, tip)?;
            let element = match declaration.shape {
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element, .. } => element,
                _ => return None,
            };
            query
                .steps
                .push(DescriptorContentsStep::AnyElement { element });
            tip = element;
            static_path.clear();
        } else {
            static_path.push(segment.clone());
        }
    }
    let suffix = descriptor_location(types, root, tip, &static_path, final_field)?;
    query
        .steps
        .extend(suffix.path.into_iter().map(DescriptorContentsStep::Address));
    query.validate(types, root_type)?;
    Some(query)
}

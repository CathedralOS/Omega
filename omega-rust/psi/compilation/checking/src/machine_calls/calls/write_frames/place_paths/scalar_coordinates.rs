//! Exact scalar storage survives coarse frame labels.
//!
//! Reading a scalar needs value-preserving interpretation; invalidating it
//! after a store needs only the actual byte coordinate. Both consumers walk
//! the same captured recast chain and normalized leaf geometry. No spelling,
//! current selector value or partial possible-origin set supplies precision.

use crate::CanonicalPlace;
use crate::LocalWriteSource;
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// One scalar observation and its actual storage coordinate. Equal geometry
/// alone permits invalidation, not transfer of a numerical guarantee.
pub struct ScalarStorageCoordinate {
    pub place: CanonicalPlace,
    pub source_primitive: PrimitiveType,
    pub view_primitive: PrimitiveType,
    pub preserves_scalar_value: bool,
    pub preserves_primitive_bits: bool,
}

/// A performed destination in its original declaration namespace. A declared
/// reference view preserves selectors without requiring physical leaf geometry;
/// an alternate representation still needs the exact scalar translation.
pub(crate) struct StorageCoordinate {
    pub place: CanonicalPlace,
    pub preserves_scalar_value: bool,
    pub preserves_primitive_bits: bool,
}

impl ScalarStorageCoordinate {
    /// Legacy bound consumers use labels only after resolving exact symbols
    /// and fixed selectors. Runtime selectors cannot become persistent facts.
    pub fn label(&self, program: &dyn crate::CheckingInput) -> Option<String> {
        let root = self.place.root.as_symbol()?;
        if !root.is_valid() {
            return None;
        }
        let mut label = program.symbols.name(root).to_owned();
        for segment in &self.place.segments {
            match segment {
                PlaceSegment::Field { symbol } if symbol.is_valid() => {
                    label.push('.');
                    label.push_str(program.symbols.name(*symbol));
                }
                PlaceSegment::FixedIndex { index } => {
                    use std::fmt::Write;
                    write!(&mut label, "[{index}]").ok()?;
                }
                _ => return None,
            }
        }
        Some(label)
    }
}

impl LocalWriteSource {
    /// Project one captured scalar source through every retained recast. An
    /// unresolved route withholds precision; callers retain their opaque or
    /// coarse write answer rather than treating absence as purity.
    pub fn scalar_storage_coordinate(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
    ) -> Option<ScalarStorageCoordinate> {
        let (view, path) = match &self.representation {
            Some((carrier, path)) => (*carrier, path.as_slice()),
            None => (
                crate::state_root_type(program, state, self.root)?,
                self.segments.as_slice(),
            ),
        };
        let view_primitive = scalar_primitive(program, view, path)?;
        let coordinate = self.storage_coordinate(program, state)?;
        let root = coordinate.place.root.as_symbol()?;
        let carrier = crate::state_root_type(program, state, root)?;
        let source_primitive = scalar_primitive(program, carrier, &coordinate.place.segments)?;
        Some(ScalarStorageCoordinate {
            place: coordinate.place,
            source_primitive,
            view_primitive,
            preserves_scalar_value: coordinate.preserves_scalar_value,
            preserves_primitive_bits: coordinate.preserves_primitive_bits,
        })
    }

    /// Keep declaration-only views in the lender's original field namespace.
    /// This identifies a destination; it grants no read, qualification, lifetime,
    /// or permission to perform the write.
    pub(crate) fn storage_coordinate(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
    ) -> Option<StorageCoordinate> {
        if !self.root.is_valid() {
            return None;
        }
        let mut root = self.root;
        let (mut carrier, mut path) = match &self.representation {
            Some((carrier, path)) => (*carrier, path.clone()),
            None => (
                crate::state_root_type(program, state, root)?,
                self.segments.clone(),
            ),
        };
        let mut preserves_scalar_value = true;
        let mut preserves_primitive_bits = true;
        for recast in self.recasts.iter().rev() {
            // A accessor is an operation capability, not an ordinary
            // scalar coordinate. Failed precision retains the backing footprint.
            let formation =
                crate::reference_formation_source(program, recast.formation.expression())?;
            if formation.interpretation.is_access_view() {
                return None;
            }
            if carrier != recast.target_type || formation.view_type != recast.target_type {
                return None;
            }
            let (previous_carrier, mut previous_path) = match &recast.representation {
                Some((carrier, path)) => (*carrier, path.clone()),
                None => (
                    crate::state_root_type(program, state, recast.root)?,
                    recast.segments.clone(),
                ),
            };
            let previous_type = crate::project_type_reference_from_segments(
                program,
                previous_carrier,
                &previous_path,
            )?;
            let previous_storage =
                crate::value_custody::places::assignment_value_type(program, previous_type);
            if crate::reference_view_storage(program, previous_storage, carrier)?
                == checked_trees::CheckedReferenceStorage::Declared
            {
                // Equality is the existing complete stored declaration identity,
                // not nominal spelling or equal byte size. Original constraints
                // remain on the lender and are checked at the receiving store.
                previous_path.extend(path);
            } else {
                let translated = crate::value_custody::recasts::recast_scalar_source_leaf(
                    program,
                    previous_type,
                    carrier,
                    &path,
                )?;
                preserves_scalar_value &= translated.preserves_scalar_value;
                preserves_primitive_bits &= translated.preserves_primitive_bits;
                previous_path.extend(translated.path.into_iter().map(|segment| match segment {
                    crate::RecastLeafSegment::Field(symbol) => PlaceSegment::Field { symbol },
                    crate::RecastLeafSegment::Element(index) => PlaceSegment::FixedIndex { index },
                }));
            }
            root = recast.root;
            carrier = previous_carrier;
            path = previous_path;
        }
        let coordinate = StorageCoordinate {
            place: CanonicalPlace {
                root: PlaceRoot::Symbol(root),
                segments: path,
            },
            preserves_scalar_value,
            preserves_primitive_bits,
        };
        Some(coordinate)
    }
}

fn scalar_primitive(
    program: &dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<PrimitiveType> {
    let leaf = crate::project_type_reference_from_segments(program, carrier, path)?;
    program.primitive_type_reference(crate::value_custody::places::assignment_value_type(
        program, leaf,
    ))
}

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
    /// Logical leaf before recasts translate it to physical storage. Its full
    /// carrier and arithmetic policy survive reference crossings; an owned-only
    /// second projection would discard that completed scalar interpretation.
    pub view_reference: TypeReferenceHandle,
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
    /// An enclosing footprint can invalidate this place without replacing its
    /// whole value. It cannot establish a missing field or its declared domain.
    pub replaces_place: bool,
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
    /// Invalidation consumes the complete captured write footprint even when
    /// an observation cannot identify one physical scalar cell. The frame's
    /// original enclosure supplies that footprint, never scalar equality or
    /// whole-field establishment. Unknown origins and descriptor protocols
    /// remain unavailable rather than becoming empty writes.
    pub(crate) fn write_storage_coordinate(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
    ) -> Result<StorageCoordinate, &'static str> {
        let reason = match self.storage_coordinate(program, state) {
            Ok(coordinate) => return Ok(coordinate),
            Err(reason) => reason,
        };
        if self.invocation_output.is_some()
            || self.invocation_result.is_some()
            || self.is_image_storage
        {
            return Err(reason);
        }
        let first = self.recasts.first().ok_or(reason)?;
        if first.representation.is_some() {
            return Err("original write enclosure retains an unresolved representation");
        }
        let (carrier, path) = self.representation.as_ref().ok_or(reason)?;
        let mut carrier = *carrier;
        crate::project_type_reference_from_segments(program, carrier, path)
            .ok_or("logical write projection has no declared type")?;
        // A coarse footprint cannot conceal a missing formation, mismatched
        // carrier, or invalid predecessor path anywhere in the retained chain.
        for recast in self.recasts.iter().rev() {
            let (previous_carrier, previous_path) =
                retained_view_predecessor(program, state, recast, carrier)?;
            let prefix_length = previous_path.len()
                - usize::from(matches!(
                    previous_path.last(),
                    Some(PlaceSegment::FixedRange { .. })
                ));
            crate::project_type_reference_from_segments(
                program,
                previous_carrier,
                &previous_path[..prefix_length],
            )
            .ok_or("retained write predecessor has no declared projection")?;
            carrier = previous_carrier;
        }
        let place = super::write_coordinates::captured_write_scope(first)
            .ok_or("write enclosure has no original declaration")?;
        let root_type = source_root_type(program, state, first.root)
            .ok_or("write enclosure has no declared root type")?;
        crate::project_type_reference_from_segments(program, root_type, &place.segments)
            .ok_or("write enclosure has no declared projection")?;
        Ok(StorageCoordinate {
            place,
            preserves_scalar_value: false,
            preserves_primitive_bits: false,
            replaces_place: false,
        })
    }

    /// Project one captured scalar source through every retained recast. An
    /// unresolved route withholds precision; callers retain their opaque or
    /// coarse write answer rather than treating absence as purity.
    pub fn scalar_storage_coordinate(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
    ) -> Option<ScalarStorageCoordinate> {
        self.scalar_storage_coordinate_result(program, state).ok()
    }

    pub(crate) fn scalar_storage_coordinate_result(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
    ) -> Result<ScalarStorageCoordinate, &'static str> {
        let (view, path) = match &self.representation {
            Some((carrier, path)) => (*carrier, path.as_slice()),
            None => (
                source_root_type(program, state, self.root)
                    .ok_or("source root has no exact declaration type")?,
                self.segments.as_slice(),
            ),
        };
        let (view_reference, view_primitive) = scalar_leaf(program, view, path)
            .ok_or("logical view path has no declared scalar type")?;
        let coordinate = self.storage_coordinate(program, state)?;
        let root = coordinate
            .place
            .root
            .as_symbol()
            .ok_or("translated source has no declaration root")?;
        let carrier = source_root_type(program, state, root)
            .ok_or("translated source root has no declared type")?;
        let (_, source_primitive) = scalar_leaf(program, carrier, &coordinate.place.segments)
            .ok_or("translated source path has no declared scalar type")?;
        Ok(ScalarStorageCoordinate {
            place: coordinate.place,
            view_reference,
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
    ) -> Result<StorageCoordinate, &'static str> {
        let (mut root, mut carrier, mut path) = if let Some(home) = self.invocation_result {
            // A completed result's opaque referent has no declaration root:
            // its producing call site is the root identity, and the declared
            // referee supplies the coordinates beneath it.
            if !self.recasts.is_empty() {
                return Err("opaque invocation result carries a retained view");
            }
            (
                PlaceRoot::Expression(home.call),
                home.referent,
                self.segments.clone(),
            )
        } else {
            if !self.root.is_valid() {
                return Err("source storage has no original declaration");
            }
            let root = self.root;
            let (carrier, path) = match &self.representation {
                Some((carrier, path)) => (*carrier, path.clone()),
                None => (
                    source_root_type(program, state, root)
                        .ok_or("source storage root has no declared type")?,
                    self.segments.clone(),
                ),
            };
            (PlaceRoot::Symbol(root), carrier, path)
        };
        let mut preserves_scalar_value = true;
        let mut preserves_primitive_bits = true;
        let mut replaces_place = true;
        for recast in self.recasts.iter().rev() {
            let (previous_carrier, mut previous_path) =
                retained_view_predecessor(program, state, recast, carrier)?;
            let range = match previous_path.last() {
                Some(PlaceSegment::FixedRange { start, end }) => Some((*start, *end)),
                _ => None,
            };
            let prefix_length = previous_path
                .len()
                .checked_sub(usize::from(range.is_some()))
                .ok_or("recast source window has no prefix")?;
            let previous_type = crate::project_type_reference_from_segments(
                program,
                previous_carrier,
                &previous_path[..prefix_length],
            )
            .ok_or("retained view predecessor has no exact declared projection")?;
            let previous_storage =
                crate::value_custody::places::assignment_value_type(program, previous_type);
            if range.is_none()
                && crate::reference_view_storage(program, previous_storage, carrier)
                    .ok_or("retained view storage classification is unavailable")?
                    == checked_trees::CheckedReferenceStorage::Declared
            {
                // Equality is the existing complete stored declaration identity,
                // not nominal spelling or equal byte size. Original constraints
                // remain on the lender and are checked at the receiving store.
                previous_path.extend(path);
            } else {
                let translated = match range {
                    Some((start, end)) => {
                        crate::value_custody::recasts::recast_scalar_source_leaf_in_array_region(
                            program,
                            previous_storage,
                            start,
                            end,
                            carrier,
                            &path,
                        )
                    }
                    None => crate::value_custody::recasts::recast_scalar_source_leaf(
                        program,
                        previous_type,
                        carrier,
                        &path,
                    ),
                }
                .ok_or("physical reference view has no exact scalar storage translation")?;
                preserves_scalar_value &= translated.preserves_scalar_value;
                preserves_primitive_bits &= translated.preserves_primitive_bits;
                replaces_place &= translated.covers_source;
                previous_path.truncate(prefix_length);
                previous_path.extend(translated.path.into_iter().map(|segment| match segment {
                    crate::RecastLeafSegment::Field(symbol) => PlaceSegment::Field { symbol },
                    crate::RecastLeafSegment::Element(index) => PlaceSegment::FixedIndex { index },
                }));
            }
            root = PlaceRoot::Symbol(recast.root);
            carrier = previous_carrier;
            path = previous_path;
        }
        if !root.is_valid() {
            return Err("retained view resolves to no original storage declaration");
        }
        let coordinate = StorageCoordinate {
            place: CanonicalPlace {
                root,
                segments: path,
            },
            preserves_scalar_value,
            preserves_primitive_bits,
            replaces_place,
        };
        Ok(coordinate)
    }
}

pub(in crate::machine_calls::calls::write_frames) fn retained_view_predecessor(
    program: &dyn crate::CheckingInput,
    state: &State,
    recast: &super::RecastSource,
    carrier: TypeReferenceHandle,
) -> Result<(TypeReferenceHandle, Vec<PlaceSegment>), &'static str> {
    let formation = crate::reference_formation_source(program, recast.formation.expression())
        .ok_or("retained view has no exact reference formation")?;
    if formation.interpretation.is_access_view() {
        return Err("accessor storage is not a declaration reference view");
    }
    if carrier != recast.target_type || formation.view_type != recast.target_type {
        return Err("retained view carrier does not match its exact formation target");
    }
    match &recast.representation {
        Some((carrier, path)) => Ok((*carrier, path.clone())),
        None => Ok((
            source_root_type(program, state, recast.root)
                .ok_or("retained view predecessor has no declared root type")?,
            recast.segments.clone(),
        )),
    }
}

/// A captured call frame can retain a root owned by its caller. That exact
/// declaration supplies the carrier; the receiving state's parameters cannot
/// reinterpret it. This imports no caller valuation, premise or access.
fn source_root_type(
    program: &dyn crate::CheckingInput,
    state: &State,
    root: symbols::SymbolHandle,
) -> Option<TypeReferenceHandle> {
    crate::state_root_type(program, state, root)
        .or_else(|| crate::value_custody::places::declaration_type_reference(program, root))
}

fn scalar_leaf(
    program: &dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<(TypeReferenceHandle, PrimitiveType)> {
    let leaf = crate::project_type_reference_from_segments(program, carrier, path)?;
    let reference = crate::value_custody::places::assignment_value_type(program, leaf);
    Some((reference, program.primitive_type_reference(reference)?))
}

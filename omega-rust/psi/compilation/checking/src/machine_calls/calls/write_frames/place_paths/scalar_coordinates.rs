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
        self.scalar_storage_coordinate_result(program, state).ok()
    }

    /// A performed store must explain unavailable interpretation rather than
    /// silently dropping its destination. Other precision queries may stay coarse.
    pub(crate) fn scalar_storage_coordinate_result(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
    ) -> Result<ScalarStorageCoordinate, &'static str> {
        if !self.root.is_valid() {
            return Err("captured source has no declaration root");
        }
        let mut root = self.root;
        let (mut carrier, mut path) = match &self.representation {
            Some((carrier, path)) => (*carrier, path.clone()),
            None => (
                source_root_type(program, state, root)
                    .ok_or("source root has no exact declaration type")?,
                self.segments.clone(),
            ),
        };
        let view_primitive = scalar_primitive(program, carrier, &path)
            .ok_or("logical view path has no declared scalar type")?;
        let mut preserves_scalar_value = true;
        let mut preserves_primitive_bits = true;
        for recast in self.recasts.iter().rev() {
            // A accessor is an operation capability, not an ordinary
            // scalar coordinate. Failed precision retains the backing footprint.
            if crate::reference_formation_source(program, recast.formation.expression())
                .ok_or("retained recast has no complete authored formation")?
                .interpretation
                .is_access_view()
            {
                return Err("access capability is not an ordinary scalar interpretation");
            }
            if carrier != recast.target_type {
                return Err("logical carrier differs from the retained recast target");
            }
            let (previous_carrier, mut previous_path) = match &recast.representation {
                Some((carrier, path)) => (*carrier, path.clone()),
                None => (
                    source_root_type(program, state, recast.root)
                        .ok_or("recast source root has no exact declaration type")?,
                    recast.segments.clone(),
                ),
            };
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
            .ok_or("recast source prefix has no declared type")?;
            let translated = match range {
                Some((start, end)) => {
                    crate::value_custody::recasts::recast_scalar_source_leaf_in_array_region(
                        program,
                        crate::value_custody::places::assignment_value_type(program, previous_type),
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
            .ok_or("logical scalar has no unique containing source cell")?;
            preserves_scalar_value &= translated.preserves_scalar_value;
            preserves_primitive_bits &= translated.preserves_primitive_bits;
            previous_path.truncate(prefix_length);
            previous_path.extend(translated.path.into_iter().map(|segment| match segment {
                crate::RecastLeafSegment::Field(symbol) => PlaceSegment::Field { symbol },
                crate::RecastLeafSegment::Element(index) => PlaceSegment::FixedIndex { index },
            }));
            root = recast.root;
            carrier = previous_carrier;
            path = previous_path;
        }
        let source_primitive = scalar_primitive(program, carrier, &path)
            .ok_or("translated source path has no declared scalar type")?;
        let coordinate = ScalarStorageCoordinate {
            place: CanonicalPlace {
                root: PlaceRoot::Symbol(root),
                segments: path,
            },
            source_primitive,
            view_primitive,
            preserves_scalar_value,
            preserves_primitive_bits,
        };
        // Presentation labels serve legacy bound/write consumers. Typed readers
        // keep case selectors and freeze runtime indices themselves.
        Ok(coordinate)
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

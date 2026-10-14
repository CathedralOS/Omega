//! Exact scalar storage survives coarse frame labels.
//!
//! Reading a scalar needs value-preserving interpretation; invalidating it
//! after a store needs only the actual byte coordinate. Both consumers walk
//! the same captured recast chain and normalized leaf geometry. No spelling,
//! current selector value or partial possible-origin set supplies precision.

use crate::CanonicalPlace;
use crate::LocalWriteSource;
use facts::{PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// One scalar observation and its actual storage coordinate. Equal geometry
/// alone permits invalidation, not transfer of a numerical guarantee.
pub struct ScalarStorageCoordinate {
    pub place: CanonicalPlace,
    pub source_primitive: PrimitiveType,
    pub view_primitive: PrimitiveType,
    pub preserves_scalar_value: bool,
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
        if !self.root.is_valid() {
            return None;
        }
        let mut root = self.root;
        let (mut carrier, mut path) = match &self.representation {
            Some((carrier, path)) => (*carrier, path.clone()),
            None => (root_type(program, state, root)?, self.segments.clone()),
        };
        let view_primitive = scalar_primitive(program, carrier, &path)?;
        let mut preserves_scalar_value = true;
        for recast in self.recasts.iter().rev() {
            // A placed accessor is an operation capability, not an ordinary
            // scalar coordinate. Failed precision retains the backing footprint.
            if crate::reference_formation_source(program, recast.formation_expression)?
                .interpretation
                == crate::ReferenceFormationInterpretation::Placed
            {
                return None;
            }
            if carrier != recast.target_type {
                return None;
            }
            let (previous_carrier, mut previous_path) = match &recast.representation {
                Some((carrier, path)) => (*carrier, path.clone()),
                None => (
                    root_type(program, state, recast.root)?,
                    recast.segments.clone(),
                ),
            };
            let previous_type = crate::project_type_reference_from_segments(
                program,
                previous_carrier,
                &previous_path,
            )?;
            let translated = crate::value_custody::recasts::recast_scalar_source_leaf(
                program,
                previous_type,
                carrier,
                &path,
            )?;
            preserves_scalar_value &= translated.preserves_scalar_value;
            previous_path.extend(translated.path.into_iter().map(|segment| match segment {
                crate::RecastLeafSegment::Field(symbol) => PlaceSegment::Field { symbol },
                crate::RecastLeafSegment::Element(index) => PlaceSegment::FixedIndex { index },
            }));
            root = recast.root;
            carrier = previous_carrier;
            path = previous_path;
        }
        let source_primitive = scalar_primitive(program, carrier, &path)?;
        let coordinate = ScalarStorageCoordinate {
            place: CanonicalPlace {
                root: PlaceRoot::Symbol(root),
                segments: path,
            },
            source_primitive,
            view_primitive,
            preserves_scalar_value,
        };
        coordinate.label(program)?;
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

fn root_type(
    program: &dyn crate::CheckingInput,
    state: &State,
    root: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    program
        .state_parameter_by_symbol(state, root)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .state_local_declaration(state, root)
                .map(|(_, local)| local.type_reference)
        })
}

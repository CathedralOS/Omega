//! Readable aggregate snapshots share the published source-home and selector
//! namespace with loans. Preparation binds source identities; the scheduled
//! effect evaluates selectors and copies contents without moving the source.

use super::{
    LoweredDirectExpression, LoweringError, PlaceId, PrimitiveType, ScalarBindings,
    StructuralTypeDeclaration, structural_paths, terminal_scalar_type, unsupported,
};

/// A snapshot source retains its readable home and exact selector evaluation.
/// These are private emission instructions, not new ownership evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedStructuralCopy {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<PreparedCopyPathSegment>,
    pub(crate) observe_length: bool,
    pub(crate) qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    pub(crate) projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PreparedCopyPathSegment {
    Static(terminal_psi::StructuralPathSegment),
    Index(LoweredDirectExpression),
}

impl ScalarBindings {
    /// Resolve a readable copy operand through the same published local/formal
    /// namespace as a loan. Its relative path selects contents; it never gives
    /// ownership of the original place to the copy's consumer.
    pub(crate) fn copied_place_source(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        machine: &checked_trees::machine::Machine,
        parameters: &[checked_trees::signature::StateParameter],
        types: &[StructuralTypeDeclaration],
    ) -> Result<PreparedStructuralCopy, LoweringError> {
        if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
            return unsupported("copied value requires readable source custody");
        }
        let source = self.borrowed_structural_home(argument, machine, parameters)?;
        let root_type =
            self.structural_argument_type(&source)
                .ok_or(LoweringError::Unsupported(
                    "copied value lost its exact source carrier",
                ))?;
        let mut path = source
            .path
            .into_iter()
            .map(structural_paths::PreparedStructuralPathSegment::Static)
            .collect::<Vec<_>>();
        path.extend(structural_paths::prepare_argument_path(
            &argument.path,
            root_type,
            types,
        )?);
        self.structural_copy_source(source.place, &path, types)
    }

    /// Bind selector reads now, but evaluate them only at the copy's scheduled
    /// operand position. Authored scalar coordinates and emitted SSA positions
    /// are distinct; all runtime selectors use the ordinary binding resolver.
    pub(crate) fn structural_copy_source(
        &self,
        place: PlaceId,
        path: &[structural_paths::PreparedStructuralPathSegment],
        types: &[StructuralTypeDeclaration],
    ) -> Result<PreparedStructuralCopy, LoweringError> {
        use structural_paths::PreparedStructuralPathSegment as Segment;
        use terminal_psi::{StructuralPathSegment, StructuralTypeShape};
        let indexed_root = matches!(
            path.first(),
            Some(Segment::RuntimeIndex(_) | Segment::Static(StructuralPathSegment::FixedIndex(_)))
        );
        let observe_length = if indexed_root {
            let root_type = self
                .structural_place_type(place)
                .ok_or(LoweringError::Unsupported(
                    "indexed copy has no published source type",
                ))?;
            let root = types
                .iter()
                .find(|declaration| declaration.id == root_type)
                .ok_or(LoweringError::Unsupported(
                    "indexed copy source type is absent",
                ))?;
            matches!(root.shape, StructuralTypeShape::ElementView { .. })
        } else {
            false
        };
        let path = path
            .iter()
            .enumerate()
            .map(|(position, segment)| {
                let index = match segment {
                    Segment::RuntimeIndex(checked_trees::CheckedRuntimeIndex::Parameter {
                        position,
                    }) => self.runtime_index_parameter(*position)?,
                    Segment::RuntimeIndex(checked_trees::CheckedRuntimeIndex::Local {
                        position,
                    }) => self.runtime_index_local(*position)?,
                    Segment::RuntimeIndex(checked_trees::CheckedRuntimeIndex::StructuralField(
                        field,
                    )) => self.runtime_index_field(field)?,
                    Segment::RuntimeIndex(_) => {
                        return unsupported("copy selector belongs to a store destination");
                    }
                    Segment::Static(StructuralPathSegment::FixedIndex(index))
                        if observe_length && position == 0 =>
                    {
                        LoweredDirectExpression::IntegerLiteral {
                            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*index)),
                            scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                        }
                    }
                    Segment::Static(segment) => {
                        return Ok(PreparedCopyPathSegment::Static(segment.clone()));
                    }
                };
                Ok(PreparedCopyPathSegment::Index(index))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let contract = self
            .structural_parameters
            .iter()
            .find(|(_, parameter)| parameter.place == place)
            .map(|(_, parameter)| parameter);
        Ok(PreparedStructuralCopy {
            place,
            path,
            observe_length,
            qualifications: contract
                .map_or_else(Vec::new, |parameter| parameter.qualifications.clone()),
            projected_qualifications: contract.map_or_else(Vec::new, |parameter| {
                parameter.projected_qualifications.clone()
            }),
        })
    }
}

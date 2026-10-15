//! Readable aggregate snapshots share the published source-home and selector
//! namespace with loans. Preparation binds source identities and schedules
//! selectors in path order before the copy effect. Captured coordinates survive
//! later selector calls; copying contents never moves the source.

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
    pub(crate) length_observation: Option<LoweredDirectExpression>,
    pub(crate) qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    pub(crate) projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    pub(crate) formation_qualifications: Vec<language_semantics::SemanticDomainId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PreparedCopyPathSegment {
    Static(terminal_psi::StructuralPathSegment),
    Index(LoweredDirectExpression),
}

/// A selector participates in the enclosing operand schedule. Computations can
/// branch or call; even direct reads must be captured before later selectors.
pub(crate) enum StructuralCopySelector {
    Computation(checked_trees::CheckedScalarComputationHandle),
    Value(LoweredDirectExpression),
}

impl ScalarBindings {
    /// Resolve a readable copy operand through the same published local/formal
    /// namespace as a loan. Its relative path selects contents; it never gives
    /// ownership of the original place to the copy's consumer.
    pub(crate) fn copied_place_source(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        parameters: &[checked_trees::signature::StateParameter],
        types: &[StructuralTypeDeclaration],
        mut capture: impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            StructuralCopySelector,
        ) -> Result<LoweredDirectExpression, LoweringError>,
    ) -> Result<PreparedStructuralCopy, LoweringError> {
        if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
            return unsupported("copied value requires readable source custody");
        }
        let source = self.borrowed_structural_home(argument, parameters)?;
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
        path.extend(structural_paths::prepare_structural_argument_path(
            identities, checked, argument, root_type, types,
        )?);
        self.structural_copy_source(identities, source.place, &path, types, &mut capture)
    }

    /// Bind selector reads now, but evaluate them only at the copy's scheduled
    /// operand position. Authored scalar coordinates and emitted SSA positions
    /// are distinct; all runtime selectors use the ordinary binding resolver.
    pub(crate) fn structural_copy_source(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        place: PlaceId,
        path: &[structural_paths::PreparedStructuralPathSegment],
        types: &[StructuralTypeDeclaration],
        mut capture: impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            StructuralCopySelector,
        ) -> Result<LoweredDirectExpression, LoweringError>,
    ) -> Result<PreparedStructuralCopy, LoweringError> {
        use structural_paths::PreparedStructuralPathSegment as Segment;
        use terminal_psi::{StructuralPathSegment, StructuralTypeShape, find_structural_type};
        let (requested_extent, path) = match path.split_first() {
            Some((Segment::ViewExtent { bytes }, suffix)) => (Some(*bytes), suffix),
            _ => (None, path),
        };
        let indexed_root = matches!(
            path.first(),
            Some(
                Segment::RuntimeIndex(_)
                    | Segment::ComputedIndex(_)
                    | Segment::Static(StructuralPathSegment::FixedIndex(_))
            )
        );
        let length_observation = if indexed_root {
            let root_type = self
                .structural_place_type(place)
                .ok_or(LoweringError::Unsupported(
                    "indexed copy has no published source type",
                ))?;
            let root = find_structural_type(types, root_type).ok_or(LoweringError::Unsupported(
                "indexed copy source type is absent",
            ))?;
            let scalar_type = terminal_scalar_type(PrimitiveType::U64)?;
            match root.shape {
                StructuralTypeShape::ElementView { .. } if requested_extent != Some(true) => {
                    Some(LoweredDirectExpression::ElementViewLength {
                        source: place,
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                        scalar_type,
                    })
                }
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                ) if requested_extent != Some(false) => {
                    Some(LoweredDirectExpression::ByteSequenceLength {
                        source: place,
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                        scalar_type,
                    })
                }
                _ if requested_extent.is_some() => {
                    return unsupported("copy extent disagrees with its published descriptor");
                }
                _ => None,
            }
        } else {
            if requested_extent.is_some() {
                return unsupported("copy extent has no element selector");
            }
            None
        };
        let path = path
            .iter()
            .enumerate()
            .map(|(position, segment)| {
                let index = match segment {
                    Segment::RuntimeIndex(checked_trees::CheckedRuntimeIndex::StorageRead { symbol }) => {
                        self.runtime_index_storage(*symbol)?
                    }
                    Segment::RuntimeIndex(checked_trees::CheckedRuntimeIndex::Parameter {
                        position,
                    }) => self.runtime_index_parameter(*position)?,
                    Segment::RuntimeIndex(checked_trees::CheckedRuntimeIndex::Local {
                        position,
                    }) => self.runtime_index_local(*position)?,
                    Segment::RuntimeIndex(checked_trees::CheckedRuntimeIndex::StructuralField(
                        field,
                    )) => self.runtime_index_field(identities, field)?,
                    Segment::CapturedIndex(place) => LoweredDirectExpression::PrimitiveRead {
                        source: crate::emission::operation_emission::expressions::PrimitiveReadSource::Place(*place),
                        path: Vec::new(),
                        scalar_type: super::terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?,
                    },
                    Segment::ViewExtent { .. } => {
                        return unsupported(
                            "copied projection has no retained nested descriptor home",
                        );
                    }
                    Segment::ComputedIndex(root) => {
                        return Ok(PreparedCopyPathSegment::Index(capture(
                            identities,
                            StructuralCopySelector::Computation(*root),
                        )?));
                    }
                    Segment::RuntimeIndex(_) => {
                        return unsupported("copy selector belongs to a store destination");
                    }
                    Segment::Static(StructuralPathSegment::FixedIndex(index))
                        if length_observation.is_some() && position == 0 =>
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
                Ok(PreparedCopyPathSegment::Index(capture(
                    identities,
                    StructuralCopySelector::Value(index),
                )?))
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
            length_observation,
            formation_qualifications: Vec::new(),
            qualifications: contract
                .map_or_else(Vec::new, |parameter| parameter.qualifications.clone()),
            projected_qualifications: contract.map_or_else(Vec::new, |parameter| {
                parameter.projected_qualifications.clone()
            }),
        })
    }
}

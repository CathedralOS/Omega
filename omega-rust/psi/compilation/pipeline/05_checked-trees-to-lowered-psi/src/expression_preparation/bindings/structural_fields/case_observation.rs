//! Prepare the same selected payload carrier for pure and computed reads.
//!
//! A scalar payload can use its dispatch snapshot directly. A scalar beneath
//! a structural payload observes its original storage; only a copyable carrier
//! may be materialized as a snapshot. Both routes retain the exact selected
//! case path and live storage authority. Neither grants affine duplication.

use semantic_vocabulary::CanonicalStructuralPathSegment;

use super::{LoweringError, PlaceId, StructuralTypeDeclaration, StructuralTypeId, unsupported};
use crate::emission::operation_emission::expressions::PrimitiveReadSource;

pub(crate) struct PreparedCaseObservation {
    pub(crate) source: PrimitiveReadSource,
    pub(crate) path: Vec<CanonicalStructuralPathSegment>,
    pub(crate) captured_prefix_length: usize,
}

pub(crate) fn prepare(
    source: PlaceId,
    root_type: StructuralTypeId,
    carrier_path: &[CanonicalStructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Result<PreparedCaseObservation, LoweringError> {
    let first_case = carrier_path
        .iter()
        .position(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)));
    let Some(case_position) = first_case.filter(|position| position + 1 < carrier_path.len())
    else {
        return Ok(PreparedCaseObservation {
            source: source.into(),
            path: carrier_path.to_vec(),
            captured_prefix_length: 0,
        });
    };
    if !matches!(
        carrier_path[case_position + 1],
        CanonicalStructuralPathSegment::Field(_)
    ) {
        return unsupported("case observation requires its declared payload field");
    }
    let captured_prefix_length = case_position + 2;
    let captured_path = &carrier_path[..captured_prefix_length];
    let structural_type =
        terminal_semantics::canonical_case_leaf_type(declarations.iter(), root_type, captured_path)
            .ok_or(LoweringError::Unsupported(
                "case observation lost its exact structural payload carrier",
            ))?;
    if !terminal_semantics::is_copyable_carrier(declarations.iter(), structural_type) {
        // A primitive read may borrow this storage; it cannot duplicate an
        // affine payload or its exclusive descriptor merely to reach a scalar.
        return Ok(PreparedCaseObservation {
            source: source.into(),
            path: carrier_path.to_vec(),
            captured_prefix_length: 0,
        });
    }
    Ok(PreparedCaseObservation {
        source: PrimitiveReadSource::CaseLeaf {
            source,
            path: captured_path.to_vec(),
            structural_type,
        },
        path: carrier_path[captured_prefix_length..].to_vec(),
        captured_prefix_length,
    })
}

impl PreparedCaseObservation {
    /// Attach a payload-relative scalar observation to its prepared source.
    /// A copied carrier starts at its own root; borrowed storage keeps the full
    /// selected-case path before any relative reference or field projections.
    pub(crate) fn rebase_observation(
        self,
        root_type: StructuralTypeId,
        declarations: &[StructuralTypeDeclaration],
        observation: &mut super::LoweredDirectExpression,
    ) -> Result<(), LoweringError> {
        use super::{LoweredBooleanReturnExpression, LoweredDirectExpression};
        let observation_type = match &self.source {
            PrimitiveReadSource::Place(_) => root_type,
            PrimitiveReadSource::CaseLeaf {
                structural_type, ..
            } => *structural_type,
        };
        let runtime_prefix = terminal_semantics::canonical_structural_storage_path(
            declarations.iter(),
            observation_type,
            &self.path,
        )
        .ok_or(LoweringError::Unsupported(
            "case observation lost its declared storage prefix",
        ))?;
        match observation {
            LoweredDirectExpression::PrimitiveRead { source, path, .. } => {
                *source = self.source;
                drop(path.splice(..0, runtime_prefix));
            }
            LoweredDirectExpression::StructuralField { source, path, .. } => {
                *source = self.source;
                drop(path.splice(..0, self.path));
            }
            LoweredDirectExpression::Boolean { expression } => match expression.as_mut() {
                LoweredBooleanReturnExpression::PrimitiveRead { source, path, .. } => {
                    *source = self.source;
                    drop(path.splice(..0, runtime_prefix));
                }
                LoweredBooleanReturnExpression::StructuralField { source, path, .. } => {
                    *source = self.source;
                    drop(path.splice(..0, self.path));
                }
                _ => return unsupported("case payload did not resolve to a Boolean observation"),
            },
            _ => return unsupported("case payload did not resolve to a field observation"),
        }
        Ok(())
    }
}

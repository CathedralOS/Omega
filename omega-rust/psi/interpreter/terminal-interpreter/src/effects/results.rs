//! Host result carriers and their exact binding to a verified boundary result.

use crate::effects::TerminalEffect;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::results::{TerminalCrash, TerminalCrashSite};
use crate::values::{TerminalScalarCaseValue, TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use terminal_psi::BoundaryMachineDeclaration;
use terminal_psi::BoundaryMachineResult;
use terminal_psi::CrashCause;
use terminal_psi::OperationResult;
use terminal_psi::StructuralAffineDiscard;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::StructuralTypeShape;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEffectResult {
    /// A no-successor outcome, permitted only by the boundary's published
    /// same-cause route under this invocation's actual scalar arguments.
    Crash(CrashCause),
    Unit,
    Scalar(TerminalScalarValue),
    /// An exact selected sum case and its actual scalar payload. No opaque
    /// host allocation identity, grants, claims or qualifications are inferred.
    ScalarCase(TerminalScalarCaseValue),
    /// The same opaque, target-neutral value used for structural entry inputs.
    /// This does not provide a sum discriminator or materialized payload fields.
    Structural(TerminalStructuralValue),
}

impl TerminalExecution {
    pub(crate) fn admit_boundary_crash(
        &self,
        declaration: &BoundaryMachineDeclaration,
        effect: &TerminalEffect,
        cause: CrashCause,
    ) -> Result<TerminalCrash, TerminalInterpretError> {
        let TerminalEffect::BoundaryCall {
            operation,
            boundary,
            arguments,
            ..
        } = effect
        else {
            return Err(malformed());
        };
        if *boundary != declaration.id {
            return Err(malformed());
        }
        // Use the already evaluated invocation snapshot, not caller value IDs
        // that happen to equal the declaration's positional scalar formal IDs.
        let arguments = arguments
            .iter()
            .map(|argument| match *argument {
                TerminalScalarValue::Boolean(value) => {
                    semantic_vocabulary::ScalarTerm::Boolean(value)
                }
                TerminalScalarValue::Integer { scalar_type, value } => {
                    semantic_vocabulary::ScalarTerm::Integer { scalar_type, value }
                }
                TerminalScalarValue::IeeeFloat(value) => {
                    semantic_vocabulary::ScalarTerm::IeeeFloatLiteral(value)
                }
            })
            .collect::<Vec<_>>();
        super::crash_outcome::validate_boundary_crash_outcome(declaration, &arguments, cause)
            .map_err(|reason| TerminalInterpretError::BoundaryCrashNotPermitted {
                operation: *operation,
                boundary: *boundary,
                cause,
                reason,
            })?;
        Ok(TerminalCrash {
            site: TerminalCrashSite::BoundaryCall {
                machine: self.current_machine,
                block: self.current,
                operation: *operation,
                boundary: *boundary,
            },
            cause,
            site_guard: Vec::new(),
            frontier_lower_bound: self.live_claims.keys().copied().collect(),
        })
    }

    pub(crate) fn preflight_boundary_result(
        &self,
        result: &OperationResult,
    ) -> Result<(), TerminalInterpretError> {
        if let OperationResult::Structural(result) = result
            && ((requires_materialized_host_custody(&self.program.structural_types, result.structural_type)
                && !self.supports_scalar_case_result(result))
                || self.structural_values.contains_key(&result.place)
                || self.scalar_case_values.contains_key(&result.place)
                || self
                    .live_affine_frontier
                    .iter()
                    .any(|value| value.place == result.place)
                // A claimed result is admissible only at Linear custody: an
                // unrestricted or affine boundary result owns no claims.
                || (!result.claims.is_empty()
                    && result.multiplicity != StructuralMultiplicity::Linear)
                || !result.projected_qualifications.is_empty())
        {
            return Err(malformed());
        }
        Ok(())
    }

    fn supports_scalar_case_result(
        &self,
        result: &terminal_psi::StructuralOperationResult,
    ) -> bool {
        result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && matches!(
                result.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            )
            && self
                .program
                .structural_types
                .get(&result.structural_type)
                .is_some_and(|declaration| {
                    matches!(&declaration.shape, StructuralTypeShape::Sum { cases }
                    if cases.iter().all(|case| case.fields.iter().all(|field|
                        !field.relevance.is_erased() && field.field_type.scalar_type().is_some())))
                })
    }
}

/// Opaque host identities establish neither bounded contents nor a reference's
/// captured loan and original backing, nor a fused binding's service authority.
pub(crate) fn requires_materialized_host_custody(
    types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        // Array consumers need actual element backing, even for an empty
        // carrier. An opaque host identity cannot supply that payload; checked
        // providers instead return through the ordinary suspended call frame.
        if terminal_semantics::scalar_array_leaf_shape(types.values(), current).is_some() {
            return true;
        }
        let Some(declaration) = types.get(&current) else {
            return true;
        };
        let mut fields = Vec::new();
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } | StructuralTypeShape::FusedServiceBinding => {
                return true;
            }
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {}
            StructuralTypeShape::FixedArray { element, .. }
            | StructuralTypeShape::ElementView { element } => pending.push(*element),
            StructuralTypeShape::Record { fields: members } => fields.extend(members),
            // A discriminator and actual payload are required even when every
            // field is an unrestricted scalar. Opaque identity is not a case.
            StructuralTypeShape::Sum { .. } => return true,
            StructuralTypeShape::Mixed {
                fields: members,
                cases,
            } => {
                fields.extend(members);
                fields.extend(cases.iter().flat_map(|case| &case.fields));
            }
        }
        for field in fields {
            match field.field_type {
                terminal_psi::StructuralFieldType::BoundedInteger(_)
                | terminal_psi::StructuralFieldType::RefinedScalar(_) => return true,
                terminal_psi::StructuralFieldType::Structural(child) => pending.push(child),
                terminal_psi::StructuralFieldType::Scalar(_)
                | terminal_psi::StructuralFieldType::Atomic(_)
                | terminal_psi::StructuralFieldType::IeeeFloat(_)
                | terminal_psi::StructuralFieldType::ByteSequence(_)
                | terminal_psi::StructuralFieldType::PredicateByteSequence { .. }
                | terminal_psi::StructuralFieldType::Erased { .. } => {}
            }
        }
    }
    false
}

impl TerminalExecution {
    pub(crate) fn commit_boundary_result(
        &mut self,
        result: &OperationResult,
        expected: &BoundaryMachineResult,
        returned: TerminalEffectResult,
    ) -> Result<(), TerminalInterpretError> {
        match (result, expected, returned) {
            (OperationResult::Unit, BoundaryMachineResult::Unit, TerminalEffectResult::Unit) => {}
            (
                OperationResult::Scalar(declaration),
                BoundaryMachineResult::Scalar(expected),
                TerminalEffectResult::Scalar(value),
            ) if declaration.scalar_type == *expected && value.scalar_type() == *expected => {
                self.values.insert(declaration.id, value);
            }
            (
                OperationResult::Structural(declaration),
                BoundaryMachineResult::Structural(expected),
                TerminalEffectResult::ScalarCase(value),
            ) if declaration.structural_type == expected.structural_type
                && declaration.multiplicity == expected.multiplicity
                && declaration.qualifications == expected.qualifications
                && self.supports_scalar_case_result(declaration) =>
            {
                value.validate_against(
                    self.program
                        .structural_types
                        .get(&declaration.structural_type)
                        .ok_or_else(malformed)?,
                )?;
                self.scalar_case_values.insert(declaration.place, value);
                if declaration.multiplicity == StructuralMultiplicity::Affine {
                    self.live_affine_frontier.insert(StructuralAffineDiscard {
                        place: declaration.place,
                        path: Vec::new(),
                        structural_type: declaration.structural_type,
                    });
                }
            }
            (
                OperationResult::Structural(declaration),
                BoundaryMachineResult::Structural(expected),
                TerminalEffectResult::Structural(value),
            ) if declaration.structural_type == expected.structural_type
                && declaration.multiplicity == expected.multiplicity
                && declaration.qualifications == expected.qualifications
                && value.structural_type == expected.structural_type
                && value.qualifications == expected.qualifications
                && value.path.is_empty()
                && !requires_materialized_host_custody(
                    &self.program.structural_types,
                    declaration.structural_type,
                ) =>
            {
                self.structural_values.insert(declaration.place, value);
                if declaration.multiplicity == StructuralMultiplicity::Affine {
                    self.live_affine_frontier.insert(StructuralAffineDiscard {
                        place: declaration.place,
                        path: Vec::new(),
                        structural_type: declaration.structural_type,
                    });
                }
            }
            _ => return Err(malformed()),
        }
        Ok(())
    }
}

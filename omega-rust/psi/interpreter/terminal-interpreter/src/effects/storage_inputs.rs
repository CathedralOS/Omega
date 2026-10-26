//! Capture host storage inputs once, then validate their exact completion.
//! Owned arguments lend immutable semantic snapshots and are consumed normally;
//! borrowed arguments retain the separate staged restoration protocol.
//! Primitive loans pin their original scalar endpoint; aggregates retain a
//! semantic tree (including nominal cases) and the original whole referent.
//! Native tags and offsets never enter this owner. Replacing a case is lawful
//! under an exclusive loan, but requires complete valid new active contents,
//! not writes to the incoming case's stale leaf roster. Opaque children retain
//! their original identity or reject; host bytes cannot manufacture permissions.

mod borrowed_completion;
pub(crate) mod contents;
mod original_residence;
mod reference_outputs;

use crate::effects::scalar_loans::{BoundaryScalarLoans, TerminalBoundaryScalarBuffer};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::record::storage::{ConstructorStorage, PreparedConstructorReplacement};
use crate::values::TerminalStructuralValue;
use std::sync::Arc;
use terminal_psi::{StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration};

pub use contents::{
    TerminalBoundaryReferenceSelection, TerminalBoundaryStorageValue,
    TerminalBoundaryStructuralBuffer, TerminalStorageContents,
};

/// Actual backing determines the presentation; these are not alternative
/// execution modes. Both commit only after all invocation inputs validate.
#[derive(Debug)]
pub enum TerminalBoundaryStorageBuffer {
    Scalar(TerminalBoundaryScalarBuffer),
    Structural(TerminalBoundaryStructuralBuffer),
}

impl TerminalBoundaryStorageBuffer {
    /// Select a typed endpoint of this invocation's captured input. Opaque
    /// identity and completion validation, not a native integer, authenticate
    /// the resulting reference request for both primitive and aggregate loans.
    pub fn select_reference(
        &self,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        access: terminal_psi::StructuralAccess,
    ) -> Result<TerminalBoundaryReferenceSelection, crate::effects::TerminalEffectRejection> {
        let (identity, argument_index, available) = match self {
            Self::Scalar(buffer) => (
                buffer.capture_identity(),
                buffer.argument_index(),
                buffer.access(),
            ),
            Self::Structural(buffer) => (
                Arc::clone(&buffer.identity),
                buffer.argument_index,
                buffer.access,
            ),
        };
        if !crate::reference::access_covers(available, access)
            || !matches!(
                access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
        {
            return Err(crate::effects::TerminalEffectRejection::new(
                "reference selection exceeds captured input access",
            ));
        }
        Ok(TerminalBoundaryReferenceSelection {
            identity,
            argument_index,
            path: path.to_vec(),
            access,
        })
    }
}

struct StructuralLoanBinding {
    direct_original: bool,
    identity: Arc<()>,
    argument_index: usize,
    source: semantic_vocabulary::PlaceId,
    access: terminal_psi::StructuralAccess,
    referent: TerminalStructuralValue,
    original: Vec<TerminalBoundaryStorageValue>,
    leaves: Vec<contents::StorageLeaf>,
    completed_leaves: Option<Vec<contents::StorageLeaf>>,
    scalars: Vec<contents::PinnedScalar>,
    replacement: Option<PreparedConstructorReplacement>,
    reference_replacement: Option<ConstructorStorage>,
    stores: Vec<(
        crate::primitive_storage::PrimitiveStorage,
        semantic_vocabulary::ScalarType,
        crate::values::TerminalScalarValue,
    )>,
}

/// An owned host argument lends only a read-only payload snapshot. It is not
/// a restoration loan: completion consumes the original argument normally.
struct OwnedPayloadSnapshot {
    identity: Arc<()>,
    argument_index: usize,
    structural_type: semantic_vocabulary::StructuralTypeId,
    values: Vec<TerminalBoundaryStorageValue>,
}

#[derive(Default)]
pub(crate) struct BoundaryStorageInputs {
    pub(crate) buffers: Vec<TerminalBoundaryStorageBuffer>,
    scalars: BoundaryScalarLoans,
    structural: Vec<StructuralLoanBinding>,
    owned: Vec<OwnedPayloadSnapshot>,
    reference_inputs: Vec<CapturedReferenceInput>,
}

/// Every storage capture publishes the same private reference-selection
/// identity, independently of scalar versus aggregate snapshot representation.
struct CapturedReferenceInput {
    identity: Arc<()>,
    argument_index: usize,
    source_argument: StructuralArgument,
    referent: TerminalStructuralValue,
    access: terminal_psi::StructuralAccess,
}

impl BoundaryStorageInputs {
    pub(crate) fn is_empty(&self) -> bool {
        self.buffers.is_empty()
            && self.scalars.is_empty()
            && self.structural.is_empty()
            && self.owned.is_empty()
            && self.reference_inputs.is_empty()
    }

    pub(crate) fn capture_scalar(
        &mut self,
        execution: &TerminalExecution,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        argument_index: usize,
        identity_cursor: &mut Option<u64>,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        let referent = self.scalars.capture(
            execution,
            parameter,
            argument,
            argument_index,
            identity_cursor,
        )?;
        let buffer = self.scalars.buffers.pop().ok_or_else(malformed)?;
        self.buffers
            .push(TerminalBoundaryStorageBuffer::Scalar(buffer));
        Ok(referent)
    }

    pub(crate) fn capture_fields(
        &mut self,
        execution: &TerminalExecution,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        referent: &TerminalStructuralValue,
        array_loan: Option<&crate::element_views::ElementView>,
        argument_index: usize,
    ) -> Result<(), TerminalInterpretError> {
        if parameter.access == terminal_psi::StructuralAccess::Owned {
            if argument.access != parameter.access
                || parameter.structural_type != referent.structural_type
            {
                return Err(malformed());
            }
            let leaves = contents::describe(execution, referent, parameter.access, None)?;
            let (values, _) = contents::capture(
                execution,
                argument.place,
                referent,
                array_loan,
                parameter.access,
                &leaves,
            )?;
            let identity = Arc::new(());
            self.buffers.push(TerminalBoundaryStorageBuffer::Structural(
                TerminalBoundaryStructuralBuffer {
                    original_storage: None,
                    identity: Arc::clone(&identity),
                    argument_index,
                    access: parameter.access,
                    structural_type: referent.structural_type,
                    values: values.clone(),
                    references: Vec::new(),
                },
            ));
            self.owned.push(OwnedPayloadSnapshot {
                identity,
                argument_index,
                structural_type: referent.structural_type,
                values,
            });
            return Ok(());
        }
        if parameter.access != argument.access
            || parameter.structural_type != referent.structural_type
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || (parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                && (!parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || !referent.qualifications.is_empty()
                    || !referent.projected_qualifications.is_empty()))
        {
            return Err(malformed());
        }
        if parameter.access == terminal_psi::StructuralAccess::MutableBorrow {
            execution.validate_storage_loan_qualifications(
                &crate::values::StructuralRuntimePlace::from(referent),
            )?;
        }
        let leaves = contents::describe(execution, referent, parameter.access, None)?;
        let (original, scalars) = contents::capture(
            execution,
            argument.place,
            referent,
            array_loan,
            parameter.access,
            &leaves,
        )?;
        let identity = Arc::new(());
        self.buffers.push(TerminalBoundaryStorageBuffer::Structural(
            TerminalBoundaryStructuralBuffer {
                original_storage: None,
                identity: Arc::clone(&identity),
                argument_index,
                access: parameter.access,
                structural_type: referent.structural_type,
                values: original.clone(),
                references: Vec::new(),
            },
        ));
        self.structural.push(StructuralLoanBinding {
            direct_original: false,
            identity,
            argument_index,
            source: argument.place,
            access: parameter.access,
            referent: referent.clone(),
            original,
            leaves,
            completed_leaves: None,
            scalars,
            replacement: None,
            reference_replacement: None,
            stores: Vec::new(),
        });
        Ok(())
    }

    /// Retain reference-selection identities only for a declaration that
    /// actually publishes reference outputs. The ordinary storage capture has
    /// already established each buffer's exact input; this is a metadata join,
    /// not another storage traversal or supplier inference.
    pub(crate) fn capture_reference_inputs(
        &mut self,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        arguments: &[StructuralArgument],
        values: &[TerminalStructuralValue],
    ) -> Result<(), TerminalInterpretError> {
        if declaration.reference_outputs.is_empty() {
            return Ok(());
        }
        if !self.reference_inputs.is_empty()
            || arguments.len() != declaration.structural_parameters.len()
            || values.len() != arguments.len()
        {
            return Err(malformed());
        }
        for buffer in &self.buffers {
            let (identity, argument_index, access) = match buffer {
                TerminalBoundaryStorageBuffer::Scalar(buffer) => (
                    buffer.capture_identity(),
                    buffer.argument_index(),
                    buffer.access(),
                ),
                TerminalBoundaryStorageBuffer::Structural(buffer) => (
                    Arc::clone(&buffer.identity),
                    buffer.argument_index,
                    buffer.access,
                ),
            };
            let parameter = declaration
                .structural_parameters
                .get(argument_index)
                .ok_or_else(malformed)?;
            let argument = arguments.get(argument_index).ok_or_else(malformed)?;
            let referent = values.get(argument_index).ok_or_else(malformed)?;
            if parameter.access != access
                || argument.access != access
                || parameter.structural_type != referent.structural_type
                || self
                    .reference_inputs
                    .iter()
                    .any(|capture| capture.argument_index == argument_index)
            {
                return Err(malformed());
            }
            self.reference_inputs.push(CapturedReferenceInput {
                identity,
                argument_index,
                source_argument: argument.clone(),
                referent: referent.clone(),
                access,
            });
        }
        Ok(())
    }

    pub(crate) fn validate_writeback(
        &mut self,
        execution: &TerminalExecution,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        identity_cursor: &mut Option<u64>,
    ) -> Result<(), TerminalInterpretError> {
        let scalar_buffers: Vec<_> = self
            .buffers
            .iter()
            .filter_map(|buffer| match buffer {
                TerminalBoundaryStorageBuffer::Scalar(buffer) => Some(buffer),
                _ => None,
            })
            .collect();
        self.scalars
            .validate_writeback(execution, &scalar_buffers)?;
        let structural_buffers: Vec<_> = self
            .buffers
            .iter()
            .filter_map(|buffer| match buffer {
                TerminalBoundaryStorageBuffer::Structural(buffer)
                    if buffer.access != terminal_psi::StructuralAccess::Owned =>
                {
                    Some(buffer)
                }
                _ => None,
            })
            .collect();
        let owned_buffers: Vec<_> = self
            .buffers
            .iter()
            .filter_map(|buffer| match buffer {
                TerminalBoundaryStorageBuffer::Structural(buffer)
                    if buffer.access == terminal_psi::StructuralAccess::Owned =>
                {
                    Some(buffer)
                }
                _ => None,
            })
            .collect();
        if owned_buffers.len() != self.owned.len() {
            return Err(malformed());
        }
        for (buffer, original) in owned_buffers.into_iter().zip(&self.owned) {
            if !Arc::ptr_eq(&buffer.identity, &original.identity)
                || buffer.argument_index != original.argument_index
                || buffer.structural_type != original.structural_type
                || buffer.values != original.values
                || !buffer.references.is_empty()
            {
                return Err(malformed());
            }
        }
        if structural_buffers.len() != self.structural.len() {
            return Err(malformed());
        }
        // Ordinary scalar/case restoration validates every original snapshot
        // first. Reference outputs use those same frozen identities and retain
        // their separate lifetime contract, never native addresses as authority.
        for (buffer, binding) in structural_buffers.into_iter().zip(&mut self.structural) {
            borrowed_completion::stage(execution, buffer, binding)?;
        }
        reference_outputs::stage(
            execution,
            declaration,
            &self.buffers,
            &mut self.structural,
            &self.reference_inputs,
            identity_cursor,
        )?;
        Ok(())
    }

    pub(crate) fn commit(
        self,
        execution: &mut TerminalExecution,
    ) -> Result<(), TerminalInterpretError> {
        let mut scalars = Vec::new();
        for buffer in self.buffers {
            match buffer {
                TerminalBoundaryStorageBuffer::Scalar(buffer) => scalars.push(buffer),
                TerminalBoundaryStorageBuffer::Structural(_) => {}
            }
        }
        self.scalars.commit(execution, scalars)?;
        for binding in self.structural {
            borrowed_completion::commit(execution, binding)?;
        }
        Ok(())
    }
}

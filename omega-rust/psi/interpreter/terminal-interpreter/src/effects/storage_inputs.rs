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

use crate::effects::scalar_loans::{BoundaryScalarLoans, TerminalBoundaryScalarBuffer};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::record::storage::PreparedConstructorReplacement;
use crate::values::TerminalStructuralValue;
use std::sync::Arc;
use terminal_psi::{StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration};

pub use contents::{
    TerminalBoundaryStorageValue, TerminalBoundaryStructuralBuffer, TerminalStorageContents,
};

/// Actual backing determines the presentation; these are not alternative
/// execution modes. Both commit only after all invocation inputs validate.
#[derive(Debug)]
pub enum TerminalBoundaryStorageBuffer {
    Scalar(TerminalBoundaryScalarBuffer),
    Structural(TerminalBoundaryStructuralBuffer),
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
    scalars: Vec<contents::PinnedScalar>,
    replacement: Option<PreparedConstructorReplacement>,
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
}

impl BoundaryStorageInputs {
    pub(crate) fn is_empty(&self) -> bool {
        self.buffers.is_empty()
            && self.scalars.is_empty()
            && self.structural.is_empty()
            && self.owned.is_empty()
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
        self.buffers.push(TerminalBoundaryStorageBuffer::Scalar(
            self.scalars.buffers.pop().ok_or_else(malformed)?,
        ));
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
            scalars,
            replacement: None,
            stores: Vec::new(),
        });
        Ok(())
    }

    pub(crate) fn validate_writeback(
        &mut self,
        execution: &TerminalExecution,
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
            {
                return Err(malformed());
            }
        }
        if structural_buffers.len() != self.structural.len() {
            return Err(malformed());
        }
        for (buffer, binding) in structural_buffers.into_iter().zip(&mut self.structural) {
            borrowed_completion::stage(execution, buffer, binding)?;
        }
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

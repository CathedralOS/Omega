//! External scalar loans pin the original backing before invoking host code.
//!
//! Buffers expose payload and admitted access, not interpreter storage keys.
//! Completion validates every buffer before result or writeback publication;
//! an opaque per-buffer token prevents moving a previous invocation's buffer
//! into a new call from substituting its backing. Captured field declarations
//! retain original restrictions even when a formal observes a plain carrier.
//! `storage_loans` coordinates complete aggregates, including case replacement,
//! using the same original-endpoint and restriction queries below. It does not
//! expose aggregate leaves as independent replaceable argument buffers.

use std::sync::Arc;

use semantic_vocabulary::{CanonicalStructuralPathSegment, PlaceId, ScalarType};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeShape,
};

use crate::effects::TerminalEffectRejection;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::primitive_storage::{PrimitiveStorage, PrimitiveStorageEndpoint};
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::{
    ScalarFieldNamespace, StructuralRuntimePlace, TerminalScalarValue, TerminalStructuralValue,
};

/// One boundary argument's captured scalar payload. Replacement is staged,
/// never a grant of access to the runtime or its qualifications.
#[derive(Debug)]
pub struct TerminalBoundaryScalarBuffer {
    identity: Arc<()>,
    argument_index: usize,
    field_path: Vec<CanonicalStructuralPathSegment>,
    access: StructuralAccess,
    value: TerminalScalarValue,
    restriction: Option<StructuralFieldType>,
    stored_scalar: Option<ScalarType>,
}

impl TerminalBoundaryScalarBuffer {
    /// Index in the structural argument lane, not the scalar argument lane.
    pub fn argument_index(&self) -> usize {
        self.argument_index
    }

    /// Closed occurrence-relative coordinates; empty for a whole scalar loan.
    pub fn field_path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.field_path
    }

    pub fn access(&self) -> StructuralAccess {
        self.access
    }

    pub fn value(&self) -> TerminalScalarValue {
        self.value
    }

    /// The captured physical scalar carrier, before the formal's logical
    /// observation. Host byte counts must not expose a narrow codec's tail.
    pub fn storage_scalar_type(&self) -> ScalarType {
        self.stored_scalar.unwrap_or(self.value.scalar_type())
    }

    /// Replace only writable, exactly typed payload satisfying its original
    /// field restriction. A rejected replacement leaves the snapshot intact.
    pub fn replace(&mut self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
        if !matches!(self.access, StructuralAccess::MutableBorrow)
            || value.scalar_type() != self.value.scalar_type()
            || !terminal_scalar_belongs_to_type(value)
            || self
                .stored_scalar
                .is_some_and(|stored| !crate::primitive_storage::scalar_fits_storage(stored, value))
            || self
                .restriction
                .as_ref()
                .is_some_and(|field| !value.satisfies_field(field))
        {
            return Err(TerminalEffectRejection::new(
                "boundary scalar replacement violates access, carrier or original field restriction",
            ));
        }
        self.value = value;
        Ok(())
    }
}

struct ScalarLoanBinding {
    identity: Arc<()>,
    argument_index: usize,
    field_path: Vec<CanonicalStructuralPathSegment>,
    access: StructuralAccess,
    source: PlaceId,
    storage: PrimitiveStorage,
    scalar_type: ScalarType,
    shared_view: bool,
    original: TerminalScalarValue,
    restriction: Option<StructuralFieldType>,
}

#[derive(Default)]
pub(crate) struct BoundaryScalarLoans {
    pub(crate) buffers: Vec<TerminalBoundaryScalarBuffer>,
    bindings: Vec<ScalarLoanBinding>,
}

impl BoundaryScalarLoans {
    pub(crate) fn capture(
        &mut self,
        execution: &TerminalExecution,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        argument_index: usize,
        identity_cursor: &mut Option<u64>,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        let StructuralTypeShape::PrimitiveScalar(expected) = execution
            .program
            .structural_types()
            .get(&parameter.structural_type)
            .ok_or_else(malformed)?
            .shape
        else {
            return Err(malformed());
        };
        if parameter.access != argument.access
            || !matches!(
                parameter.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(malformed());
        }
        let loan = execution.capture_primitive_loan(parameter, argument, identity_cursor)?;
        let value = loan.referent;
        let storage = loan.storage;
        let scalar_type = loan.scalar_type;
        let shared_view = loan.shared_view;
        if scalar_type != expected {
            return Err(malformed());
        }
        self.capture_storage(
            execution,
            argument.place,
            storage,
            scalar_type,
            shared_view,
            parameter.access,
            argument_index,
            Vec::new(),
        )?;
        Ok(value)
    }

    fn capture_storage(
        &mut self,
        execution: &TerminalExecution,
        source: PlaceId,
        storage: PrimitiveStorage,
        scalar_type: ScalarType,
        shared_view: bool,
        access: StructuralAccess,
        argument_index: usize,
        field_path: Vec<CanonicalStructuralPathSegment>,
    ) -> Result<(), TerminalInterpretError> {
        execution.validate_scalar_loan_qualifications(&storage)?;
        let original = execution.read_primitive_storage(
            source,
            storage.clone(),
            scalar_type,
            scalar_type,
            shared_view,
        )?;
        let restriction = execution.captured_scalar_restriction(&storage, access)?;
        if restriction
            .as_ref()
            .is_some_and(|field| !original.satisfies_field(field))
        {
            return Err(malformed());
        }
        let identity = Arc::new(());
        self.buffers.push(TerminalBoundaryScalarBuffer {
            identity: Arc::clone(&identity),
            argument_index,
            field_path: field_path.clone(),
            access,
            value: original,
            restriction: restriction.clone(),
            stored_scalar: storage.stored_scalar,
        });
        self.bindings.push(ScalarLoanBinding {
            identity,
            argument_index,
            field_path,
            access,
            source,
            storage,
            scalar_type,
            shared_view,
            original,
            restriction,
        });
        Ok(())
    }

    pub(crate) fn validate_writeback(
        &self,
        execution: &TerminalExecution,
        buffers: &[&TerminalBoundaryScalarBuffer],
    ) -> Result<(), TerminalInterpretError> {
        if buffers.len() != self.bindings.len() {
            return Err(malformed());
        }
        for (buffer, binding) in buffers.iter().zip(&self.bindings) {
            if !Arc::ptr_eq(&buffer.identity, &binding.identity)
                || buffer.argument_index != binding.argument_index
                || buffer.field_path != binding.field_path
                || buffer.access != binding.access
                || buffer.value.scalar_type() != binding.scalar_type
                || !terminal_scalar_belongs_to_type(buffer.value)
                || buffer.stored_scalar != binding.storage.stored_scalar
                || binding.storage.stored_scalar.is_some_and(|stored| {
                    !crate::primitive_storage::scalar_fits_storage(stored, buffer.value)
                })
                || (binding.access == StructuralAccess::SharedBorrow
                    && buffer.value != binding.original)
                || binding
                    .restriction
                    .as_ref()
                    .is_some_and(|field| !buffer.value.satisfies_field(field))
            {
                return Err(malformed());
            }
            // Validate the captured backing, never resolve a source path after
            // host code. Callback code receives no mutable execution reference.
            if execution.read_primitive_storage(
                binding.source,
                binding.storage.clone(),
                binding.scalar_type,
                binding.scalar_type,
                binding.shared_view,
            )? != binding.original
            {
                return Err(malformed());
            }
        }
        Ok(())
    }

    pub(crate) fn commit(
        self,
        execution: &mut TerminalExecution,
        buffers: impl IntoIterator<Item = TerminalBoundaryScalarBuffer>,
    ) -> Result<(), TerminalInterpretError> {
        // All endpoints, payload carriers and original field restrictions were
        // validated together before result publication. Host code cannot enter
        // this execution, and a result binds a fresh SSA/place without changing
        // any captured input home. Thus these stores do not discover new source
        // paths or introduce a later, partially committed validation phase.
        for (buffer, binding) in buffers.into_iter().zip(self.bindings) {
            if binding.access != StructuralAccess::SharedBorrow {
                execution.store_primitive_storage(
                    binding.source,
                    binding.storage,
                    binding.scalar_type,
                    buffer.value,
                )?;
            }
        }
        Ok(())
    }
}

impl TerminalExecution<'_> {
    /// A plain formal cannot erase a stronger qualification on its caller's
    /// backing. This transport has no qualified replacement protocol, so it
    /// conservatively refuses qualified roots and projected owner constraints.
    pub(crate) fn validate_scalar_loan_qualifications(
        &self,
        storage: &PrimitiveStorage,
    ) -> Result<(), TerminalInterpretError> {
        let target: &StructuralRuntimePlace = match &storage.endpoint {
            PrimitiveStorageEndpoint::Scalar(place) => place,
            PrimitiveStorageEndpoint::ScalarField(field) => &field.parent,
            PrimitiveStorageEndpoint::ArrayElement { array, .. } => &array.referent,
            // The frame-home resolver already requires unqualified parameters
            // and unqualified, claim-free structural results for array locals.
            // An immutable view byte carries no replacement authority at all.
            PrimitiveStorageEndpoint::OwnedArrayElement { .. }
            | PrimitiveStorageEndpoint::ViewByte { .. } => return Ok(()),
        };
        self.validate_storage_loan_qualifications(target)
    }

    pub(crate) fn validate_storage_loan_qualifications(
        &self,
        target: &StructuralRuntimePlace,
    ) -> Result<(), TerminalInterpretError> {
        let owns_target = |value: &TerminalStructuralValue| {
            value.opaque_identity == target.opaque_identity && target.path.starts_with(&value.path)
        };
        for (machine_id, values) in std::iter::once((self.current_machine, &self.structural_values))
            .chain(
                self.call_stack
                    .iter()
                    .map(|frame| (frame.current_machine, &frame.structural_values)),
            )
        {
            let machine = self
                .program
                .machines()
                .get(&machine_id)
                .ok_or_else(malformed)?;
            for (place, value) in values.iter().filter(|(_, value)| owns_target(value)) {
                if !value.qualifications.is_empty()
                    || machine
                        .structural_parameters
                        .iter()
                        .chain(machine.block_structural_parameters())
                        .any(|parameter| {
                            parameter.place == *place
                                && (!parameter.qualifications.is_empty()
                                    || !parameter.projected_qualifications.is_empty())
                        })
                    || machine
                        .operations()
                        .filter_map(|operation| operation.result.structural())
                        .any(|result| {
                            result.place == *place
                                && (!result.qualifications.is_empty()
                                    || !result.projected_qualifications.is_empty())
                        })
                {
                    return Err(malformed());
                }
            }
        }
        if self
            .reference_referents
            .values()
            .any(|value| owns_target(&value.view) && !value.view.qualifications.is_empty())
        {
            return Err(malformed());
        }
        Ok(())
    }

    /// Recover the declaration of the actual cell, not the boundary formal.
    /// Equal opaque roots must agree on its containing type; an ambiguous
    /// owner cannot authorize host writeback under one convenient declaration.
    pub(crate) fn captured_scalar_restriction(
        &self,
        storage: &PrimitiveStorage,
        access: StructuralAccess,
    ) -> Result<Option<StructuralFieldType>, TerminalInterpretError> {
        let PrimitiveStorageEndpoint::ScalarField(cell) = &storage.endpoint else {
            return Ok(None);
        };
        let parent_type = self.actual_storage_type_from_owner(&cell.parent)?;
        let declaration = self
            .program
            .structural_types()
            .get(&parent_type)
            .ok_or_else(malformed)?;
        let fields = if cell.namespace == ScalarFieldNamespace::CasePayload {
            let case = self
                .structural_cases
                .get(&cell.parent)
                .ok_or_else(malformed)?;
            if case.structural_type != declaration.id {
                return Err(malformed());
            }
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return Err(malformed());
            };
            &terminal_psi::find_structural_case(cases, case.case)
                .ok_or_else(malformed)?
                .fields
        } else {
            match &declaration.shape {
                StructuralTypeShape::Record { fields, .. }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => return Err(malformed()),
            }
        };
        let mut matching = fields.iter().filter(|field| field.id == cell.field);
        let (Some(field), None) = (matching.next(), matching.next()) else {
            return Err(malformed());
        };
        if field.relevance.is_erased()
            || field.field_type.scalar_type().is_none()
            || (matches!(field.field_type, StructuralFieldType::Atomic(_))
                && access != StructuralAccess::MutableBorrow)
        {
            return Err(malformed());
        }
        Ok(Some(field.field_type.clone()))
    }
}

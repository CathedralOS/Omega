//! Cumulative result publication follows retained runtime owners, not declared
//! types or all storage left in an activation. Each publication has one envelope
//! cell (including Unit), concrete payload rows, and each shared byte backing
//! once. Borrowed homes do not transfer their referent's storage. Component
//! completions are publications too; inspecting a cached completion is not.

use crate::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{CompletedReturn, TerminalExecution};
use crate::provider_storage::{ProviderStorageBinding, ProviderStorageSlot, ResidentStorageSlot};
use crate::results::TerminalExecutionResult;
use crate::structural_storage::{StorageKey, cells};
use crate::values::StructuralRuntimePlace;
use evaluation_accounting::{EvaluationAccountingError, EvaluationResource};

pub(crate) struct ResultCustody<'storage> {
    cells: u64,
    payload_bytes: u64,
    byte_backings: Vec<&'storage ByteSequenceView>,
    allocations: Vec<arena::Handle<ProviderStorageSlot>>,
    residents: Vec<arena::Handle<ResidentStorageSlot>>,
}

fn count_overflow() -> TerminalInterpretError {
    TerminalInterpretError::Storage(EvaluationAccountingError::CounterOverflow(
        EvaluationResource::ResultCustody,
    ))
}

impl<'storage> ResultCustody<'storage> {
    fn new() -> Self {
        Self {
            cells: 1,
            payload_bytes: 0,
            byte_backings: Vec::new(),
            allocations: Vec::new(),
            residents: Vec::new(),
        }
    }

    pub(crate) fn add_cells(&mut self, count: usize) -> Result<(), TerminalInterpretError> {
        self.cells = self
            .cells
            .checked_add(u64::try_from(count).map_err(|_| count_overflow())?)
            .ok_or_else(count_overflow)?;
        Ok(())
    }

    pub(crate) fn rows<K: StorageKey, Value>(
        &mut self,
        rows: &impl std::borrow::Borrow<std::collections::BTreeMap<K, Value>>,
        root: &StructuralRuntimePlace,
    ) -> Result<(), TerminalInterpretError> {
        self.add_cells(cells(rows, root).count())
    }

    pub(crate) fn bytes(
        &mut self,
        view: &'storage ByteSequenceView,
    ) -> Result<(), TerminalInterpretError> {
        if !self
            .byte_backings
            .iter()
            .any(|previous| previous.shares_backing(view))
        {
            self.payload_bytes = self
                .payload_bytes
                .checked_add(
                    u64::try_from(view.retained_payload_bytes()).map_err(|_| count_overflow())?,
                )
                .ok_or_else(count_overflow)?;
            self.byte_backings.push(view);
        }
        Ok(())
    }

    pub(crate) fn binding(
        &mut self,
        binding: &'storage ByteSequenceBinding,
    ) -> Result<(), TerminalInterpretError> {
        if let ByteSequenceBinding::Immutable(view) = binding {
            self.bytes(view)?;
        }
        Ok(())
    }

    pub(crate) fn element_view(
        &mut self,
        view: &'storage crate::element_views::ElementView,
    ) -> Result<(), TerminalInterpretError> {
        if let Some(bytes) = view.retained_bytes() {
            self.bytes(bytes)?;
        }
        Ok(())
    }

    pub(crate) fn reference(
        &mut self,
        reference: &'storage crate::reference::ReferenceReferent,
    ) -> Result<(), TerminalInterpretError> {
        if let Some(binding) = &reference.byte_owner {
            self.binding(binding)?;
        }
        if let Some(view) = &reference.collection_loan {
            self.element_view(view)?;
        }
        if let Some(loan) = &reference.primitive_loan {
            self.endpoint(&loan.storage.endpoint)?;
        }
        if let Some(region) = &reference.byte_region {
            self.endpoint(&region.endpoint)?;
        }
        Ok(())
    }

    fn endpoint(
        &mut self,
        endpoint: &'storage crate::primitive_storage::PrimitiveStorageEndpoint,
    ) -> Result<(), TerminalInterpretError> {
        if let crate::primitive_storage::PrimitiveStorageEndpoint::ViewByte { bytes, .. } = endpoint
        {
            self.bytes(bytes)?;
        }
        Ok(())
    }

    pub(crate) fn provider(
        &mut self,
        execution: &'storage TerminalExecution<'_>,
        binding: ProviderStorageBinding,
    ) -> Result<(), TerminalInterpretError> {
        if !self.allocations.contains(&binding.allocation) {
            if !matches!(
                execution
                    .provider_storage
                    .allocations
                    .get(binding.allocation),
                ProviderStorageSlot::Live { .. }
            ) {
                return Err(malformed());
            }
            self.allocations.push(binding.allocation);
            self.add_cells(1)?;
        }
        if binding.resident != arena::Handle::default()
            && !self.residents.contains(&binding.resident)
        {
            let ResidentStorageSlot::Live { custody, .. } =
                execution.provider_storage.residents.get(binding.resident)
            else {
                return Err(malformed());
            };
            self.residents.push(binding.resident);
            self.add_cells(1)?;
            custody.storage.measure_result_custody(execution, self)?;
        }
        Ok(())
    }
}

impl TerminalExecution<'_> {
    pub(crate) fn charge_completed_result(
        &self,
        completed: &CompletedReturn,
    ) -> Result<(), TerminalInterpretError> {
        let Some(account) = &self.account else {
            return Ok(());
        };
        let mut custody = ResultCustody::new();
        match &completed.value {
            TerminalExecutionResult::Unit | TerminalExecutionResult::Scalar(_) => {}
            TerminalExecutionResult::ScalarCase(result) => {
                custody.add_cells(result.value().fields.len())?
            }
            TerminalExecutionResult::ScalarArray(result) => {
                custody.add_cells(result.value.elements().len())?
            }
            TerminalExecutionResult::Structural(result) => {
                // A frame descriptor carries its own backing association. Its
                // presented identity can also name borrowed referent storage;
                // that coincidence must not turn the referent into owned data.
                if completed.byte_sequence.is_none() && completed.element_view.is_none() {
                    self.measure_result_storage(
                        &StructuralRuntimePlace::from(&result.value),
                        &mut custody,
                    )?;
                }
            }
        }
        if let Some(binding) = &completed.byte_sequence {
            custody.binding(binding)?;
        }
        if let Some(view) = &completed.element_view {
            custody.element_view(view)?;
        }
        account
            .charge_result_custody(custody.cells, custody.payload_bytes)
            .map_err(TerminalInterpretError::Storage)
    }

    fn measure_result_storage<'storage>(
        &'storage self,
        root: &StructuralRuntimePlace,
        custody: &mut ResultCustody<'storage>,
    ) -> Result<(), TerminalInterpretError> {
        custody.rows(&self.structural_scalar_fields, root)?;
        custody.rows(&self.structural_primitive_storage, root)?;
        custody.rows(&self.structural_cases, root)?;
        custody.rows(&self.structural_host_atoms, root)?;
        custody.rows(&self.provider_storage.completed_profiles, root)?;
        custody.rows(&self.structural_byte_storage, root)?;
        custody.rows(&self.structural_byte_sequence_fields, root)?;
        custody.rows(&self.structural_byte_descriptors, root)?;
        custody.rows(&self.structural_element_view_fields, root)?;
        custody.rows(&self.reference_referents, root)?;
        custody.rows(&self.provider_storage.descriptors, root)?;
        for (_, view) in cells(&self.structural_byte_storage, root) {
            custody.bytes(view)?;
        }
        for (_, binding) in cells(&self.structural_byte_sequence_fields, root) {
            custody.binding(binding)?;
        }
        for (_, binding) in cells(&self.structural_byte_descriptors, root) {
            custody.binding(binding)?;
        }
        for (_, view) in cells(&self.structural_element_view_fields, root) {
            custody.element_view(view)?;
        }
        for (_, reference) in cells(&self.reference_referents, root) {
            custody.reference(reference)?;
        }
        for (_, binding) in cells(&self.provider_storage.descriptors, root) {
            custody.provider(self, *binding)?;
        }
        self.activation_storage
            .measure_result_custody(root, custody)
    }
}

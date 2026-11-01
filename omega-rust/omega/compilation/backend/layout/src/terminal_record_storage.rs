//! Borrow selected storage without replacing the canonical declarations.

use semantic_vocabulary::StructuralTypeId;
use target::TargetProfile;
use terminal_psi::{LayoutApplicationId, TargetLayoutRealization, TerminalModule};
use terminal_semantics::{RecordStorageResolver, StorageGeometryError};

pub struct TerminalRecordStorage<'module> {
    pub module: &'module TerminalModule,
    pub target: TargetProfile,
}

impl RecordStorageResolver for TerminalRecordStorage<'_> {
    fn realization(
        &self,
        record: StructuralTypeId,
        application: LayoutApplicationId,
    ) -> Result<&TargetLayoutRealization, StorageGeometryError> {
        let application = self
            .module
            .target_layout_applications
            .iter()
            .find(|row| row.id == application && row.record == record)
            .ok_or(StorageGeometryError::UnresolvedLayout(record))?;
        let selected = application
            .realizations
            .iter()
            .find(|row| row.target == self.target.target_name())
            .ok_or(StorageGeometryError::UnresolvedLayout(record))?;
        let facts = self
            .target
            .layout_target_facts()
            .ok_or(StorageGeometryError::MissingCarrierLayout)?;
        if selected.pointer_size != facts.pointer_size
            || selected.pointer_alignment != facts.pointer_alignment
            || selected.little_endian != facts.little_endian
        {
            return Err(StorageGeometryError::InvalidRecordStorage(record));
        }
        Ok(selected)
    }
}

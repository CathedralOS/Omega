//! Link the evidence attached to a source projection to its actual machine
//! operation. A local holding the selected callback keeps its SSA value.
use super::Lowering;
use crate::read::field_openings::FieldOpening;
use crate::source_state::package_openings::PackageOpeningHandle;
use arena::IdTable;
use semantic_vocabulary::{OperationId, StructuralFieldId, StructuralTypeId, ValueId};
use terminal_check_tables::Checked;
use terminal_psi::{
    StructuralArgument, TerminalPackageEvent, TerminalPackageOpening, TerminalPackageSite,
};

#[derive(Default)]
pub(super) struct PackageUses {
    openings: Vec<Option<(PackageOpeningHandle, u32)>>,
    descriptors: IdTable<ValueId, Option<u32>>,
}

impl Lowering<'_> {
    fn package_opening(
        &mut self,
        source: FieldOpening,
        subject: StructuralArgument,
        record: StructuralTypeId,
    ) -> Checked<u32> {
        let row = TerminalPackageOpening {
            machine: self.id,
            record,
            subject,
            witnesses: source.witnesses,
        };
        let slot = source.opening.arena_index() as usize;
        let opening = if let Some((handle, opening)) =
            self.package_uses.openings.get(slot).copied().flatten()
        {
            let existing = &self.packages.openings[opening as usize];
            if handle != source.opening || existing != &row {
                return Err(self.internal(
                    "one checked package opening denotes different terminal places or types",
                ));
            }
            opening
        } else {
            let opening = self
                .packages
                .openings
                .len()
                .try_into()
                .map_err(|_| self.internal("too many package openings in one machine"))?;
            self.packages.openings.push(row);
            let handles = &mut self.package_uses.openings;
            if handles.len() <= slot {
                handles.resize(slot + 1, None);
            }
            handles[slot] = Some((source.opening, opening));
            opening
        };
        Ok(opening)
    }

    pub(super) fn package_store_site(
        &mut self,
        source: FieldOpening,
        subject: StructuralArgument,
        record: StructuralTypeId,
        field: StructuralFieldId,
        operation: OperationId,
    ) -> Checked<()> {
        let opening = self.package_opening(source, subject, record)?;
        self.packages.sites.push(TerminalPackageSite {
            machine: self.id,
            operation,
            event: TerminalPackageEvent::Store { opening, field },
        });
        Ok(())
    }

    pub(super) fn package_projection_site(
        &mut self,
        source: FieldOpening,
        subject: StructuralArgument,
        record: StructuralTypeId,
        field: StructuralFieldId,
        operation: OperationId,
        descriptor: Option<ValueId>,
    ) -> Checked<()> {
        let opening = self.package_opening(source, subject, record)?;
        self.packages.sites.push(TerminalPackageSite {
            machine: self.id,
            operation,
            event: TerminalPackageEvent::Project { opening, field },
        });
        if let Some(value) = descriptor {
            let descriptors = &mut self.package_uses.descriptors;
            while descriptors.len() < self.tables.values.len() {
                descriptors.push(None);
            }
            let Some(slot) = descriptors.get_mut(value) else {
                return Err(self.internal("package descriptor has no value in this machine"));
            };
            *slot = Some(opening);
        }
        Ok(())
    }

    pub(super) fn package_invocation_site(&mut self, descriptor: ValueId, operation: OperationId) {
        if let Some(&opening) = self
            .package_uses
            .descriptors
            .get(descriptor)
            .and_then(Option::as_ref)
        {
            self.packages.sites.push(TerminalPackageSite {
                machine: self.id,
                operation,
                event: TerminalPackageEvent::Invoke { opening },
            });
        }
    }
}

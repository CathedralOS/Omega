//! Link the evidence attached to a source projection to its actual machine
//! operation. A local holding the selected callback keeps its SSA value.
use super::Lowering;
use crate::read::package_projections::PreparedProjection;
use crate::source_state::package_openings::PackageOpeningHandle;
use semantic_vocabulary::{OperationId, StructuralFieldId, StructuralTypeId, ValueId};
use std::collections::HashMap;
use terminal_check_tables::Checked;
use terminal_psi::{
    StructuralArgument, TerminalPackageEvent, TerminalPackageOpening, TerminalPackageSite,
};

#[derive(Default)]
pub(super) struct PackageUses {
    openings: HashMap<PackageOpeningHandle, u32>,
    descriptors: HashMap<ValueId, u32>,
}

impl Lowering<'_> {
    pub(super) fn package_projection_site(
        &mut self,
        source: PreparedProjection,
        subject: StructuralArgument,
        record: StructuralTypeId,
        field: StructuralFieldId,
        operation: OperationId,
        descriptor: Option<ValueId>,
    ) -> Checked<()> {
        let row = TerminalPackageOpening {
            machine: self.id,
            record,
            subject,
            witnesses: source.witnesses,
        };
        let opening = if let Some(&opening) = self.package_uses.openings.get(&source.opening) {
            let existing = &self.packages.openings[opening as usize];
            if existing != &row {
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
            self.package_uses.openings.insert(source.opening, opening);
            opening
        };
        self.packages.sites.push(TerminalPackageSite {
            machine: self.id,
            operation,
            event: TerminalPackageEvent::Project { opening, field },
        });
        if let Some(value) = descriptor {
            self.package_uses.descriptors.insert(value, opening);
        }
        Ok(())
    }

    pub(super) fn package_invocation_site(&mut self, descriptor: ValueId, operation: OperationId) {
        if let Some(&opening) = self.package_uses.descriptors.get(&descriptor) {
            self.packages.sites.push(TerminalPackageSite {
                machine: self.id,
                operation,
                event: TerminalPackageEvent::Invoke { opening },
            });
        }
    }
}

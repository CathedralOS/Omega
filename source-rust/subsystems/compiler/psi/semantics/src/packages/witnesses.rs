//! Own each abstract binder and verify reuse by a separately opened value copy.
use super::*;
use terminal_psi::TerminalPackageCatalog;

pub(super) struct WitnessBinders {
    /// Untrusted portable IDs can be sparse, so index by the observed roster.
    pub(super) types: Vec<StructuralTypeId>,
    openings: Vec<OpeningWitness>,
    observations: Vec<Option<(u32, OperationId)>>,
}

#[derive(Clone, Copy)]
enum OpeningWitness {
    Fresh,
    Shared { group: usize, observed: bool },
}

impl WitnessBinders {
    pub(super) fn from_catalog(catalog: &TerminalPackageCatalog) -> Result<Self, &'static str> {
        let mut occurrences = Vec::new();
        for schema in &catalog.schemas {
            occurrences.extend(schema.witnesses.iter().map(|id| (*id, None)));
        }
        for (index, row) in catalog.openings.iter().enumerate() {
            occurrences.extend(
                row.witnesses
                    .iter()
                    .enumerate()
                    .map(|(slot, id)| (*id, Some((index, slot)))),
            );
        }
        occurrences.sort_unstable();
        let mut openings = vec![OpeningWitness::Fresh; catalog.openings.len()];
        let mut types = Vec::new();
        for group in occurrences.chunk_by(|a, b| a.0 == b.0) {
            let (id, first) = group[0];
            types.push(id);
            for &(_, next) in &group[1..] {
                let (Some((anchor, slot)), Some((index, repeated_slot))) = (first, next) else {
                    return Err(
                        "package declarations and openings must have distinct witness identities",
                    );
                };
                if anchor == index
                    || repeated_slot != slot
                    || matches!(openings[anchor], OpeningWitness::Shared { group, .. } if group != anchor)
                    || matches!(openings[index], OpeningWitness::Shared { group, .. } if group != anchor)
                {
                    return Err("a copied package must retain its complete original witness tuple");
                }
                let shared = OpeningWitness::Shared {
                    group: anchor,
                    observed: false,
                };
                openings[anchor] = shared;
                openings[index] = shared;
            }
        }
        // Compare each tuple once, independently of its number of binders.
        for (row, owner) in catalog.openings.iter().zip(&openings) {
            if let OpeningWitness::Shared { group, .. } = owner {
                let origin = &catalog.openings[*group];
                if row.machine != origin.machine
                    || row.record != origin.record
                    || row.witnesses != origin.witnesses
                {
                    return Err("a copied package must retain its complete original witness tuple");
                }
            }
        }
        let observations = vec![None; openings.len()];
        Ok(Self {
            types,
            openings,
            observations,
        })
    }

    pub(super) fn check_use(
        &mut self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
        index: u32,
    ) -> Result<(), &'static str> {
        let slot = index as usize;
        let origin = self
            .openings
            .get_mut(slot)
            .ok_or("package operation names an absent opening")?;
        let OpeningWitness::Shared { group, observed } = origin else {
            return Ok(());
        };
        // Keep the instruction that observed the value. The first catalog
        // row need not still contain that value when other copies are used.
        let anchor = &mut self.observations[*group];
        if let Some(previous) = *anchor {
            if !subjects::observed_witness(module, machine, operation, index, previous) {
                return Err("package openings reuse a witness without a live copy of the original");
            }
        } else {
            *anchor = Some((index, operation.id));
        }
        *observed = true;
        Ok(())
    }

    pub(super) fn require_copy_evidence(&self) -> Result<(), &'static str> {
        if self.openings.iter().any(|owner| {
            matches!(
                owner,
                OpeningWitness::Shared {
                    observed: false,
                    ..
                }
            )
        }) {
            return Err("a duplicated package witness has no observed copy");
        }
        Ok(())
    }
}

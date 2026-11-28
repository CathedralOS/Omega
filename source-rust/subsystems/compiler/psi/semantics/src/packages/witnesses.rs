//! Own each abstract binder and verify reuse by a separately opened value copy.
use super::*;
use terminal_psi::TerminalPackageCatalog;

pub(super) struct WitnessBinders {
    /// Untrusted portable IDs can be sparse, so index by the observed roster.
    pub(super) types: Vec<StructuralTypeId>,
    openings: Vec<OpeningWitness>,
}

#[derive(Clone, Copy)]
enum OpeningWitness {
    Fresh,
    Copied { original: usize, observed: bool },
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
                    || matches!(openings[index], OpeningWitness::Copied { original, .. } if original != anchor)
                {
                    return Err("a copied package must retain its complete original witness tuple");
                }
                openings[index] = OpeningWitness::Copied {
                    original: anchor,
                    observed: false,
                };
            }
        }
        // Compare each tuple once, independently of its number of binders.
        for (row, owner) in catalog.openings.iter().zip(&openings) {
            if let OpeningWitness::Copied { original, .. } = owner {
                let origin = &catalog.openings[*original];
                if row.machine != origin.machine
                    || row.record != origin.record
                    || row.witnesses != origin.witnesses
                {
                    return Err("a copied package must retain its complete original witness tuple");
                }
            }
        }
        Ok(Self { types, openings })
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
        let OpeningWitness::Copied { original, observed } = origin else {
            return Ok(());
        };
        let openings = &module.execution.packages.openings;
        if !subjects::copied_witness_at(
            module,
            machine,
            operation,
            &openings[*original],
            &openings[slot],
        ) {
            return Err("package openings reuse a witness without a live copy of the original");
        }
        *observed = true;
        Ok(())
    }

    pub(super) fn require_copy_evidence(&self) -> Result<(), &'static str> {
        if self.openings.iter().any(|owner| {
            matches!(
                owner,
                OpeningWitness::Copied {
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

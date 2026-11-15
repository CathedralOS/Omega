//! Materialize reference dispositions before planning structural disposal.
//!
//! A last descriptor use proposes a cut; it does not itself return the loan.
//! The shared resource planner authenticates every surviving carrier and its
//! children, and this producer emits the existing explicit release operation.
//! Receiving analysis independently checks its custody and restoration.

use semantic_vocabulary::{IdTable, MachineId};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::{
    CheckTables, Checked, Construct, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{Operation, OperationKind, OperationResult, TerminalModule};

use crate::MachineSources;

pub(crate) fn publish(
    module: &mut TerminalModule,
    tables: &mut CheckTables,
    sources: &mut IdTable<MachineId, MachineSources>,
) -> Checked<()> {
    for ordinal in 0..module.machines.len() {
        let machine = &module.machines[ordinal];
        let identity = machine.id;
        let refuse = |detail: &str| Refusal {
            reason: RefusalReason::Unsupported,
            machine: identity,
            concerns: Construct::Machine,
            detail: detail.to_owned(),
        };
        let releases =
            terminal_semantics::reference_releases::plan(module, identity).map_err(|error| {
                let mut refusal = refuse(error.detail);
                if let Some((block, position)) = error.at
                    && let Some(block) = machine.blocks.iter().find(|row| row.id == block)
                {
                    refusal.concerns = block
                        .operations
                        .get(position as usize)
                        .map_or(Construct::Block(block.id), |row| {
                            Construct::Operation(row.id)
                        });
                }
                refusal
            })?;
        if releases.is_empty() {
            continue;
        }
        let mut prepared = releases
            .into_iter()
            .map(|release| {
                let declaration = machine
                    .structural_place_type(release.descriptor.root)
                    .ok_or_else(|| refuse("reference release has no exact carrier type"))?;
                let path = terminal_semantics::descriptor_lifetimes::descriptor_path(
                    &module.structural_types,
                    declaration,
                    &release.descriptor.path,
                )
                .ok_or_else(|| refuse("reference release has no finite owned carrier path"))?;
                Ok((release.at, release.descriptor.root, path))
            })
            .collect::<Checked<Vec<_>>>()?;
        // Stable ordering preserves the planner's descendant-before-parent
        // transaction within each cut, independently of storage root IDs.
        prepared.sort_by_key(|(at, _, _)| *at);
        let mut pending = prepared.into_iter().peekable();
        let source = sources
            .get_mut(identity)
            .ok_or_else(|| refuse("reference release has no machine source roster"))?;
        let machine = &mut module.machines[ordinal];
        let mut moved_points = Vec::new();
        for block in &mut machine.blocks {
            let original = std::mem::take(&mut block.operations);
            let mut operations = Vec::with_capacity(original.len());
            let mut before = ProgramPoint::BlockEntry(block.id);
            for (position, operation) in original
                .into_iter()
                .map(Some)
                .chain(std::iter::once(None))
                .enumerate()
            {
                let position = u32::try_from(position)
                    .map_err(|_| refuse("reference release position overflows"))?;
                let cut = (block.id, position);
                let origin = operation
                    .as_ref()
                    .and_then(|operation| source.operations.get(operation.id))
                    .or_else(|| source.blocks.get(block.id))
                    .copied()
                    .ok_or_else(|| refuse("reference release cut has no source correspondence"))?;
                let mut after = before;
                while pending.peek().is_some_and(|(at, _, _)| *at == cut) {
                    let (_, root, path) = pending.next().expect("the release cut was inspected");
                    let identity = source.operations.push(origin);
                    operations.push(Operation::unsuspended(
                        identity,
                        OperationResult::Unit,
                        OperationKind::ReleaseReference { source: root, path },
                    ));
                    after = ProgramPoint::AfterOperation(identity);
                }
                if after != before {
                    moved_points.push((before, after));
                }
                if let Some(operation) = operation {
                    before = ProgramPoint::AfterOperation(operation.id);
                    operations.push(operation);
                }
            }
            block.operations = operations;
        }
        if pending.next().is_some() {
            return Err(refuse("reference release names no actual operation cut"));
        }
        moved_points.sort_by_key(|(before, _)| *before);
        let table = tables
            .machines
            .get_mut(identity)
            .ok_or_else(|| refuse("reference release has no machine check tables"))?;
        for identity in table.obligations.keys().collect::<Vec<_>>() {
            let row = table
                .obligations
                .get_mut(identity)
                .expect("this table minted the row");
            // Pre-operation and pre-edge questions follow the inserted
            // dispositions. Actual establishments, writes and knowledge
            // keep the point of their original producer; delaying a store's
            // write epoch would no longer describe the emitted operation.
            let follows = match row.kind {
                ObligationKind::Loan { .. }
                | ObligationKind::CallRequirement { .. }
                | ObligationKind::OperationSafety { .. }
                | ObligationKind::DataDomain { .. }
                | ObligationKind::ArrivalInvariant { .. }
                | ObligationKind::NormalGuarantee { .. }
                | ObligationKind::CrashCoverage { .. }
                | ObligationKind::RankComparison { .. } => true,
                ObligationKind::Claim { .. }
                | ObligationKind::RestorationWindow { .. }
                | ObligationKind::DomainDebt { .. }
                | ObligationKind::EstablishmentDebt { .. }
                | ObligationKind::IntegerWidthPositive { .. }
                | ObligationKind::PublishedGuarantee { .. }
                | ObligationKind::WellFoundedness { .. } => false,
            };
            if follows
                && let Ok(position) =
                    moved_points.binary_search_by_key(&row.created_at, |(before, _)| *before)
            {
                row.created_at = moved_points[position].1;
            }
        }
    }
    Ok(())
}

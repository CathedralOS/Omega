//! Materialize reference dispositions before planning structural disposal.
//!
//! A last descriptor use proposes a cut; it does not itself return the loan.
//! The shared resource planner authenticates every surviving carrier and its
//! children, and this producer emits the existing explicit release operation.
//! Receiving analysis independently checks its custody and restoration.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IdTable, MachineId, OperationId, PlaceId,
};
use terminal_check_tables::bindings::{
    Content, Segment, WriteRow, intern_projection, record_write,
};
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
                let canonical = terminal_semantics::canonical_inline_region(
                    module.structural_types.iter(),
                    declaration,
                    &path,
                )
                .ok_or_else(|| refuse("reference release crosses borrowed carrier storage"))?;
                let segments = canonical
                    .into_iter()
                    .map(|segment| match segment {
                        CanonicalStructuralPathSegment::Field(field) => Ok(Segment::Field(field)),
                        CanonicalStructuralPathSegment::Case(case) => Ok(Segment::Case(case)),
                        CanonicalStructuralPathSegment::FixedIndex(index) => {
                            Ok(Segment::FixedIndex(index))
                        }
                        CanonicalStructuralPathSegment::Referent => {
                            Err(refuse("reference release crosses a referent"))
                        }
                    })
                    .collect::<Checked<Vec<_>>>()?;
                Ok((release.at, release.descriptor.root, path, segments))
            })
            .collect::<Checked<Vec<_>>>()?;
        // Stable ordering preserves the planner's descendant-before-parent
        // transaction within each cut, independently of storage root IDs.
        prepared.sort_by_key(|(at, _, _, _)| *at);
        let mut pending = prepared.into_iter().peekable();
        let source = sources
            .get_mut(identity)
            .ok_or_else(|| refuse("reference release has no machine source roster"))?;
        let machine = &mut module.machines[ordinal];
        let mut moved_points = Vec::new();
        let mut vacated = Vec::new();
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
                while pending.peek().is_some_and(|(at, _, _, _)| *at == cut) {
                    let (_, root, path, segments) =
                        pending.next().expect("the release cut was inspected");
                    let identity = source.operations.push(origin);
                    vacated.push((identity, root, segments));
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
        record_releases(machine, table, source.operations.len(), vacated)?;
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

/// Disposing a descriptor vacates its exact storage in the ordinary write log.
/// Rebuild only the derived write IDs/columns in emitted operation order; all
/// existing operation epochs, contents and exact place identities are retained.
fn record_releases(
    machine: &terminal_psi::TerminalMachine,
    table: &mut terminal_check_tables::MachineTables,
    operation_count: usize,
    vacated: Vec<(OperationId, PlaceId, Vec<Segment>)>,
) -> Checked<()> {
    let refuse = |detail: &str| Refusal {
        reason: RefusalReason::ForeignIdentity,
        machine: machine.id,
        concerns: Construct::Machine,
        detail: detail.to_owned(),
    };
    let slot = |operation: OperationId| {
        usize::try_from(operation.get())
            .ok()
            .and_then(|raw| raw.checked_sub(1))
            .filter(|index| *index < operation_count)
            .ok_or_else(|| refuse("a reference release write names no allocated operation"))
    };
    let mut rows = (0..operation_count)
        .map(|_| Vec::new())
        .collect::<Vec<Vec<WriteRow>>>();
    for row in table.writes.values() {
        let ProgramPoint::AfterOperation(operation) = row.at else {
            return Err(refuse("an existing write has no actual operation epoch"));
        };
        rows[slot(operation)?].push(*row);
    }
    for (operation, root, path) in vacated {
        let mut place = table
            .roots
            .get(root)
            .copied()
            .ok_or_else(|| refuse("reference release has no exact carrier root"))?;
        for segment in path {
            place = intern_projection(machine.id, &mut table.places, place, segment)?;
        }
        rows[slot(operation)?].push(WriteRow {
            place,
            at: ProgramPoint::AfterOperation(operation),
            content: Content::Vacated,
        });
    }
    for place in table.places.keys().collect::<Vec<_>>() {
        table
            .places
            .get_mut(place)
            .expect("this table minted the place")
            .writes
            .clear();
    }
    table.writes = IdTable::default();
    for operation in machine.operations() {
        for row in std::mem::take(&mut rows[slot(operation.id)?]) {
            record_write(machine.id, &mut table.places, &mut table.writes, row)?;
        }
    }
    if rows.iter().any(|rows| !rows.is_empty()) {
        return Err(refuse(
            "a write names an operation absent from the emitted body",
        ));
    }
    Ok(())
}

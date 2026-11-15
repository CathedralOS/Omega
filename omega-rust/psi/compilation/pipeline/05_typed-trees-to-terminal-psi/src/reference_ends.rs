//! Turn the descriptor lifetime frontier into explicit storage disposition.
//! Releases precede the operation after the last use; ordinary edge cleanup
//! handles the complement. Both are planned by the same owned-region walk.

use crate::{SourceOrigin, Unpositioned, Unserved, refuse};
use semantic_vocabulary::{CanonicalStructuralPathSegment, IdTable, MachineId};
use terminal_check_tables::bindings::{self, Content, Segment, WriteRow};
use terminal_check_tables::program::MachineSources;
use terminal_check_tables::{CheckTables, Checked, Construct, ProgramPoint};
use terminal_psi::{Operation, OperationKind, OperationResult, TerminalModule};

/// A machine whose reference ends cannot be planned yet is refused on its
/// own: the first such refusal is returned once every machine was tried,
/// and each later one is put in `further`.
pub(crate) fn install(
    module: &mut TerminalModule,
    tables: &mut CheckTables,
    sources: &mut IdTable<MachineId, MachineSources>,
    further: &mut Vec<terminal_check_tables::Refusal>,
) -> Checked<()> {
    let mut unsupported: Vec<terminal_check_tables::Refusal> = Vec::new();
    let candidates = terminal_semantics::reference_ends::candidates(module).map_err(|error| {
        terminal_check_tables::Refusal {
            source_span: error.operation.and_then(|operation| {
                sources
                    .get(error.machine)?
                    .operations
                    .get(operation)?
                    .authored_span()
            }),
            reason: terminal_check_tables::RefusalReason::Unsupported,
            machine: error.machine,
            concerns: error
                .operation
                .map(Construct::Operation)
                .unwrap_or(Construct::Machine),
            detail: error.detail,
        }
    })?;
    for ordinal in 0..module.machines.len() {
        let owner = module.machines[ordinal].id;
        let candidates = candidates.get(owner).ok_or_else(|| {
            refuse(
                owner,
                Construct::Machine,
                Unserved::Machine("reference disposition lost its machine"),
            )
        })?;
        if candidates.is_empty() {
            continue;
        }
        let plan = terminal_semantics::plain_affine::plan_reference_ends(module, owner, candidates)
            .map_err(|error| {
                let concerns = match error.site() {
                    Some(terminal_semantics::plain_affine::Site::Operation(at)) => {
                        Construct::Operation(at)
                    }
                    Some(terminal_semantics::plain_affine::Site::Edge(at)) => Construct::Edge(at),
                    None => Construct::Machine,
                };
                let (reason, detail) = match error {
                    terminal_semantics::plain_affine::Error::Unsupported { detail, .. } => {
                        (terminal_check_tables::RefusalReason::Unsupported, detail)
                    }
                    terminal_semantics::plain_affine::Error::Malformed { detail, .. } => (
                        terminal_check_tables::RefusalReason::ForeignIdentity,
                        detail,
                    ),
                };
                terminal_check_tables::Refusal {
                    source_span: None,
                    reason,
                    machine: owner,
                    concerns,
                    detail: detail.to_owned(),
                }
            });
        let plan = match plan {
            Ok(plan) => plan,
            Err(refused) if refused.reason == terminal_check_tables::RefusalReason::Unsupported => {
                unsupported.push(refused);
                continue;
            }
            Err(refused) => return Err(refused),
        };
        let machine = &mut module.machines[ordinal];
        let source = sources.get_mut(owner).ok_or_else(|| {
            refuse(
                owner,
                Construct::Machine,
                Unserved::Machine("reference disposition lost its source correspondence"),
            )
        })?;
        let tables = tables.machines.get_mut(owner).ok_or_else(|| {
            refuse(
                owner,
                Construct::Machine,
                Unserved::Machine("reference disposition lost its storage tables"),
            )
        })?;
        let mut ends = plan.reference_ends.into_iter().peekable();
        let mut relocated = Vec::new();
        for block in &mut machine.blocks {
            let mut before = ProgramPoint::BlockEntry(block.id);
            let old = std::mem::take(&mut block.operations);
            let mut operations = Vec::with_capacity(old.len());
            for (position, operation) in old.into_iter().enumerate() {
                let at = (
                    block.id,
                    u32::try_from(position).map_err(|_| {
                        refuse(
                            owner,
                            Construct::Block(block.id),
                            Unserved::Machine("reference end position overflows"),
                        )
                    })?,
                );
                let mut revised_before = before;
                while ends.peek().is_some_and(|end| end.at == at) {
                    let end = ends.next().expect("peeked reference end");
                    let argument = end.source;
                    let id = source
                        .operations
                        .push(SourceOrigin::Unpositioned(Unpositioned::ReferenceLastUse));
                    let path = terminal_semantics::canonical_inline_region(
                        module.structural_types.iter(),
                        end.structural_type,
                        &argument.path,
                    )
                    .ok_or_else(|| {
                        refuse(
                            owner,
                            Construct::Operation(id),
                            Unserved::Machine("reference end has no exact owned path"),
                        )
                    })?;
                    let mut place = *tables.roots.get(argument.place).ok_or_else(|| {
                        refuse(
                            owner,
                            Construct::Operation(id),
                            Unserved::Machine("reference end lost its exact root"),
                        )
                    })?;
                    for segment in path {
                        let segment = match segment {
                            CanonicalStructuralPathSegment::Field(field) => Segment::Field(field),
                            CanonicalStructuralPathSegment::FixedIndex(index) => {
                                Segment::FixedIndex(index)
                            }
                            CanonicalStructuralPathSegment::Case(case) => Segment::Case(case),
                            CanonicalStructuralPathSegment::Referent => {
                                return Err(refuse(
                                    owner,
                                    Construct::Operation(id),
                                    Unserved::Machine("reference end crosses borrowed storage"),
                                ));
                            }
                        };
                        place =
                            bindings::intern_projection(owner, &mut tables.places, place, segment)?;
                    }
                    bindings::record_write(
                        owner,
                        &mut tables.places,
                        &mut tables.writes,
                        WriteRow {
                            place,
                            at: ProgramPoint::AfterOperation(id),
                            content: Content::Vacated,
                        },
                    )?;
                    operations.push(Operation::unsuspended(
                        id,
                        OperationResult::Unit,
                        OperationKind::ReleaseReference {
                            source: argument.place,
                            path: argument.path,
                        },
                    ));
                    revised_before = ProgramPoint::AfterOperation(id);
                }
                if revised_before != before {
                    relocated.push((before, revised_before));
                }
                before = ProgramPoint::AfterOperation(operation.id);
                operations.push(operation);
            }
            block.operations = operations;
        }
        relocated.sort_unstable_by_key(|(before, _)| *before);
        let obligations: Vec<_> = tables
            .obligations
            .iter()
            .filter_map(|(id, row)| {
                row.kind
                    .precedes_execution()
                    .then(|| {
                        relocated
                            .binary_search_by_key(&row.created_at, |(before, _)| *before)
                            .ok()
                            .map(|position| (id, relocated[position].1))
                    })
                    .flatten()
            })
            .collect();
        for (id, point) in obligations {
            tables
                .obligations
                .get_mut(id)
                .ok_or_else(|| {
                    refuse(
                        owner,
                        Construct::Obligation(id),
                        Unserved::Machine("reference disposition lost its relocated obligation"),
                    )
                })?
                .created_at = point;
        }
        if ends.next().is_some() {
            return Err(refuse(
                owner,
                Construct::Machine,
                Unserved::Machine("reference end has no owning operation boundary"),
            ));
        }
    }
    if !unsupported.is_empty() {
        let first = unsupported.remove(0);
        further.append(&mut unsupported);
        return Err(first);
    }
    Ok(())
}

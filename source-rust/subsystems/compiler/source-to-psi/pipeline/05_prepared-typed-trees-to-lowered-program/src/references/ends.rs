//! Turn the descriptor lifetime frontier into explicit storage disposition.
//! Releases precede the operation, or the dispatch, after the last use;
//! handles the complement. Both are planned by the same owned-region walk.

use crate::{SourceOrigin, Unpositioned, Unserved, refuse};
use arena::IdTable;
use semantic_vocabulary::{CanonicalStructuralPathSegment, MachineId};
use terminal_check_tables::bindings::{self, Content, Segment, WriteRow};
use terminal_check_tables::program::MachineSources;
use terminal_check_tables::{
    CheckTables, Checked, Construct, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{Operation, OperationKind, OperationResult, TerminalModule};
use terminal_semantics::plain_affine::{AffineSummaries, Error, MachineAffine, Site};

fn planning_refusal(machine: MachineId, error: Error, sources: Option<&MachineSources>) -> Refusal {
    let (concerns, origin) = match error.site() {
        Some(Site::Operation(at)) => (
            Construct::Operation(at),
            sources.and_then(|sources| sources.operations.get(at)),
        ),
        Some(Site::Edge(at)) => (
            Construct::Edge(at),
            sources.and_then(|sources| sources.edges.get(at)),
        ),
        None => (Construct::Machine, None),
    };
    let (reason, detail) = match error {
        Error::Unavailable(unavailable) => (RefusalReason::Unestablished, unavailable.message()),
        Error::Unconsumed { at_join, .. } => {
            (RefusalReason::Violation, Error::unconsumed_message(at_join))
        }
        Error::Unsupported { detail, .. } => (RefusalReason::Unsupported, detail),
        Error::Malformed { detail, .. } => (RefusalReason::ForeignIdentity, detail),
    };
    Refusal {
        source_span: origin
            .copied()
            .and_then(terminal_check_tables::program::SourceOrigin::authored_span),
        reason,
        site: terminal_check_tables::RefusalSite::Machine(machine, concerns),
        detail: detail.to_owned(),
    }
}

fn candidates_refusal(
    error: terminal_semantics::reference_ends::Error,
    sources: &IdTable<MachineId, MachineSources>,
) -> Refusal {
    Refusal {
        source_span: error.operation.and_then(|operation| {
            sources
                .get(error.machine)?
                .operations
                .get(operation)?
                .authored_span()
        }),
        reason: RefusalReason::Unsupported,
        site: terminal_check_tables::RefusalSite::Machine(
            error.machine,
            error
                .operation
                .map(Construct::Operation)
                .unwrap_or(Construct::Machine),
        ),
        detail: error.detail,
    }
}

/// Plan the reference ends of one machine against `module` and `summaries`.
fn plan_machine_reference_ends(
    module: &TerminalModule,
    summaries: &AffineSummaries<'_>,
    machine: &terminal_psi::TerminalMachine,
    candidates: &IdTable<MachineId, terminal_semantics::reference_ends::Candidates>,
    sources: &IdTable<MachineId, MachineSources>,
) -> Checked<Option<terminal_semantics::plain_affine::Plan>> {
    let owner = machine.id;
    let candidates = candidates.get(owner).ok_or_else(|| {
        refuse(
            owner,
            Construct::Machine,
            Unserved::Machine("reference disposition lost its machine"),
        )
    })?;
    if candidates.is_empty() {
        return Ok(None);
    }
    let plan = MachineAffine::new(module, owner, Some(summaries))
        .and_then(|walks| walks.plan_reference_ends(candidates))
        .map_err(|error| planning_refusal(owner, error, sources.get(owner)))?;
    Ok(Some(plan))
}

/// A machine whose reference ends cannot be planned yet is refused on its
/// own: the first such refusal is returned once every machine was tried,
/// and each later one is put in `further`.
pub(crate) fn install(
    module: &mut TerminalModule,
    tables: &mut CheckTables,
    sources: &mut IdTable<MachineId, MachineSources>,
    further: &mut Vec<terminal_check_tables::Refusal>,
    parallel: bool,
) -> Checked<()> {
    let mut unsupported: Vec<terminal_check_tables::Refusal> = Vec::new();
    let (candidates, advance) = {
        let module = &*module;
        let summaries = AffineSummaries::new(module);
        let candidates = terminal_semantics::reference_ends::candidates_sharing(module, &summaries)
            .map_err(|error| candidates_refusal(error, sources))?;
        let active_count = module
            .machines
            .iter()
            .filter(|machine| {
                candidates
                    .get(machine.id)
                    .is_some_and(|rows| !rows.is_empty())
            })
            .count();
        let advance: Vec<Option<Checked<Option<terminal_semantics::plain_affine::Plan>>>> =
            if parallel && active_count >= 2 {
                use rayon::prelude::*;
                module
                    .machines
                    .par_iter()
                    .with_max_len(1)
                    .map(|machine| {
                        Some(plan_machine_reference_ends(
                            module,
                            &summaries,
                            machine,
                            &candidates,
                            sources,
                        ))
                    })
                    .collect()
            } else {
                module
                    .machines
                    .iter()
                    .map(|machine| {
                        Some(plan_machine_reference_ends(
                            module,
                            &summaries,
                            machine,
                            &candidates,
                            sources,
                        ))
                    })
                    .collect()
            };
        (candidates, advance)
    };
    let sources_ref = &*sources;
    let mut pending_sources: Vec<(MachineId, Vec<terminal_check_tables::program::SourceOrigin>)> =
        Vec::new();
    // Note: re-planning reads `sources_ref` only when a machine's `plan` errors.
    // The inserted `ReleaseReference` ops are unpositioned, and `sources_ref` is
    // unchanged for existing operations, so `sources_ref` stays exact during `plan`.
    crate::lower::machine_order::plan_then_apply_from_advance(
        module,
        advance,
        |module, summaries, machine| {
            plan_machine_reference_ends(module, summaries, machine, &candidates, sources_ref)
        },
        |planned| match planned {
            Ok(Some(plan)) => Some(plan.graphed_machines.as_slice()),
            Ok(None) => Some(&[]),
            Err(_) => None,
        },
        |machine, structural_types, planned| {
            let owner = machine.id;
            let plan = match planned {
                Ok(Some(plan)) => plan,
                Ok(None) => return Ok(false),
                Err(refused) if refused.reason == RefusalReason::Unsupported => {
                    unsupported.push(refused);
                    return Ok(false);
                }
                Err(refused) => return Err(refused),
            };
            let source = sources_ref.get(owner).ok_or_else(|| {
                refuse(
                    owner,
                    Construct::Machine,
                    Unserved::Machine("reference disposition lost its source correspondence"),
                )
            })?;
            let machine_tables = tables.machines.get_mut(owner).ok_or_else(|| {
                refuse(
                    owner,
                    Construct::Machine,
                    Unserved::Machine("reference disposition lost its storage tables"),
                )
            })?;
            if plan.reference_ends.is_empty() {
                return Ok(false);
            }
            let mut ends = plan.reference_ends.into_iter().peekable();
            let mut relocated = Vec::new();
            let mut new_operations = Vec::new();
            for block in &mut machine.blocks {
                let mut before = ProgramPoint::BlockEntry(block.id);
                let old = std::mem::take(&mut block.operations);
                let mut operations = Vec::with_capacity(old.len());
                let count = old.len();
                let mut old = old.into_iter();
                for position in 0..=count {
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
                        let id = semantic_vocabulary::OperationId::new(
                            u64::try_from(source.operations.len() + new_operations.len() + 1)
                                .expect("operation identity fits u64"),
                        )
                        .expect("nonzero operation identity");
                        new_operations.push(
                            SourceOrigin::Unpositioned(Unpositioned::ReferenceLastUse).into(),
                        );
                        let path = terminal_semantics::canonical_inline_region(
                            structural_types.iter(),
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
                        let mut place =
                            *machine_tables.roots.get(argument.place).ok_or_else(|| {
                                refuse(
                                    owner,
                                    Construct::Operation(id),
                                    Unserved::Machine("reference end lost its exact root"),
                                )
                            })?;
                        for segment in path {
                            let segment = match segment {
                                CanonicalStructuralPathSegment::Field(field) => {
                                    Segment::Field(field)
                                }
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
                            place = bindings::intern_projection(
                                owner,
                                &mut machine_tables.places,
                                place,
                                segment,
                            )?;
                        }
                        bindings::record_write(
                            owner,
                            &mut machine_tables.places,
                            &mut machine_tables.writes,
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
                    let Some(operation) = old.next() else {
                        break;
                    };
                    before = ProgramPoint::AfterOperation(operation.id);
                    operations.push(operation);
                }
                block.operations = operations;
            }
            relocated.sort_unstable_by_key(|(before, _)| *before);
            let obligations: Vec<_> = machine_tables
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
                machine_tables
                    .obligations
                    .get_mut(id)
                    .ok_or_else(|| {
                        refuse(
                            owner,
                            Construct::Obligation(id),
                            Unserved::Machine(
                                "reference disposition lost its relocated obligation",
                            ),
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
            if !new_operations.is_empty() {
                pending_sources.push((owner, new_operations));
            }
            Ok(true)
        },
    )?;
    for (owner, new_operations) in pending_sources {
        let source = sources.get_mut(owner).ok_or_else(|| {
            refuse(
                owner,
                Construct::Machine,
                Unserved::Machine("reference disposition lost its source correspondence"),
            )
        })?;
        for origin in new_operations {
            source.operations.push(origin);
        }
    }
    if !unsupported.is_empty() {
        let first = unsupported.remove(0);
        further.append(&mut unsupported);
        return Err(first);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planning_refusals_use_the_exact_authored_operation_or_edge() {
        let machine = MachineId::new(1).unwrap();
        let source = source_text::SourceId(2);
        let operation_span = source_text::SourceSpan::new(source, source_text::Span::new(10, 13));
        let edge_span = source_text::SourceSpan::new(source, source_text::Span::new(20, 25));
        let mut sources = MachineSources::default();
        let operation = sources
            .operations
            .push(SourceOrigin::Authored(operation_span).into());
        let edge = sources.edges.push(SourceOrigin::Authored(edge_span).into());
        let synthetic = sources
            .operations
            .push(SourceOrigin::Unpositioned(Unpositioned::ReferenceLastUse).into());

        for (site, expected) in [
            (Some(Site::Operation(operation)), Some(operation_span)),
            (Some(Site::Edge(edge)), Some(edge_span)),
            (Some(Site::Operation(synthetic)), None),
            (None, None),
        ] {
            let refusal = planning_refusal(
                machine,
                Error::Unsupported {
                    detail: "last-use reference disposition needs an established active case",
                    site,
                },
                Some(&sources),
            );
            assert_eq!(refusal.source_span, expected);
        }
    }
}

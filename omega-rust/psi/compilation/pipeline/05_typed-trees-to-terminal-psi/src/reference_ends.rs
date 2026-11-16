//! Turn the descriptor lifetime frontier into explicit storage disposition.
//! Releases precede the operation after the last use; ordinary edge cleanup
//! handles the complement. Both are planned by the same owned-region walk.

use crate::{SourceOrigin, Unpositioned, Unserved, refuse};
use semantic_vocabulary::{CanonicalStructuralPathSegment, IdTable, MachineId};
use terminal_check_tables::bindings::{self, Content, Segment, WriteRow};
use terminal_check_tables::program::MachineSources;
use terminal_check_tables::{
    CheckTables, Checked, Construct, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{Operation, OperationKind, OperationResult, TerminalModule};
use terminal_semantics::plain_affine::{Error, Site};

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
        Error::Unsupported { detail, .. } => (RefusalReason::Unsupported, detail),
        Error::Malformed { detail, .. } => (RefusalReason::ForeignIdentity, detail),
    };
    Refusal {
        source_span: origin
            .copied()
            .and_then(terminal_check_tables::program::SourceOrigin::authored_span),
        reason,
        machine,
        concerns,
        detail: detail.to_owned(),
    }
}

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
            .map_err(|error| planning_refusal(owner, error, sources.get(owner)));
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
                        .push(SourceOrigin::Unpositioned(Unpositioned::ReferenceLastUse).into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planning_refusals_use_the_exact_authored_operation_or_edge() {
        let machine = MachineId::new(1).unwrap();
        let source = source::SourceId(2);
        let operation_span = source::SourceSpan::new(source, source::Span::new(10, 13));
        let edge_span = source::SourceSpan::new(source, source::Span::new(20, 25));
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

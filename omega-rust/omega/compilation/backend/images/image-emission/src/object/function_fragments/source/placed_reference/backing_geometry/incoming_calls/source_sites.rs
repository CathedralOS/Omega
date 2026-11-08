//! Index terminal input cuts once; an edge never borrows a normal operation ID.
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use semantic_vocabulary::{BoundaryMachineId, ExternalCompletionKind, InvocationSite, MachineId};
use terminal_psi::StructuralArgument;
use terminal_semantics::descriptor_lifetimes::DescriptorPosition;

#[derive(Clone, Copy)]
pub(super) enum SourceCallTarget {
    Machine(MachineId),
    Boundary {
        boundary: BoundaryMachineId,
        kind: ExternalCompletionKind,
    },
}
pub(super) struct SourceSite<'a> {
    pub position: Result<DescriptorPosition, &'static str>,
    pub target: SourceCallTarget,
    pub arguments: &'a [StructuralArgument],
}
struct Row<'a> {
    caller: MachineId,
    site: InvocationSite,
    source: SourceSite<'a>,
}
pub(super) struct SourceSites<'a> {
    rows: Vec<Row<'a>>,
}
impl<'a> SourceSites<'a> {
    pub(super) fn new(source: &'a AbstractOperationPlan) -> Self {
        let mut rows = Vec::new();
        for body in &source.functions {
            for (index, block) in body.block_entries.iter().enumerate() {
                let end = body
                    .block_entries
                    .get(index + 1)
                    .map_or(body.operations.len(), |next| next.operation_offset);
                for (offset, operation) in body.operations[block.operation_offset..end]
                    .iter()
                    .enumerate()
                {
                    let (site, target, arguments) = match operation {
                        AbstractOperation::InvokeNoNormal {
                            psi_edge,
                            callee,
                            structural_arguments,
                            ..
                        } => (
                            InvocationSite::InvokeNoNormal(*psi_edge),
                            SourceCallTarget::Machine(*callee),
                            structural_arguments.as_slice(),
                        ),
                        AbstractOperation::ExternalCompletion {
                            psi_edge,
                            boundary,
                            kind,
                            structural_arguments,
                            ..
                        } => (
                            InvocationSite::ExternalCompletion(*psi_edge),
                            SourceCallTarget::Boundary {
                                boundary: *boundary,
                                kind: *kind,
                            },
                            structural_arguments.as_slice(),
                        ),
                        _ => continue,
                    };
                    let position = u32::try_from(offset)
                        .map(|offset| (block.block, offset))
                        .map_err(|_| "terminal caller position exceeds descriptor range");
                    rows.push(Row {
                        caller: body.machine,
                        site,
                        source: SourceSite {
                            position,
                            target,
                            arguments,
                        },
                    });
                }
            }
        }
        rows.sort_by_key(|row| (row.caller, row.site));
        Self { rows }
    }
    pub(super) fn get(
        &self,
        caller: MachineId,
        site: InvocationSite,
    ) -> Result<&SourceSite<'a>, &'static str> {
        let key = (caller, site);
        let start = self
            .rows
            .partition_point(|row| (row.caller, row.site) < key);
        let end = self
            .rows
            .partition_point(|row| (row.caller, row.site) <= key);
        if end != start + 1 {
            return Err("terminal call has no unique abstract snapshot");
        }
        Ok(&self.rows[start].source)
    }
}

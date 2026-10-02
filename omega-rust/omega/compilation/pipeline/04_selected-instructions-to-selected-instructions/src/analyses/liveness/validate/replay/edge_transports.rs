//! Replay-owned edge substitutions from independently validated selected input.
//!
//! Transport validation establishes the exact parameter/argument relationships
//! before this preparation. Keep those relationships for all fixed-point sweeps
//! and successor publication instead of resolving them through the full register
//! roster again. No producer flow, liveness result, or retained scratch is read.

use crate::analyses::liveness::LivenessError;
use selected_instructions::{
    SelectedBlockId, SelectedCasePayloadTransport, SelectedFunction, SelectedSuccessor,
    SelectedTerminator, SelectedValueTransport, VirtualRegisterId,
};

pub(super) struct EdgeTransport {
    pub(super) target: SelectedBlockId,
    substitutions: Vec<(VirtualRegisterId, VirtualRegisterId)>,
}

pub(super) struct EdgeTransports {
    // Actual block and successor ordinals retain polarity, including two edges
    // targeting the same block. Sparse selected IDs never size working storage.
    pub(super) blocks: Vec<Vec<EdgeTransport>>,
    registers: Vec<VirtualRegisterId>,
}

impl EdgeTransports {
    /// Only called after this replay's own exact `validate_transports` succeeds.
    pub(super) fn prepare(function: &SelectedFunction) -> Self {
        let blocks = function
            .blocks
            .iter()
            .map(|block| {
                successors(&block.terminator)
                    .map(|successor| {
                        let mut substitutions = successor
                            .bindings
                            .iter()
                            .filter_map(|binding| match binding.transport {
                                SelectedValueTransport::Registers {
                                    argument,
                                    parameter,
                                } => Some((parameter, argument)),
                                SelectedValueTransport::Unused => None,
                            })
                            .collect::<Vec<_>>();
                        if let Some(case) = &successor.structural_case {
                            substitutions.extend(case.payloads.iter().filter_map(|payload| {
                                match payload.transport {
                                    SelectedCasePayloadTransport::Registers {
                                        argument,
                                        parameter,
                                    } => Some((parameter, argument)),
                                    SelectedCasePayloadTransport::Unused
                                    | SelectedCasePayloadTransport::Unmaterialized { .. } => None,
                                }
                            }));
                        }
                        substitutions.sort_unstable_by_key(|(parameter, _)| *parameter);
                        EdgeTransport {
                            target: successor.block,
                            substitutions,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut registers = if blocks.iter().all(Vec::is_empty) {
            Vec::new()
        } else {
            function
                .virtual_registers
                .iter()
                .map(|register| register.id)
                .collect()
        };
        registers.sort_unstable();
        Self { blocks, registers }
    }

    pub(super) fn incoming_argument(
        &self,
        function_index: usize,
        edge: &EdgeTransport,
        destination: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, LivenessError> {
        let position = self
            .registers
            .partition_point(|register| *register < destination);
        // Check at the same query boundary as full reconstruction. Unused
        // duplicates must not become a new eager rejection; live ones still do.
        if self.registers.get(position) != Some(&destination)
            || self.registers.get(position + 1) == Some(&destination)
        {
            return Err(LivenessError::FunctionMismatch {
                function: function_index,
            });
        }
        // Substitutions are simultaneous, not transitive. A mapped argument can
        // itself be a parameter; it is returned once, never chased through here.
        Ok(edge
            .substitutions
            .binary_search_by_key(&destination, |(parameter, _)| *parameter)
            .map_or(destination, |position| edge.substitutions[position].1))
    }
}

fn successors(terminator: &SelectedTerminator) -> impl Iterator<Item = &SelectedSuccessor> {
    let edges = match terminator {
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => [Some(when_nonzero), Some(when_zero)],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => [Some(when_less), Some(when_not_less)],
        SelectedTerminator::Jump { successor, .. } => [Some(successor), None],
        SelectedTerminator::Return { .. }
        | SelectedTerminator::Crash { .. }
        | SelectedTerminator::HostedExitProcess { .. } => [None, None],
    };
    edges.into_iter().flatten()
}

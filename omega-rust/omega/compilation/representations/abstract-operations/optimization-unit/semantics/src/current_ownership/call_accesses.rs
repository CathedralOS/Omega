//! Current ordinary callees refine the storage a call can disturb.
//!
//! A mutable formal grants authority, not evidence that every byte is written.
//! Reconstruct writes from the current operation inventory and borrowed-region
//! origins, then substitute the exact actual projection. Never use the immutable
//! Terminal snapshot or a source-language receiver spelling. Missing callees,
//! recursive closures, opaque effects and unresolved local origins retain the
//! ordinary whole-input fence. This proves interference only, not loan closure.

use super::address_joins::borrowed_regions::{BorrowedRegions, Projection};
use super::storage_regions::written_or_exclusive_regions;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration};

#[derive(Clone)]
struct ParameterMutation {
    position: usize,
    projection: Projection,
}

#[derive(Clone)]
struct FunctionMutations {
    parameters: usize,
    writes: Vec<ParameterMutation>,
}

/// Prepared once for the current unit; rewritten units reconstruct this table.
#[derive(Default)]
pub(crate) struct CallAccesses {
    functions: BTreeMap<MachineId, Option<FunctionMutations>>,
}

impl CallAccesses {
    pub(crate) fn new(
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Self {
        let mut result = Self::default();
        for machine in functions.keys() {
            result.reconstruct(*machine, functions, types);
        }
        result
    }

    fn reconstruct(
        &mut self,
        machine: MachineId,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) {
        if self.functions.contains_key(&machine) {
            return;
        }
        // An in-progress recursive closure has no proved summary yet.
        self.functions.insert(machine, None);
        let Some(function) = functions.get(&machine) else {
            return;
        };
        let origins = BorrowedRegions::origins(function);
        let mut writes = Vec::<ParameterMutation>::new();
        for node in function.blocks.iter().flat_map(|block| &block.nodes) {
            if let O::CallStructural { callee, .. } = &node.operation {
                self.reconstruct(*callee, functions, types);
                if !self.functions.get(callee).is_some_and(Option::is_some) {
                    return;
                }
            }
            if matches!(
                node.operation,
                O::CallDynamic { .. }
                    | O::Call { .. }
                    | O::AtomicEvent { .. }
                    | O::PortWrite { .. }
            ) {
                return;
            }
            if let O::BoundaryCall {
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                ..
            } = &node.operation
                && (structural_arguments
                    .iter()
                    .any(|argument| argument.access == StructuralAccess::Owned)
                    || !completion_claim_sources.is_empty()
                    || !completion_receipts.is_empty())
            {
                // Service reach grants no storage authority. Direct borrowed
                // actuals have the declared access and exact referent projection
                // checked by the boundary-call contract, so the ordinary write
                // inventory bounds their effects. Owned carriers and completion
                // custody need a separate mapping of carried authority.
                return;
            }
            if let O::Return {
                cleanup_actions, ..
            }
            | O::ReturnUnit {
                cleanup_actions, ..
            } = &node.operation
                && cleanup_actions.iter().any(|action| {
                    matches!(
                        action,
                        terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_)
                    )
                })
            {
                // Nominal cleanup executes code after the result is observed;
                // absence of an explicit store node does not make it read-only.
                return;
            }
            for (place, region) in self.written_regions(function, types, &node.operation) {
                let mut projection = Projection::new(&region.path);
                projection.indirect |= region.indirect;
                for region in origins.resolve(place, projection) {
                    let Some(position) = function
                        .structural_parameters
                        .iter()
                        .position(|parameter| Some(parameter.place) == region.root)
                    else {
                        // A local carrier can itself contain borrowed storage.
                        // Absence from the formal roster is not disjointness.
                        return;
                    };
                    if function.structural_parameters[position].access == StructuralAccess::Owned {
                        // Copy/move and carried-reference effects require their
                        // own custody mapping; do not reinterpret owned inputs.
                        return;
                    }
                    if !writes.iter().any(|existing| {
                        existing.position == position
                            && existing.projection.path == region.projection.path
                            && existing.projection.exact == region.projection.exact
                            && existing.projection.indirect == region.projection.indirect
                    }) {
                        writes.push(ParameterMutation {
                            position,
                            projection: region.projection,
                        });
                    }
                }
            }
        }
        self.functions.insert(
            machine,
            Some(FunctionMutations {
                parameters: function.structural_parameters.len(),
                writes,
            }),
        );
    }

    pub(super) fn written_regions(
        &self,
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        operation: &O,
    ) -> Vec<(PlaceId, terminal_semantics::StorageRegion)> {
        if let O::CallStructural {
            callee,
            structural_arguments,
            dynamic_arguments,
            claim_transfers,
            ..
        } = operation
            && dynamic_arguments.is_empty()
            && claim_transfers.is_empty()
            && let Some(Some(summary)) = self.functions.get(callee)
            && summary.parameters == structural_arguments.len()
        {
            return summary
                .writes
                .iter()
                .map(|write| {
                    let actual = &structural_arguments[write.position];
                    let projection = Projection::new(&actual.path).then(&write.projection);
                    (
                        actual.place,
                        terminal_semantics::StorageRegion {
                            path: projection.path,
                            indirect: projection.indirect,
                        },
                    )
                })
                .collect();
        }
        written_or_exclusive_regions(function, types, operation)
    }
}

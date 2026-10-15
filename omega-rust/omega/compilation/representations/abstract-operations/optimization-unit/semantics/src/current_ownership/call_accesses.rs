//! Current ordinary callees refine the storage a call can disturb.
//!
//! A mutable formal grants authority, not evidence that every byte is written.
//! Reconstruct writes from the current operation inventory and borrowed-region
//! origins, then substitute the exact actual projection. Never use the immutable
//! Terminal snapshot or a source-language receiver spelling. Missing callees,
//! recursive closures, opaque effects and unresolved local origins retain the
//! ordinary whole-input fence. This proves interference only, not loan closure.

use super::address_joins::borrowed_regions::{BorrowedRegions, Projection};
use super::storage_regions::{storage_cannot_retain_loan, written_or_exclusive_regions};
use crate::unit_validation::operation_contracts::structural_source_contract;
use crate::unit_validation::structural_catalog::resolve_structural_path;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

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
pub(crate) struct CallAccesses<'unit> {
    functions: BTreeMap<MachineId, Option<FunctionMutations>>,
    /// Host declarations a descriptor replay consults at each boundary
    /// completion; boundary bodies are not part of the unit.
    boundaries: Vec<&'unit BoundaryMachineDeclaration>,
    pub(super) references: super::references::call_effects::ReferenceCallEffects,
}

impl<'unit> CallAccesses<'unit> {
    pub(crate) fn new(
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        boundaries: impl IntoIterator<Item = &'unit BoundaryMachineDeclaration>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Self {
        let boundaries = boundaries.into_iter().collect::<Vec<_>>();
        let references = super::references::call_effects::ReferenceCallEffects::new(
            functions,
            &boundaries,
            types,
        );
        let mut result = Self {
            functions: BTreeMap::new(),
            boundaries,
            references,
        };
        let mut closed_storage = BTreeMap::new();
        for machine in functions.keys() {
            result.reconstruct(*machine, functions, types, &mut closed_storage);
        }
        result
    }

    pub(crate) fn boundaries(&self) -> &[&'unit BoundaryMachineDeclaration] {
        &self.boundaries
    }

    fn reconstruct(
        &mut self,
        machine: MachineId,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        closed_storage: &mut BTreeMap<StructuralTypeId, bool>,
    ) {
        if self.functions.contains_key(&machine) {
            return;
        }
        // An in-progress recursive closure has no proved summary yet.
        self.functions.insert(machine, None);
        let Some(function) = functions.get(&machine) else {
            return;
        };
        let origins = BorrowedRegions::origins(function, functions, &self.boundaries, types);
        let mut writes = Vec::<ParameterMutation>::new();
        for node in function.block_nodes() {
            if let O::CallStructural { callee, .. } = &node.operation {
                let Some(machine) = callee.machine() else {
                    return;
                };
                self.reconstruct(machine, functions, types, closed_storage);
                if !self.functions.get(&machine).is_some_and(Option::is_some) {
                    return;
                }
            }
            if matches!(
                node.operation,
                O::Call { .. } | O::AtomicEvent { .. } | O::PortWrite { .. }
            ) {
                return;
            }
            if let O::BoundaryCall {
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                ..
            } = &node.operation
                && (structural_arguments.iter().any(|argument| {
                    argument.access == StructuralAccess::Owned
                        || !structural_source_contract(function, argument.place).is_some_and(
                            |source| {
                                argument_storage_is_closed(
                                    types,
                                    source.structural_type,
                                    &argument.path,
                                    argument.access,
                                    closed_storage,
                                )
                            },
                        )
                }) || !completion_claim_sources.is_empty()
                    || !completion_receipts.is_empty())
            {
                // A borrowed record may contain a mutable reference: its
                // boundary callee can then write outside the record's bytes.
                // Exact outer access alone cannot bound that carried authority.
                // Closed referents and direct view payloads use the ordinary
                // backing inventory; other carriers and completion custody
                // require an additional authority mapping.
                return;
            }
            if node
                .operation
                .cleanup_rosters()
                .flat_map(|(_, actions)| actions)
                .any(|action| {
                    matches!(
                        action,
                        terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_)
                    )
                })
            {
                // Nominal cleanup executes code after outgoing values are observed;
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
                    let projection = region.projection.without_runtime_coordinates();
                    if !writes.iter().any(|existing| {
                        existing.position == position && existing.projection == projection
                    }) {
                        writes.push(ParameterMutation {
                            position,
                            projection,
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
            claim_transfers,
            ..
        } = operation
            && claim_transfers.is_empty()
            && let Some(machine) = callee.machine()
            && let Some(Some(summary)) = self.functions.get(&machine)
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

pub(crate) fn argument_storage_is_closed(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    source: StructuralTypeId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
    established: &mut BTreeMap<StructuralTypeId, bool>,
) -> bool {
    let Some(selected) = resolve_structural_path(types, source, path) else {
        return false;
    };
    // A direct descriptor is already a backing alias in BorrowedRegions.
    // A descriptor nested inside a record is not that record's own backing.
    if path.is_empty() && access != StructuralAccess::Owned {
        match types.get(&selected).map(|declaration| &declaration.shape) {
            Some(StructuralTypeShape::ByteSequence(carrier)) if carrier.is_borrowed() => {
                return true;
            }
            Some(StructuralTypeShape::ElementView { element, .. }) => {
                return storage_cannot_retain_loan(types, *element, established);
            }
            _ => {}
        }
    }
    storage_cannot_retain_loan(types, selected, established)
}

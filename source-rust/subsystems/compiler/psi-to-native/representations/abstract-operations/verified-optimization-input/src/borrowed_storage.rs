//! The admitted boundary and its reachable, target-bound types determine host
//! borrow geometry. Keep those inputs before optimization can remove a backing
//! local or a call. Both execution modes and object replay resolve this subject.

use abstract_operations::{AbstractOperationPlan, StructuralTypeCatalog};
use calling_conventions::terminal_layout::NativeGeometry;
use runtime_abi::NativeBorrowedStorageLayouts;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{BoundaryMachineResult, StructuralAccess, StructuralTypeShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedStorageInputs {
    catalog: StructuralTypeCatalog,
    roots: BTreeSet<StructuralTypeId>,
}

impl BorrowedStorageInputs {
    pub fn of(plan: &AbstractOperationPlan) -> Self {
        let results: BTreeSet<_> = plan
            .boundary_machines
            .iter()
            .filter_map(|machine| match &machine.result {
                BoundaryMachineResult::Structural(result) => Some(result.structural_type),
                _ => None,
            })
            .collect();
        let mut roots = results.clone();
        // A returned reference has a carrier recipe and a separate recipe for
        // its provider-owned backing.
        roots.extend(
            plan.structural_types
                .iter()
                .filter(|ty| results.contains(&ty.id))
                .filter_map(|ty| match &ty.shape {
                    StructuralTypeShape::Reference { referent, .. } => Some(*referent),
                    _ => None,
                }),
        );
        let mut needs_backing = false;
        for parameter in plan
            .boundary_machines
            .iter()
            .flat_map(|machine| &machine.structural_parameters)
        {
            roots.insert(parameter.structural_type);
            needs_backing |= matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            );
        }
        if needs_backing {
            for function in &plan.functions {
                roots.extend(
                    function
                        .structural_parameters
                        .iter()
                        .chain(function.block_structural_parameters())
                        .filter(|parameter| parameter.access == StructuralAccess::Owned)
                        .map(|parameter| parameter.structural_type),
                );
                roots.extend(
                    function
                        .operations()
                        .filter_map(|operation| operation.materialized_structural_result())
                        .map(|(_, result)| result.structural_type),
                );
            }
        }
        Self {
            catalog: plan.structural_types.clone(),
            roots,
        }
    }

    pub fn layouts(&self) -> Result<NativeBorrowedStorageLayouts, &'static str> {
        if self.roots.is_empty() {
            return Ok(NativeBorrowedStorageLayouts::default());
        }
        let geometry = NativeGeometry::with_owned_sequence_bounds(
            &self.catalog,
            self.catalog.owned_sequence_bounds(),
        )
        .map_err(|_| "borrowed storage declarations have non-dense structural identities")?;
        // A reachable opaque type may have no native representation. Consumers
        // requiring that recipe must refuse the borrow when its row is absent.
        let layouts = self
            .roots
            .iter()
            .filter_map(|root| geometry.borrowed_storage(*root))
            .collect();
        NativeBorrowedStorageLayouts::seal(layouts).map_err(|error| error.reason)
    }
}

//! The admitted boundary and its reachable, target-bound types determine host
//! borrow geometry. Keep those inputs before optimization can remove a backing
//! local or a call. Both execution modes and object replay resolve this subject.

use abstract_operations::{AbstractOperationPlan, StructuralTypeCatalog};
use calling_conventions::terminal_layout::NativeGeometry;
use runtime_abi::NativeBorrowedStorageLayouts;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{BoundaryMachineResult, StructuralAccess, StructuralTypeShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedStorageInputs {
    catalog: StructuralTypeCatalog,
    roots: Vec<StructuralTypeId>,
}

impl BorrowedStorageInputs {
    pub fn of(plan: &AbstractOperationPlan) -> Self {
        let mut results: Vec<_> = plan
            .boundary_machines
            .iter()
            .filter_map(|machine| match &machine.result {
                BoundaryMachineResult::Structural(result) => Some(result.structural_type),
                _ => None,
            })
            .collect();
        results.sort_unstable();
        let mut roots = Vec::new();
        // A returned reference has a carrier recipe and a separate recipe for
        // its provider-owned backing.
        if !results.is_empty() {
            roots.extend(
                plan.structural_types
                    .iter()
                    .filter(|ty| results.binary_search(&ty.id).is_ok())
                    .filter_map(|ty| match &ty.shape {
                        StructuralTypeShape::Reference { referent, .. } => Some(*referent),
                        _ => None,
                    }),
            );
            roots.append(&mut results);
        }
        let mut needs_backing = false;
        for parameter in plan
            .boundary_machines
            .iter()
            .flat_map(|machine| &machine.structural_parameters)
        {
            roots.push(parameter.structural_type);
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
        roots.sort_unstable();
        roots.dedup();
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

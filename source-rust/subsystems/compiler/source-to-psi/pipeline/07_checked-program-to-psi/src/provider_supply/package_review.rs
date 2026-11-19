//! Close the package's source coordinates before target planning. The review
//! inventory includes every Build binding for this deployment, including rows
//! absent from a runnable entry's call graph.
use crate::{AcceptedTerminalSource, DeclaredBoundaryRequirement};
use diagnostics::Diagnostic;
use evaluation_receipts::NativeBindingMaterialization;
use semantic_vocabulary::{BoundaryMachineId, MachineId};

pub struct SourcePackageReviewRequirements {
    pub numeric: Result<Vec<BoundaryMachineId>, Vec<Diagnostic>>,
    pub bindings: Result<Vec<(BoundaryMachineId, NativeBindingMaterialization)>, Vec<Diagnostic>>,
}

impl AcceptedTerminalSource {
    pub fn source_package_review_requirements(
        &self,
        target: &str,
        supplies: &[crate::provider_choice_inputs::NativeBuildSupply],
        declared: &[DeclaredBoundaryRequirement],
        provider_bodies: impl IntoIterator<Item = MachineId>,
    ) -> SourcePackageReviewRequirements {
        let unavailable = |what: &str| {
            vec![Diagnostic::error(format!(
                "package review cannot retain {what} from the accepted declaration"
            ))]
        };
        let numeric = self
            .publish_package_review_numeric_requirements(provider_bodies)
            .map_err(unavailable);
        let bindings = (|| {
            let mut supplies = supplies
                .iter()
                .filter(|supply| supply.selection.target.canonical_name() == target)
                .peekable();
            if supplies.peek().is_none() {
                return Ok(Vec::new());
            }
            let mut declared = declared.iter().collect::<Vec<_>>();
            let key = |row: &&DeclaredBoundaryRequirement| {
                (
                    row.slot.arena_index(),
                    row.slot.generation(),
                    row.requirement.arena_index(),
                    row.requirement.generation(),
                )
            };
            declared.sort_unstable_by_key(key);
            if declared
                .windows(2)
                .any(|pair| key(&pair[0]) == key(&pair[1]))
            {
                return Err(unavailable(
                    "one accepted declaration for a native requirement",
                ));
            }
            supplies
                .map(|supply| {
                    let slot = supply.selection.subject;
                    let requirement = supply.selection.requirement;
                    let index = declared
                        .binary_search_by_key(
                            &(
                                slot.arena_index(),
                                slot.generation(),
                                requirement.arena_index(),
                                requirement.generation(),
                            ),
                            key,
                        )
                        .map_err(|_| {
                            unavailable("the native binding's exact accepted requirement")
                        })?;
                    Ok((declared[index].boundary, supply.materialization()))
                })
                .collect()
        })();
        SourcePackageReviewRequirements { numeric, bindings }
    }
}

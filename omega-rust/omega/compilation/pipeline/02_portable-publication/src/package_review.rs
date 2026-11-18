//! Close the package's source coordinates before target planning. The review
//! inventory includes every Build binding for this deployment, including rows
//! absent from a runnable entry's call graph.
use backend_plan::package_review::PackageReviewRequirements;
use diagnostics::Diagnostic;
use semantic_vocabulary::MachineId;
use source_compiler::AcceptedTerminalSource;

pub fn package_review_requirements(
    accepted: &AcceptedTerminalSource,
    target: &str,
    supplies: &[provider_choices::NativeBuildSupply],
    provider_bodies: impl IntoIterator<Item = MachineId>,
) -> PackageReviewRequirements {
    let unavailable = |what: &str| {
        vec![Diagnostic::error(format!(
            "package review cannot retain {what} from the accepted declaration"
        ))]
    };
    let numeric = accepted
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
        let mut declared = accepted
            .boundary_requirements_declared()
            .map_err(|reason| vec![reason])?;
        let key = |row: &source_compiler::DeclaredBoundaryRequirement| {
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
                    .map_err(|_| unavailable("the native binding's exact accepted requirement"))?;
                Ok((declared[index].boundary, supply.materialization()))
            })
            .collect()
    })();
    PackageReviewRequirements { numeric, bindings }
}

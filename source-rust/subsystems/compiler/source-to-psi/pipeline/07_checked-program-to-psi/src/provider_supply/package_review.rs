//! Close the package's source coordinates before target planning. The review
//! inventory includes every Build binding for this deployment, including rows
//! absent from a runnable entry's call graph.
use super::SourcePackageReviewRequirements;
use crate::{AcceptedTerminalSource, DeclaredBoundaryRequirement};
use diagnostics::Diagnostic;
use semantic_vocabulary::MachineId;

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
            let mut bindings = Vec::new();
            for supply in supplies {
                let before = bindings.len();
                for row in declared
                    .iter()
                    .filter(|row| row.slot == supply.selection.subject)
                {
                    let binding = supply
                        .materialization_for(self.source(), row.requirement)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    if let Some(binding) = binding {
                        bindings.push((row.boundary, binding));
                    }
                }
                if bindings.len() == before {
                    let selection = &supply.selection;
                    let name = self.source().symbols.display_path(selection.requirement, "::");
                    let diagnostic = Diagnostic::error(format!(
                        "no accepted application of native requirement `{name}` for `{target}` during package review"
                    )).with_source_span(selection.source_span);
                    return Err(vec![diagnostic]);
                }
            }
            Ok(bindings)
        })();
        SourcePackageReviewRequirements { numeric, bindings }
    }
}

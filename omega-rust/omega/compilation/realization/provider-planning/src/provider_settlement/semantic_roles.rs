//! A program's exit API must belong to the package that declares it and use an
//! implementation that actually exits. Save the declared identity before
//! publishing Terminal Psi, then compare it with the selected implementation
//! without reading source again.

use crate::provider_planning::trait_name_text;
use crate::{ProviderPlanProvenance, ProviderSchemaDeclaration, SelectedProviderReviewProvenance};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::boundary_completions::completion_declaration;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;
use symbols::SymbolHandle;

/// Identify the exact declared exit promise. Human-readable paths are useful
/// in diagnostics, but cannot distinguish declarations from different packages.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletionIdentity {
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    owner: SymbolHandle,
    requirement: SymbolHandle,
    completion: semantic_vocabulary::BoundaryCompletion,
}

impl From<&checked_trees::typed_trees::BoundaryCompletionDeclaration> for CompletionIdentity {
    fn from(declaration: &checked_trees::typed_trees::BoundaryCompletionDeclaration) -> Self {
        Self {
            package: declaration.package,
            owner: declaration.owner,
            requirement: declaration.requirement,
            completion: declaration.completion,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedCompletionRole {
    declaration: CompletionIdentity,
    eligible_rows: Vec<usize>,
}

impl PreparedCompletionRole {
    pub(crate) fn capture(
        typed: &TypedTrees,
        plan: &ProviderPlan,
        provenance: &ProviderPlanProvenance,
    ) -> Option<Self> {
        let ProviderSchemaDeclaration::BoundaryTrait(owner) = provenance.schema else {
            return None;
        };
        let package = plan.provider_type_package_identity?;
        if trait_name_text(&plan.schema) != "Console"
            || plan.schema.declaration.trait_package_identity != Some(package)
            || plan.origin_package_identity != Some(package)
            || typed.symbols.symbol_package_identity(owner) != Some(package)
        {
            return None;
        }
        let declaration = completion_declaration(typed, owner, package)?;
        let eligible_rows =
            plan.rows
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    provenance.row_requirements.get(*index) == Some(&declaration.requirement)
                        && provenance
                            .row_realizations
                            .get(*index)
                            .is_some_and(|realization| {
                                realization.authored().and_then(|symbol| {
                                    typed.symbols.symbol_package_identity(symbol)
                                }) == Some(package)
                            })
                })
                .map(|(index, _)| index)
                .collect();
        Some(Self {
            declaration: CompletionIdentity::from(&declaration),
            eligible_rows,
        })
    }
}

pub(crate) struct PreparedCompletionRoles {
    proposals: Vec<CompletionIdentity>,
}

impl PreparedCompletionRoles {
    pub(crate) fn capture(typed: &TypedTrees) -> Self {
        Self {
            proposals: typed
                .boundary_completion_declarations
                .iter()
                .map(|(_, declaration)| CompletionIdentity::from(declaration))
                .collect(),
        }
    }

    /// Confirm that each selected exit API has the implementation its package
    /// declared. A hosted exit API that this target does not select needs no
    /// implementation here, as in a library also built for firmware.
    pub(crate) fn validate(
        &self,
        selected: &[(
            SelectedProviderReviewProvenance,
            Option<PreparedCompletionRole>,
        )],
    ) -> Result<(), Vec<Diagnostic>> {
        let mut declarations = Vec::new();
        for (selected, role) in selected {
            let Some(role) = role else { continue };
            let external_rows = role
                .eligible_rows
                .iter()
                .filter(|index| {
                    selected
                        .row_external_completion_leaves
                        .get(**index)
                        .is_some_and(|leaves| !leaves.is_empty())
                })
                .count();
            if external_rows > 1 {
                return Err(vec![Diagnostic::error(
                    "completion role discovery has multiple realizations for one source requirement",
                )]);
            }
            if external_rows == 1 {
                declarations.push(&role.declaration);
            }
        }
        if declarations.len() > 1 {
            return Err(vec![Diagnostic::error(
                "completion role discovery has multiple selected package-owned Console declarations",
            )]);
        }
        let proposals = self
            .proposals
            .iter()
            .filter(|declaration| {
                selected.iter().any(|(provider, _)| {
                    matches!(provider.provider.schema,
                    ProviderSchemaDeclaration::BoundaryTrait(owner) if owner == declaration.owner)
                })
            })
            .collect::<Vec<_>>();
        if proposals.len() != declarations.len()
            || proposals
                .iter()
                .any(|proposal| !declarations.contains(proposal))
        {
            return Err(vec![Diagnostic::error(
                "completion role discovery does not match the exact source declaration and selected realization",
            )]);
        }
        Ok(())
    }
}

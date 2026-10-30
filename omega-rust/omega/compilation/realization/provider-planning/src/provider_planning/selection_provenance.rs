//! Source correspondence adapter for the shared whole-provider selector.

use crate::{DerivedProviderPlan, ProviderPlanProvenance};
use component_description::VerifiedComponent;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::ProviderPlan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionProvenance {
    BuildOverride(Vec<crate::ProviderSelection>),
    TargetDefault(Vec<crate::ProviderSelection>),
    UniqueCoveringCandidate,
    NativeBuild,
}

impl ProviderSelectionProvenance {
    /// Composition policy is shared with canonical publication.
    pub fn composition_mode(&self) -> Result<crate::CompositionMode, String> {
        super::selection_declarations::project_provenance(self.clone()).composition_mode()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderPlanWithProvenance {
    pub derived: DerivedProviderPlan,
    pub selected_by: ProviderSelectionProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderReviewProvenance {
    pub plan: ProviderPlan,
    pub provider: ProviderPlanProvenance,
    pub selected_by: ProviderSelectionProvenance,
    /// Host and placement execution identity for each provider row. Numeric
    /// rows carry None: their meaning lives in Psi's external declarations.
    ///
    /// Selection initially leaves this empty because exact execution is not
    /// settled until after checking. The compiler must replace it with one
    /// row-aligned entry per plan row before publishing `CheckedCompilation`.
    /// `Some` is reserved for compiler-intrinsic rows whose selected
    /// execution has a closed identity; all other rows retain `None`.
    pub row_compiler_intrinsic_executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>,
    /// Exact external leaves reached on every checked completion path. This
    /// does not turn an authored adapter into a compiler intrinsic.
    pub row_external_completion_leaves: Vec<Vec<SymbolHandle>>,
}

pub fn selected_provider_plan_facts(
    context: crate::ProviderPlanningContext<'_>,
    selected: Vec<SelectedProviderPlanWithProvenance>,
) -> Result<
    (
        effects::SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    selected_provider_plan_facts_with_independent_components(context, selected, &[])
}

/// Keep rootless Check/review on exact invocation declarations. No canonical
/// owner is required here; publication projects its own closed identities.
pub(crate) fn selected_provider_plan_facts_with_independent_components(
    context: crate::ProviderPlanningContext<'_>,
    selected: Vec<SelectedProviderPlanWithProvenance>,
    independent_components: &[VerifiedComponent],
) -> Result<
    (
        effects::SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    let mut choices = Vec::with_capacity(selected.len());
    for selected in selected {
        let mut provenance =
            super::selection_declarations::project_provenance(selected.selected_by);
        match &mut provenance {
            provider_selection::SelectionProvenance::BuildOverride(declarations)
            | provider_selection::SelectionProvenance::TargetDefault(declarations) => {
                for declaration in declarations {
                    let source = declaration.origin.clone();
                    super::selection_declarations::retain_family_replay(
                        context,
                        &source,
                        declaration,
                    );
                }
            }
            provider_selection::SelectionProvenance::UniqueCoveringCandidate
            | provider_selection::SelectionProvenance::NativeBuild => {}
        }
        choices.push(provider_selection::SelectedProvider {
            candidate: super::selection_declarations::source_candidate(selected.derived),
            selected_by: provenance,
        });
    }
    let (facts, selected) = provider_selection::admit(choices, independent_components)?;
    let provenance = selected
        .into_iter()
        .map(|selected| SelectedProviderReviewProvenance {
            plan: selected.candidate.plan,
            provider: selected.candidate.owner,
            selected_by: super::selection_declarations::restore_provenance(selected.selected_by),
            row_compiler_intrinsic_executions: Vec::new(),
            row_external_completion_leaves: Vec::new(),
        })
        .collect();
    Ok((facts, provenance))
}

//! Choose whole providers, then admit their complete correspondence and composition.
//!
//! Source consumers may perform cycle/ABI preparation between these operations.
//! The chosen records own the exact candidates and declaration choices, so the
//! later admission cannot reinterpret indices against a different population.

use crate::declarations::same_declaration;
use crate::independent_components::IndependentComponentJoin;
use crate::{
    CompositionMode, ProviderCandidate, ProviderDeclaration, ProviderSubject, SelectedProvider,
    SelectionProvenance,
};
use diagnostics::Diagnostic;

pub fn choose<Declaration: Clone + Eq, Owner, Origin: Clone + Eq>(
    candidates: Vec<ProviderCandidate<Declaration, Owner>>,
    target: target::TargetProfile,
    defaults: &[ProviderDeclaration<Declaration, Origin>],
    overrides: &[ProviderDeclaration<Declaration, Origin>],
) -> Result<Vec<SelectedProvider<Declaration, Owner, Origin>>, Vec<Diagnostic>> {
    let plans = candidates
        .iter()
        .map(|candidate| &candidate.plan)
        .collect::<Vec<_>>();
    let selected =
        crate::selection::select_provider_plan_indices(&plans, target, defaults, overrides)?;
    let mut candidates = candidates.into_iter().map(Some).collect::<Vec<_>>();
    let mut chosen = Vec::with_capacity(selected.len());
    for selected in selected {
        let candidate = candidates
            .get_mut(selected.candidate)
            .and_then(Option::take)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "provider choice did not retain one exact candidate",
                )]
            })?;
        let selected_by = if candidate.native_build {
            SelectionProvenance::NativeBuild
        } else {
            selected.selected_by
        };
        chosen.push(SelectedProvider {
            candidate,
            selected_by,
        });
    }
    Ok(chosen)
}

/// Preserve source admission order: family completeness, composition closure,
/// exact declaration correspondence, then immutable selected facts.
pub fn admit<'a, Declaration: Clone + Eq, Owner, Origin: Clone + Eq>(
    mut selected: Vec<SelectedProvider<Declaration, Owner, Origin>>,
    independent_components: impl IntoIterator<Item = &'a component_description::VerifiedComponent>,
) -> Result<
    (
        effects::SelectedProviderPlanFacts,
        Vec<SelectedProvider<Declaration, Owner, Origin>>,
    ),
    Vec<Diagnostic>,
> {
    selected.sort_by(|left, right| left.candidate.plan.selection_order(&right.candidate.plan));
    let mut diagnostics = check_operator_families(&selected);
    let mut independent_join = IndependentComponentJoin::new(independent_components);
    for selected in &selected {
        let plan = &selected.candidate.plan;
        let correspondence = &selected.candidate.declarations;
        let composition = match selected.selected_by.composition_mode() {
            Ok(mode) => mode,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(reason));
                continue;
            }
        };
        if composition == CompositionMode::Independent
            && let Err(diagnostic) = independent_join.realize(plan)
        {
            diagnostics.push(diagnostic);
        }
        let declarations = match &selected.selected_by {
            SelectionProvenance::BuildOverride(declarations)
            | SelectionProvenance::TargetDefault(declarations) => declarations,
            SelectionProvenance::UniqueCoveringCandidate | SelectionProvenance::NativeBuild => {
                continue;
            }
        };
        if declarations.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "selected provider plan `{}` has an authored selection origin without a declaration", plan.name,
            )));
        }
        for declaration in declarations {
            diagnostics.extend(declaration.subject.owner_errors());
            if let Err(reason) = &declaration.provider_type.declaration {
                diagnostics.push(Diagnostic::error(reason.clone()));
            }
            if !declaration.subject.selects_schema(
                &correspondence.schema,
                plan.schema
                    .identity_text(plan.schema.declaration.trait_name),
                plan.schema.root_application_identity(),
            ) || !correspondence.provider.as_ref().is_some_and(|provider| {
                same_declaration(&declaration.provider_type.declaration, provider)
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "selected provider plan `{}` has a selection declaration outside its exact schema, provider, or selecting-machine provenance", plan.name,
                )));
            }
        }
    }
    diagnostics.extend(independent_join.finish());
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let plans = selected
        .iter()
        .map(|selected| selected.candidate.plan.clone())
        .collect::<Vec<_>>();
    let facts = effects::SelectedProviderPlanFacts::from_selected_plans(plans.clone())
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    if facts.plans() != plans
        || plans.iter().any(|plan| {
            facts
                .plan_by_exact_evidence(plan.report_fingerprint(), plan)
                .is_none()
        })
    {
        return Err(vec![Diagnostic::error(
            "selected provider semantic facts do not retain the exact plans aligned with provenance",
        )]);
    }
    Ok((facts, selected))
}

fn same_family<Declaration: PartialEq, Origin>(
    left: &ProviderDeclaration<Declaration, Origin>,
    right: &ProviderDeclaration<Declaration, Origin>,
) -> bool {
    left.subject.same_declaration_as(&right.subject)
        && same_declaration(
            &left.provider_type.declaration,
            &right.provider_type.declaration,
        )
        && left.provider_type.application_identity == right.provider_type.application_identity
}

fn check_operator_families<Declaration: Clone + Eq, Owner, Origin: Clone + Eq>(
    selected: &[SelectedProvider<Declaration, Owner, Origin>],
) -> Vec<Diagnostic> {
    let mut families: Vec<&ProviderDeclaration<Declaration, Origin>> = Vec::new();
    for selected in selected {
        for declaration in selected.selected_by.declarations() {
            if matches!(
                declaration.subject,
                ProviderSubject::BoundaryOperatorFamily(_)
            ) && !families
                .iter()
                .any(|existing| same_family(existing, declaration))
            {
                families.push(declaration);
            }
        }
    }
    let mut diagnostics = Vec::new();
    for declaration in families {
        let ProviderSubject::BoundaryOperatorFamily(family) = &declaration.subject else {
            continue;
        };
        diagnostics.extend(family.preparation_errors.iter().cloned());
        diagnostics.extend(declaration.subject.owner_errors());
        for coordinate in &family.coordinates {
            let realized = selected
                .iter()
                .filter(|selected| {
                    same_declaration(
                        &selected.candidate.declarations.schema,
                        &coordinate.declaration,
                    ) && selected
                        .candidate
                        .plan
                        .schema
                        .identity_text(selected.candidate.plan.schema.declaration.trait_name)
                        == coordinate.requirement_identity
                        && selected
                            .candidate
                            .declarations
                            .provider
                            .as_ref()
                            .is_some_and(|provider| {
                                same_declaration(provider, &declaration.provider_type.declaration)
                            })
                        && selected
                            .selected_by
                            .declarations()
                            .iter()
                            .any(|retained| same_family(retained, declaration))
                })
                .count();
            if realized != 1 {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary-operator family `{}` selection resolves coordinate `{}` to {realized} selected plans of provider `{}`; every coordinate is realized by exactly one",
                    family.authored_path, coordinate.requirement_identity, declaration.provider_type.authored_path,
                )));
            }
        }
    }
    diagnostics
}

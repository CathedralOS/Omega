//! The requirement view resolves one `Owner::name` intrinsic signature
//! identically whether it is spelled as a named boundary operator or as a
//! public receiver-free top-level boundary requirement: same realization,
//! same diagnostic label, same selected execution identity, and the same
//! staged rewrite from either use fact.

use crate::selected_dispatch::float_intrinsic::execution_identities::selected_compiler_intrinsic_realization;
use crate::selected_dispatch::float_intrinsic::intrinsic_resolution::{
    SelectedIntrinsicUse, resolve_selected_float_intrinsic_call,
};
use crate::selected_dispatch::float_intrinsic::named_float_realizations::named_float_realization_for;
use crate::selected_dispatch::float_intrinsic::{
    NamedFloatRealization, SelectedCompilerIntrinsicRealization,
};
use checked_trees::CheckedTrees;
use effects::provider_plan::ProviderPlan;
use numerics::literals::FloatFormat;
use provider_planning::{IntrinsicRequirement, IntrinsicRequirementKind, ProviderPlanDerivation};
use std::sync::Arc;

const OPERATOR_SOURCE: &str = r#"
    pub data F32 {}
    pub boundary operator F32::negate(value: f32) -> f32;
    pub boundary operator F32::minimum(left: f32, right: f32) -> f32;

    data FloatProvider {}
    machine FloatProvider::negate(value: f32) -> f32
    satisfies F32::negate
    via Binding::CompilerIntrinsic;
    machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;

    machine run() -> f32 {
        let flipped: f32 = F32::negate(1.0f32);
        transition { _ -> (F32::minimum(flipped, 2.0f32)) }
    }
"#;

fn requirement_source() -> String {
    OPERATOR_SOURCE.replace("pub boundary operator", "pub boundary requirement")
}

fn checked_with_plans(source: &str) -> (CheckedTrees, Vec<ProviderPlan>) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check");
    (checked, plans)
}

fn bound(
    checked: CheckedTrees,
    plans: &[ProviderPlan],
) -> (Arc<CheckedTrees>, effects::SelectedProviderPlanFacts) {
    let names = plans
        .iter()
        .map(|plan| plan.name.clone())
        .collect::<Vec<_>>();
    let selected = effects::SelectedProviderPlanFacts::from_selection(plans, &names)
        .expect("select every plan");
    let binding = provider_planning::bind_selected_provider_plan_facts(
        &Arc::new(checked),
        plans,
        selected,
        &[],
        &[],
    )
    .expect("bind the selected plans onto the checked program");
    let (program, selected, _) = binding.into_parts();
    (program, selected)
}

fn view<'a>(checked: &'a CheckedTrees, name: &str) -> IntrinsicRequirement<'a> {
    let operator = checked.typed.operators().iter().find(|operator| {
        checked
            .typed
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .eq(["F32", name])
    });
    match operator {
        Some(operator) => {
            IntrinsicRequirement::from_operator(&checked.typed, operator).expect("operator view")
        }
        None => {
            let machine = checked
                .typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == format!("F32::{name}"))
                .expect("requirement machine");
            IntrinsicRequirement::from_requirement(&checked.typed, machine)
                .expect("requirement view")
        }
    }
}

#[test]
fn both_spellings_resolve_the_same_realization_label_and_execution() {
    let (operator_checked, operator_plans) = checked_with_plans(OPERATOR_SOURCE);
    let (requirement_checked, requirement_plans) = checked_with_plans(&requirement_source());
    assert_eq!(operator_plans.len(), 2);
    assert_eq!(requirement_plans.len(), 2, "{requirement_plans:?}");
    for name in ["negate", "minimum"] {
        let operator_view = view(&operator_checked, name);
        let requirement_view = view(&requirement_checked, name);
        assert_eq!(operator_view.kind, IntrinsicRequirementKind::Operator);
        assert_eq!(
            requirement_view.kind,
            IntrinsicRequirementKind::TopLevelRequirement
        );
        assert_eq!(operator_view.display(), requirement_view.display());
        assert_eq!(
            operator_view.parameters.len(),
            requirement_view.parameters.len()
        );
        assert_eq!(
            named_float_realization_for(&operator_checked.typed, &operator_view),
            named_float_realization_for(&requirement_checked.typed, &requirement_view),
        );
        assert_eq!(
            provider_planning::compiler_intrinsic_diagnostic_label_for(
                &operator_checked.typed,
                &operator_view
            ),
            provider_planning::compiler_intrinsic_diagnostic_label_for(
                &requirement_checked.typed,
                &requirement_view
            ),
        );
        let plan_for =
            |plans: &[ProviderPlan], checked: &CheckedTrees, view: &IntrinsicRequirement<'_>| {
                plans
                    .iter()
                    .find(|plan| view.schema_binds(&checked.typed, &plan.schema))
                    .cloned()
                    .expect("the intrinsic satisfier derives one plan")
            };
        let operator_plan = plan_for(&operator_plans, &operator_checked, &operator_view);
        let requirement_plan =
            plan_for(&requirement_plans, &requirement_checked, &requirement_view);
        let operator_realization = selected_compiler_intrinsic_realization(
            &operator_checked.typed,
            &operator_plan,
            operator_view.symbol,
        )
        .expect("operator realization resolves");
        let requirement_realization = selected_compiler_intrinsic_realization(
            &requirement_checked.typed,
            &requirement_plan,
            requirement_view.symbol,
        )
        .expect("requirement realization resolves");
        assert_eq!(operator_realization, requirement_realization);
        assert!(matches!(
            requirement_realization,
            Some(SelectedCompilerIntrinsicRealization::NamedFloat(
                NamedFloatRealization::Negate(FloatFormat::F32)
                    | NamedFloatRealization::Builtin { .. }
            ))
        ));
        // The entry-state symbol a direct call retains selects the same view.
        assert_eq!(
            IntrinsicRequirement::by_symbol(
                &requirement_checked.typed,
                requirement_view.call_target
            )
            .map(|resolved| resolved.symbol),
            Some(requirement_view.symbol)
        );
    }
}

#[test]
fn requirement_uses_are_stamped_with_their_selected_plan_and_stage_the_same_rewrite() {
    let (operator_checked, operator_plans) = checked_with_plans(OPERATOR_SOURCE);
    let (requirement_checked, requirement_plans) = checked_with_plans(&requirement_source());
    assert_eq!(operator_checked.facts.operators.named_uses().count(), 2);
    assert_eq!(
        operator_checked
            .facts
            .operators
            .named_requirement_uses()
            .count(),
        0
    );
    assert_eq!(requirement_checked.facts.operators.named_uses().count(), 0);
    assert_eq!(
        requirement_checked
            .facts
            .operators
            .named_requirement_uses()
            .count(),
        2,
        "both direct requirement calls are retained as named requirement uses"
    );

    let (operator_bound, operator_selected) = bound(operator_checked, &operator_plans);
    let (requirement_bound, requirement_selected) = bound(requirement_checked, &requirement_plans);
    let operator_uses = operator_bound
        .facts
        .operators
        .named_uses()
        .map(SelectedIntrinsicUse::from)
        .collect::<Vec<_>>();
    let requirement_uses = requirement_bound
        .facts
        .operators
        .named_requirement_uses()
        .map(SelectedIntrinsicUse::from)
        .collect::<Vec<_>>();
    assert!(
        requirement_uses
            .iter()
            .all(
                |selected_use| selected_use.provider_plan_report_fingerprint != 0
                    && !selected_use.provider_plan_commitment.is_empty()
            ),
        "provider planning stamps each requirement use: {requirement_uses:?}"
    );
    let mut operator_rewrites = operator_uses
        .iter()
        .map(|selected_use| {
            resolve_selected_float_intrinsic_call(
                &operator_bound,
                operator_selected.plans(),
                selected_use,
            )
            .expect("operator use resolves")
            .expect("operator use stages a rewrite")
        })
        .map(|rewrite| (rewrite.realization, rewrite.execution))
        .collect::<Vec<_>>();
    let mut requirement_rewrites = requirement_uses
        .iter()
        .map(|selected_use| {
            resolve_selected_float_intrinsic_call(
                &requirement_bound,
                requirement_selected.plans(),
                selected_use,
            )
            .expect("requirement use resolves")
            .expect("requirement use stages a rewrite")
        })
        .map(|rewrite| (rewrite.realization, rewrite.execution))
        .collect::<Vec<_>>();
    let key = |(realization, _): &(NamedFloatRealization, _)| format!("{realization:?}");
    operator_rewrites.sort_by_key(key);
    requirement_rewrites.sort_by_key(key);
    assert_eq!(
        operator_rewrites
            .iter()
            .map(|(realization, _)| realization)
            .collect::<Vec<_>>(),
        requirement_rewrites
            .iter()
            .map(|(realization, _)| realization)
            .collect::<Vec<_>>(),
    );
    // Same execution forms modulo the target type handles of each program.
    for ((_, operator_execution), (_, requirement_execution)) in
        operator_rewrites.iter().zip(&requirement_rewrites)
    {
        assert_eq!(
            std::mem::discriminant(operator_execution),
            std::mem::discriminant(requirement_execution)
        );
    }
}

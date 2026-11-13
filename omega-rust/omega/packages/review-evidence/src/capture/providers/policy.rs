// Selected-provider policy projection captured from live checked associations.
// Canonical-to-custody plan indices travel to boundary-application projection within
// the same immutable capture invocation without entering serialized evidence.
// Selected-instance custody is derived from the original compiler program, grounded
// installation dependency ordering is verified against circular rows, and
// toolchain-settled plans (such as `FilesystemHost`) omit authored provider types
// while covering only the leaves the toolchain binds.

pub(crate) mod bindings;
mod families;
pub(super) mod realizations;
mod rows;

use crate::capture::PackageReviewInput;
use crate::capture::providers::selection::validate_selected_provider_declaration_owner;
use crate::capture::refusal;
use crate::capture::semantics::declarations::{
    nominal_identity, policy_provider_requirement_identity,
};
use crate::capture::semantics::services;
use crate::record::{
    PackagePolicyProviderPlan, PackagePolicySelectedProviders,
    PackageReviewProviderGrantSelectorKind,
};
use diagnostics::Diagnostic;
use provider_planning::ProviderSchemaDeclaration;
use provider_planning::SelectedProviderReviewProvenance;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;
use trust_model::ProviderGrantSelectorKind;

pub(crate) fn project_checked_selected_provider_policy<'a>(
    input: impl Into<PackageReviewInput<'a>>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<(PackagePolicySelectedProviders, Vec<usize>), Vec<Diagnostic>> {
    let input = input.into();
    let projected = input
        .external_providers
        .is_none()
        .then(|| input.project_external_providers())
        .transpose()?;
    let services = input
        .service_declarations
        .is_none()
        .then(|| input.project_service_declarations())
        .transpose()?;
    let compilation = &PackageReviewInput {
        service_declarations: input.service_declarations.or(services.as_ref()),
        external_providers: input.external_providers.or(projected.as_ref()),
        ..input
    };
    let selected_execution = compilation.custody.execution();
    let selected_instances = compilation
        .custody
        .source_inspection()
        .project_instances()?;
    let mut signatures = services::SignatureProjection::new(*compilation);
    let mut plans = selected_execution
        .selected_provider_provenance
        .as_slice()
        .iter()
        .enumerate()
        .map(|(index, retained)| {
            project_plan(&mut signatures, target, retained, &selected_instances)
                .map(|plan| (index, plan))
        })
        .collect::<Result<Vec<_>, _>>()?;
    drop(signatures);
    plans.sort_by(|(_, left), (_, right)| left.compare_canonical(right));
    super::installation::validate_installation_dependency_order(compilation, &selected_instances)?;
    let families = families::project(compilation, target, &plans)?;
    let projected_installation_reaches = plans
        .iter()
        .flat_map(|(_, plan)| &plan.rows)
        .filter(|row| row.installation_reach.is_some())
        .count();
    if projected_installation_reaches
        != selected_execution
            .plans()
            .installation_reach_resolutions()
            .len()
    {
        return Err(rejected(
            "an installation-reach resolution has no exact selected row",
        ));
    }
    let projected_grants: usize = plans.iter().map(|(_, plan)| plan.grants.len()).sum();
    if projected_grants != selected_execution.selected_provider_grants.as_slice().len() {
        return Err(rejected(
            "an authored provider grant has no unique selected plan",
        ));
    }
    let original_indices = plans.iter().map(|(index, _)| *index).collect();
    let policy = PackagePolicySelectedProviders {
        package,
        target,
        plans: plans.into_iter().map(|(_, plan)| plan).collect(),
        families,
    };
    Ok((policy, original_indices))
}

fn project_plan(
    signatures: &mut services::SignatureProjection<'_>,
    target: TargetProfile,
    retained: &SelectedProviderReviewProvenance,
    selected_instances: &effects::SelectedProviderPlanFacts,
) -> Result<PackagePolicyProviderPlan, Vec<Diagnostic>> {
    let input = signatures.input();
    let compilation = &input;
    let selected_execution = compilation.custody.execution();
    let plan = &retained.plan;
    let toolchain_settled = selected_execution.plans().is_toolchain_settled(plan);
    let schema_declaration = nominal_identity(compilation, retained.provider.schema.symbol())?;
    validate_selected_provider_declaration_owner(
        &schema_declaration,
        plan.schema.declaration.trait_package_identity,
        &plan.name,
        "service schema",
    )?;
    let provider_type_declaration = retained
        .provider
        .provider_type
        .map(|symbol| nominal_identity(compilation, symbol))
        .transpose()?;
    match &provider_type_declaration {
        Some(declaration) => validate_selected_provider_declaration_owner(
            declaration,
            plan.provider_type_package_identity,
            &plan.name,
            "provider type",
        )?,
        None if matches!(
            retained.selected_by,
            provider_planning::ProviderSelectionProvenance::NativeBuild
        ) && !plan.rows.is_empty()
            && retained
                .provider
                .row_realizations
                .iter()
                .all(|row| row.native().is_some()) => {}
        None if plan.provider_type_package_identity.is_none()
            && (plan.provider_type.is_empty() || toolchain_settled) => {}
        None => return Err(rejected("provider type has no exact declaration")),
    }
    let mut methods = Vec::with_capacity(plan.schema.declaration.methods.len());
    for method in &plan.schema.declaration.methods {
        let matching_rows = plan
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                plan.identities.text(row.declared_requirement_identity)
                    == plan.schema.identity_text(method.requirement_identity)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if matching_rows.is_empty() && toolchain_settled {
            continue;
        }
        let Some(index) = matching_rows.first() else {
            return Err(rejected("service method has no selected realization row"));
        };
        if matching_rows.iter().any(|other| {
            retained.provider.row_requirements[*other] != retained.provider.row_requirements[*index]
        }) {
            return Err(rejected(
                "selected applications disagree on their declared service method",
            ));
        }
        methods.push(services::project(
            signatures,
            &plan.schema,
            retained.provider.schema,
            retained.provider.provider_type,
            retained.provider.row_requirements[*index],
            method,
            &plan.schema.declaration.identities,
        )?);
    }
    let mut rows = rows::project(
        compilation,
        target,
        retained,
        toolchain_settled,
        selected_instances,
    )?;
    rows.sort_by(|left, right| {
        left.requirement.cmp(&right.requirement).then_with(|| {
            left.requirement_application
                .cmp(&right.requirement_application)
        })
    });
    let mut grants = selected_execution
        .selected_provider_grants
        .as_slice()
        .iter()
        .filter(|grant| grant.grant.replays_selected_plan(plan))
        .map(|grant| match grant.grant.selector_kind {
            ProviderGrantSelectorKind::PlanName => PackageReviewProviderGrantSelectorKind::PlanName,
            ProviderGrantSelectorKind::ProviderSlot => {
                PackageReviewProviderGrantSelectorKind::ProviderSlot
            }
        })
        .collect::<Vec<_>>();
    grants.sort();
    if grants.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(rejected(
            "selected plan has duplicate exact grant selectors",
        ));
    }
    Ok(PackagePolicyProviderPlan {
        plan_name: normalized_plan_name(compilation, retained)?,
        realizing_package: plan.origin_package_identity,
        schema_declaration,
        provider_type: plan.provider_type.clone(),
        provider_type_declaration,
        target: plan.target.clone(),
        methods,
        rows,
        grants,
    })
}

fn normalized_plan_name(
    compilation: &PackageReviewInput<'_>,
    retained: &SelectedProviderReviewProvenance,
) -> Result<String, Vec<Diagnostic>> {
    let plan = &retained.plan;
    let ProviderSchemaDeclaration::BoundaryOperator(symbol) = retained.provider.schema else {
        return Ok(plan.name.clone());
    };
    let generated = provider_planning::satisfies_plan_name(
        &plan.target,
        plan.schema
            .identity_text(plan.schema.declaration.trait_name),
        &plan.provider_type,
    );
    if plan.name != generated {
        return Err(rejected(
            "operator plan name differs from its exact generated provenance",
        ));
    }
    let requirement =
        policy_provider_requirement_identity(compilation, retained.provider.schema, symbol)?;
    Ok(provider_planning::satisfies_plan_name(
        &plan.target,
        requirement.path(),
        &plan.provider_type,
    ))
}

refusal!(reason, "selected-provider policy rejects {reason}");

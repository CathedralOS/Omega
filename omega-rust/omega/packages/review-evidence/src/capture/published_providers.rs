//! Selected provider disclosure follows the retained actual selection and its
//! accepted source. Candidate eligibility does not create a selected plan.

use super::published_input::{PublishedPackageReviewInput, rejected};
use super::published_projection::nominal;
use crate::record::*;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use provider_planning::{
    ProviderSchemaDeclaration, ProviderSelectionProvenance, ProviderSelectionSubject,
};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

type Review<T> = Result<T, Vec<Diagnostic>>;

pub(super) fn selected(
    input: PublishedPackageReviewInput<'_>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Review<PackagePolicySelectedProviders> {
    let execution = input.custody.execution();
    let instances = input.custody.source_inspection().project_instances()?;
    let retained = execution.selected_provider_provenance.as_slice();
    if retained.len() != instances.plans().len() {
        return Err(rejected(
            "selected plan census differs from its accepted instance projection",
        ));
    }
    if !execution
        .plans()
        .installation_reach_resolutions()
        .is_empty()
    {
        return Err(rejected(
            "installation-relative reach needs its exact selected application projection",
        ));
    }
    let mut plans = Vec::new();
    for (retained, projected) in retained.iter().zip(instances.plans()) {
        let plan = &retained.plan;
        if plan.rows.len() != retained.provider.row_requirements.len()
            || plan.rows.len() != retained.provider.row_realizations.len()
            || plan.rows.len() != retained.row_compiler_intrinsic_executions.len()
            || plan.rows.len() != projected.rows.len()
        {
            return Err(rejected(
                "selected provider lost its aligned requirement and realization census",
            ));
        }
        if let ProviderSelectionProvenance::BuildOverride(selections)
        | ProviderSelectionProvenance::TargetDefault(selections) = &retained.selected_by
        {
            if selections.iter().any(|row| {
                matches!(
                    row.subject,
                    ProviderSelectionSubject::BoundaryOperatorFamily(_)
                )
            }) {
                return Err(rejected(
                    "selected operator families need their complete exact coordinate coverage",
                ));
            }
        }
        let ProviderSchemaDeclaration::BoundaryTrait(schema_symbol) = retained.provider.schema
        else {
            return Err(rejected(
                "non-trait provider needs its complete accepted declaration projection",
            ));
        };
        let mut methods = Vec::new();
        let toolchain = execution.plans().is_toolchain_settled(plan);
        for method in &plan.schema.declaration.methods {
            let mut matching = plan.rows.iter().enumerate().filter(|(_, row)| {
                plan.identities.text(row.declared_requirement_identity)
                    == plan.schema.identity_text(method.requirement_identity)
            });
            let Some((index, _)) = matching.next() else {
                if toolchain {
                    continue;
                }
                return Err(rejected(
                    "selected service method has no exact realization row",
                ));
            };
            if matching.any(|(other, _)| {
                retained.provider.row_requirements[other]
                    != retained.provider.row_requirements[index]
            }) {
                return Err(rejected(
                    "selected applications disagree on the source requirement",
                ));
            }
            methods.push(super::published_services::method(
                input,
                schema_symbol,
                retained.provider.row_requirements[index],
                &plan.schema,
                method,
            )?);
        }
        let mut rows = Vec::new();
        for (index, (row, instance)) in plan.rows.iter().zip(&projected.rows).enumerate() {
            let requirement = retained.provider.row_requirements[index];
            let origin = &retained.provider.row_realizations[index];
            let realization = origin
                .authored()
                .map(|symbol| nominal(input, symbol))
                .transpose()?;
            if !row.requirement_lifetime_partition.is_empty() {
                return Err(rejected(
                    "provider lifetime partitions need their complete applied lifetime projection",
                ));
            }
            if retained.row_compiler_intrinsic_executions[index].is_some() {
                return Err(rejected(
                    "compiler intrinsic execution needs its complete target and source commitment projection",
                ));
            }
            let native_build = origin
                .native()
                .map(|native| super::native_bindings::native_producer(&input, native))
                .transpose()?;
            let binding = if let Some(native) = origin.native() {
                super::native_bindings::project_native(&input, native, &row.binding)?
            } else {
                match (&row.binding, &instance.binding) {
                    (
                        ProviderBinding::Syscall { number },
                        ProviderBinding::Syscall { number: other },
                    ) if number == other => PackagePolicyProviderBinding::Syscall {
                        number: *number,
                        evaluated: None,
                    },
                    (
                        ProviderBinding::CompilerIntrinsic { machine },
                        ProviderBinding::CompilerIntrinsic { machine: other },
                    ) if machine == other => PackagePolicyProviderBinding::CompilerIntrinsic {
                        machine: machine.clone(),
                    },
                    (
                        ProviderBinding::VtableSlot { index },
                        ProviderBinding::VtableSlot { index: other },
                    ) if index == other => {
                        PackagePolicyProviderBinding::VtableSlot { index: *index }
                    }
                    (
                        ProviderBinding::CheckedAdapter {
                            machine_identity,
                            machine_package_identity,
                        },
                        ProviderBinding::CheckedAdapter {
                            machine_identity: portable,
                            machine_package_identity: portable_package,
                        },
                    ) => {
                        let symbol = origin.authored().ok_or_else(|| {
                            rejected("checked adapter has no retained selected source machine")
                        })?;
                        let source = input.custody.source_inspection().program();
                        let machine = source
                            .accepted_machine(symbol)
                            .map_err(|reason| rejected(&reason))?;
                        let declaration = machine.declaration.as_ref().ok_or_else(|| {
                            rejected("checked adapter has no accepted callable declaration")
                        })?;
                        if source.module().identities.text(declaration.callable)
                            != Some(plan.identities.text(*machine_identity))
                            || *machine_package_identity
                                != input.candidate.symbols.symbol_package_identity(symbol)
                            || machine_package_identity != portable_package
                        {
                            return Err(rejected(
                                "checked adapter changed its exact accepted selected callable",
                            ));
                        }
                        PackagePolicyProviderBinding::CheckedAdapter {
                            machine_identity: projected.identities.text(*portable).to_owned(),
                            machine_package_identity: *portable_package,
                        }
                    }
                    _ => {
                        return Err(rejected(
                            "provider binding needs its complete evaluated import, table or native producer projection",
                        ));
                    }
                }
            };
            rows.push(PackagePolicyProviderRow {
                method: row.method.clone(),
                requirement: super::published_services::requirement_identity(input, requirement)?,
                requirement_application: (row.requirement_identity
                    != row.declared_requirement_identity)
                    .then(|| plan.identities.text(row.requirement_identity).to_owned()),
                realization,
                native_build,
                requirement_lifetime_partition: row.requirement_lifetime_partition.clone(),
                admitted_same_stack_bytes: row.admitted_same_stack_bytes,
                binding,
                compiler_intrinsic_execution: None,
                installation_reach: None,
            });
        }
        rows.sort_by(|a, b| {
            a.requirement
                .cmp(&b.requirement)
                .then(a.requirement_application.cmp(&b.requirement_application))
        });
        let mut grants = execution
            .selected_provider_grants
            .as_slice()
            .iter()
            .filter(|grant| grant.grant.replays_selected_plan(plan))
            .map(|grant| match grant.grant.selector_kind {
                trust_model::ProviderGrantSelectorKind::PlanName => {
                    PackageReviewProviderGrantSelectorKind::PlanName
                }
                trust_model::ProviderGrantSelectorKind::ProviderSlot => {
                    PackageReviewProviderGrantSelectorKind::ProviderSlot
                }
            })
            .collect::<Vec<_>>();
        grants.sort();
        if grants.windows(2).any(|rows| rows[0] == rows[1]) {
            return Err(rejected("selected plan repeats an exact grant selector"));
        }
        plans.push(PackagePolicyProviderPlan {
            plan_name: plan.name.clone(),
            realizing_package: plan.origin_package_identity,
            schema_declaration: nominal(input, schema_symbol)?,
            provider_type: plan.provider_type.clone(),
            provider_type_declaration: retained
                .provider
                .provider_type
                .map(|symbol| nominal(input, symbol))
                .transpose()?,
            target: plan.target.clone(),
            methods,
            rows,
            grants,
        });
    }
    if plans.iter().map(|plan| plan.grants.len()).sum::<usize>()
        != execution.selected_provider_grants.as_slice().len()
    {
        return Err(rejected(
            "an actual provider grant has no exact selected plan",
        ));
    }
    plans.sort_by(|a, b| a.compare_canonical(b));
    Ok(PackagePolicySelectedProviders {
        package,
        target,
        plans,
        families: Vec::new(),
    })
}

pub(super) fn native_inventory(
    input: PublishedPackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Review<Vec<PackagePolicyNativeBinding>> {
    let mut rows = Vec::new();
    for candidate in &input.custody.providers().native_build_plans {
        if candidate.plan.origin_package_identity != Some(package) {
            continue;
        }
        let ProviderSchemaDeclaration::BoundaryTrait(schema_symbol) = candidate.provenance.schema
        else {
            return Err(rejected(
                "native inventory needs its complete non-trait requirement projection",
            ));
        };
        if candidate.plan.rows.len() != candidate.provenance.row_realizations.len() {
            return Err(rejected(
                "native inventory lost its aligned realization census",
            ));
        }
        for (row, origin) in candidate
            .plan
            .rows
            .iter()
            .zip(&candidate.provenance.row_realizations)
        {
            let native = origin
                .native()
                .ok_or_else(|| rejected("native inventory has no retained Build producer"))?;
            let mut methods = candidate
                .plan
                .schema
                .declaration
                .methods
                .iter()
                .filter(|method| {
                    candidate
                        .plan
                        .schema
                        .identity_text(method.requirement_identity)
                        == candidate
                            .plan
                            .identities
                            .text(row.declared_requirement_identity)
                });
            let (Some(method), None) = (methods.next(), methods.next()) else {
                return Err(rejected(
                    "native inventory lost its unique complete requirement",
                ));
            };
            let method = super::published_services::method(
                input,
                schema_symbol,
                native.requirement,
                &candidate.plan.schema,
                method,
            )?;
            for production in
                std::iter::once(&native.origin.primary).chain(&native.origin.equivalents)
            {
                let occurrence = provider_planning::native_bindings::NativeBuildRowOrigin {
                    application: native.application,
                    requirement: native.requirement,
                    origin: package_compilation::PackageNativeBindingOrigin {
                        primary: production.clone(),
                        equivalents: Vec::new(),
                    },
                };
                rows.push(PackagePolicyNativeBinding {
                    application: candidate.plan.provider_application.is_valid().then(|| {
                        candidate
                            .plan
                            .identities
                            .text(candidate.plan.provider_application)
                            .to_owned()
                    }),
                    method: method.clone(),
                    binding: super::native_bindings::project_native(
                        &input,
                        &occurrence,
                        &row.binding,
                    )?,
                });
            }
        }
    }
    rows.sort_by(|a, b| a.key().cmp(&b.key()));
    rows.dedup();
    if rows.windows(2).any(|rows| rows[0].key() == rows[1].key()) {
        return Err(rejected(
            "native inventory repeats conflicting producer and requirement coordinates",
        ));
    }
    Ok(rows)
}

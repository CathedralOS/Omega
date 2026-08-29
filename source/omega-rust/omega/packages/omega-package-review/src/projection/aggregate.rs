use super::callables::*;
use super::contracts::*;
use super::evidence::*;
use super::exact_identity::*;
use super::provider_intrinsics::project_compiler_intrinsic_builtin;
use super::public_api::*;
use super::public_traits::*;
use crate::model::*;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;

/// Project the exact checked authority facts that are already safely joined.
///
/// This refuses standalone and target-free compilations, missing checked fact
/// rows, and a non-root build machine. Compiler-generated nominals inherit the
/// exact authored source provenance of their mandatory derivation origin.
/// Truly source-free nominals remain explicit `Unresolved` review rows; a later
/// admission certificate must reject them rather than treating them as empty
/// authority.
pub fn project_checked_package_review(
    compilation: &CheckedCompilation,
) -> Result<CheckedPackageReviewProjection, Vec<Diagnostic>> {
    let package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires package-aware checked compilation",
        )]
    })?;
    let target = compilation.selected_target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires one explicit target selection",
        )]
    })?;
    if !compilation.contract_entailment_stand_downs().is_empty() {
        return Err(compilation
            .contract_entailment_stand_downs()
            .iter()
            .map(|stand_down| {
                Diagnostic::error(format!(
                    "package review rejects unresolved contract-entailment stand-down at machine symbol {}, contract {}, fact {}: {}",
                    stand_down.machine_symbol.arena_index(),
                    stand_down.contract_index,
                    stand_down.fact_index,
                    stand_down.reason.label(),
                ))
            })
            .collect());
    }
    let derived_operator_realizations =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &compilation.typed,
        );
    if derived_operator_realizations != compilation.facts.operators.operator_realization_contracts {
        return Err(vec![Diagnostic::error(format!(
            "retained checked operator-realization contracts do not equal compiler rederivation (retained {} rows, derived {} rows)",
            compilation
                .facts
                .operators
                .operator_realization_contracts
                .len(),
            derived_operator_realizations.len(),
        ))]);
    }
    let build_machine = compilation.selected_build_machine_symbol();
    let public_traits = project_public_traits(compilation, package)?;
    let public_conformances = project_public_conformances(compilation, package)?;
    let public_domains = project_public_domains(compilation, package)?;
    let public_propositions = project_public_propositions(compilation, package)?;
    let public_consts = project_public_consts(compilation, package)?;
    let public_operators = project_public_operators(compilation, package)?;
    let public_data = project_public_data(compilation, package)?;
    let representation_tcb = project_representation_tcb(compilation, package)?;
    let semantic_dependencies = project_semantic_dependencies(compilation, package)?;
    let mut callables = Vec::new();
    let mut external_executable_supply = Vec::new();
    let mut projected_build_machine = false;

    for machine in compilation.machines() {
        let role = if Some(machine.symbol) == build_machine {
            Some(PackageReviewCallableRole::Build)
        } else if machine.supply_mode.is_boundary_declaration() {
            Some(PackageReviewCallableRole::Boundary)
        } else if machine.is_public {
            Some(PackageReviewCallableRole::Public)
        } else {
            None
        };
        let Some(role) = role else {
            continue;
        };
        let owner = nominal_identity(compilation, machine.symbol)?;
        match owner.owner {
            PackageReviewNominalOwner::Package(owner) if owner == package => {}
            PackageReviewNominalOwner::Package(_)
            | PackageReviewNominalOwner::ToolchainSource(_) => {
                continue;
            }
            PackageReviewNominalOwner::Unresolved => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` has no managed package owner",
                    owner.path
                ))]);
            }
        }

        let (callable, executable_supply) = project_callable(compilation, machine, role, owner)?;
        let mut contract_locations =
            project_contract_source_locations(compilation, compilation.machine_contracts(machine))?;
        contract_locations.extend(project_machine_invocation_source_locations(
            compilation,
            machine,
        )?);
        contract_locations.extend(project_machine_service_reach_source_locations(
            compilation,
            machine,
        )?);
        contract_locations.extend(project_machine_operational_source_locations(
            compilation,
            machine,
        )?);
        collect_type_parameter_source_locations(
            compilation,
            compilation.machine_type_parameters(machine),
            &mut contract_locations,
        )?;
        let entry = compilation.machine_states(machine).first().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{}` has no canonical entry signature",
                compilation.typed.symbols.display_path(machine.symbol, "::")
            ))]
        })?;
        collect_callable_parameter_source_locations(
            compilation,
            compilation.state_parameters(entry),
            "reviewed callable parameter",
            &mut contract_locations,
        )?;
        contract_locations.extend(
            psi_typed_trees_to_checked_trees::derive_checked_body_call_source_spans(
                &compilation.typed,
                &compilation.facts,
                machine.symbol,
            )?
            .into_iter()
            .map(|source_span| ProjectedNestedSourceLocation {
                source_span,
                role: PackageReviewSourceLocationRole::BodyCall,
            }),
        );
        external_executable_supply.extend(executable_supply);
        callables.push(ProjectedReviewRow {
            row: callable,
            declaration: machine.symbol,
            nested_source_locations: contract_locations,
        });
        projected_build_machine |= role == PackageReviewCallableRole::Build;
    }

    // External executable supply is trust-bearing even when the leaf is a
    // private implementation detail. Public/build leaves were projected with
    // their callable envelopes above; project every remaining package-owned
    // external leaf without manufacturing a public callable row.
    for machine in compilation.machines() {
        if !matches!(
            machine.supply_mode,
            MachineSupplyMode::ExternalRealization { .. }
        ) || machine.is_public
            || Some(machine.symbol) == build_machine
        {
            continue;
        }
        let owner = nominal_identity(compilation, machine.symbol)?;
        match owner.owner {
            PackageReviewNominalOwner::Package(owner_package) if owner_package == package => {}
            PackageReviewNominalOwner::Package(_)
            | PackageReviewNominalOwner::ToolchainSource(_) => continue,
            PackageReviewNominalOwner::Unresolved => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has no managed package owner",
                    owner.path
                ))]);
            }
        }
        external_executable_supply.extend(project_private_external_executable_supply(
            compilation,
            machine,
            &owner,
        )?);
    }

    if build_machine.is_some() && !projected_build_machine {
        return Err(vec![Diagnostic::error(
            "selected build machine is not owned by the reviewed root package",
        )]);
    }

    callables.sort_by(|left, right| {
        left.row
            .identity
            .cmp(&right.row.identity)
            .then(left.row.role.cmp(&right.row.role))
            .then(left.row.contracts.cmp(&right.row.contracts))
    });
    external_executable_supply.sort_by(|left, right| left.row.cmp(&right.row));
    if external_executable_supply
        .windows(2)
        .any(|rows| rows[0].row == rows[1].row)
    {
        return Err(vec![Diagnostic::error(
            "package review contains a duplicate exact external executable-supply row",
        )]);
    }
    let dangerous_authorities = project_dangerous_authorities(compilation, &callables)?;
    let dangerous_authority_slack = project_dangerous_authority_slack(compilation, &callables)?;
    let selected_plans = compilation.selected_provider_plans().plans();
    let selected_provider_provenance = compilation.selected_provider_provenance();
    if selected_plans.len() != selected_provider_provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected-provider review provenance is not aligned with the canonical selected plan set",
        )]);
    }
    let mut selected_providers = Vec::with_capacity(selected_plans.len());
    for (plan, retained) in selected_plans.iter().zip(selected_provider_provenance) {
        if retained.plan != *plan
            || retained.provider.row_requirements.len() != plan.rows.len()
            || retained.provider.row_realizations.len() != plan.rows.len()
            || retained.row_compiler_intrinsic_builtins.len() != plan.rows.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` has incomplete or misaligned declaration provenance",
                plan.name,
            ))]);
        }
        let row_declarations = retained
            .provider
            .row_requirements
            .iter()
            .zip(&retained.provider.row_realizations)
            .zip(&retained.row_compiler_intrinsic_builtins)
            .zip(&plan.rows)
            .map(|(((requirement, realization), retained_builtin), row)| {
                Ok(CheckedPackageProviderRowIdentity {
                    requirement: provider_requirement_identity(
                        compilation,
                        retained.provider.schema,
                        *requirement,
                    )?,
                    realization: nominal_identity(compilation, *realization)?,
                    compiler_intrinsic_builtin: project_compiler_intrinsic_builtin(
                        compilation,
                        plan,
                        row,
                        matches!(
                            retained.provider.schema,
                            omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryOperator(_)
                        ),
                        *requirement,
                        *retained_builtin,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let schema_declaration = nominal_identity(compilation, retained.provider.schema.symbol())?;
        validate_selected_provider_declaration_owner(
            &schema_declaration,
            plan.schema.trait_package_identity,
            &plan.name,
            "service schema",
        )?;
        let provider_type_declaration = retained
            .provider
            .provider_type
            .map(|symbol| nominal_identity(compilation, symbol))
            .transpose()?;
        match provider_type_declaration.as_ref() {
            Some(declaration) => validate_selected_provider_declaration_owner(
                declaration,
                plan.provider_type_package_identity,
                &plan.name,
                "provider type",
            )?,
            None if plan.provider_type.is_empty()
                && plan.provider_type_package_identity.is_none() => {}
            None => {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` has provider-type identity without one exact declaration",
                    plan.name,
                ))]);
            }
        }
        for (row, declarations) in plan.rows.iter().zip(&row_declarations) {
            let mut methods = plan
                .schema
                .methods
                .iter()
                .filter(|method| method.requirement_identity == row.requirement_identity);
            let Some(method) = methods.next() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` row `{}` has no exact schema method",
                    plan.name, row.requirement_identity,
                ))]);
            };
            if methods.next().is_some() {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` row `{}` has duplicate schema methods",
                    plan.name, row.requirement_identity,
                ))]);
            }
            validate_selected_provider_declaration_owner(
                &declarations.requirement,
                method.requirement_owner_package_identity,
                &plan.name,
                "row requirement",
            )?;
            validate_selected_provider_declaration_owner(
                &declarations.realization,
                plan.origin_package_identity,
                &plan.name,
                "row realization",
            )?;
        }
        selected_providers.push(CheckedPackageProviderReview {
            plan_name: plan.name.clone(),
            plan_fingerprint: plan.identity_fingerprint(),
            realizing_package: plan.origin_package_identity,
            schema_declaration,
            provider_type: plan.provider_type.clone(),
            provider_type_package: plan.provider_type_package_identity,
            provider_type_declaration,
            schema: plan.schema.clone(),
            target: plan.target.clone(),
            rows: plan.rows.clone(),
            row_declarations,
        });
    }
    let (public_traits, public_trait_sources) = finalize_projected_rows(
        compilation,
        public_traits,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_conformances, public_conformance_sources) = finalize_projected_rows(
        compilation,
        public_conformances,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_domains, public_domain_sources) = finalize_projected_rows(
        compilation,
        public_domains,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_propositions, public_proposition_sources) = finalize_projected_rows(
        compilation,
        public_propositions,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_consts, public_const_sources) = finalize_projected_rows(
        compilation,
        public_consts,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_operators, public_operator_sources) = finalize_projected_rows(
        compilation,
        public_operators,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_data, public_data_sources) = finalize_projected_rows(
        compilation,
        public_data,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (representation_tcb, representation_tcb_sources) = finalize_projected_rows(
        compilation,
        representation_tcb,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (semantic_dependencies, semantic_dependency_sources) =
        finalize_semantic_dependency_rows(compilation, semantic_dependencies)?;
    let (callables, callable_sources) = finalize_projected_rows(
        compilation,
        callables,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (external_executable_supply, external_executable_supply_sources) = finalize_projected_rows(
        compilation,
        external_executable_supply,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (dangerous_authorities, dangerous_authority_sources) =
        finalize_dangerous_authority_rows(compilation, dangerous_authorities)?;
    let (dangerous_authority_slack, dangerous_authority_slack_sources) =
        finalize_dangerous_authority_slack_rows(compilation, dangerous_authority_slack)?;
    let row_sources = PackageReviewCanonicalRowSources {
        public_traits: public_trait_sources,
        public_conformances: public_conformance_sources,
        public_domains: public_domain_sources,
        public_propositions: public_proposition_sources,
        public_consts: public_const_sources,
        public_operators: public_operator_sources,
        public_data: public_data_sources,
        representation_tcb: representation_tcb_sources,
        semantic_dependencies: semantic_dependency_sources,
        callables: callable_sources,
        external_executable_supply: external_executable_supply_sources,
        dangerous_authorities: dangerous_authority_sources,
        dangerous_authority_slack: dangerous_authority_slack_sources,
        selected_provider_set: selected_provider_row_source(compilation, &selected_providers)?,
    };
    validate_canonical_row_source_limits(&row_sources)?;

    Ok(CheckedPackageReviewProjection {
        package,
        target,
        public_traits,
        public_conformances,
        public_domains,
        public_propositions,
        public_consts,
        public_operators,
        public_data,
        representation_tcb,
        semantic_dependencies,
        callables,
        external_executable_supply,
        dangerous_authorities,
        dangerous_authority_slack,
        selected_providers,
        row_sources,
    })
}

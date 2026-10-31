//! Whole native maps retain their supplier, exact requirements and completed Build.
//! There is no realization machine: source visibility was admitted on the
//! designated operands, and each irreducible row now refines that requirement's
//! independently evaluated calling application. Candidate construction grants
//! no trust; ordinary selected-provider and executable admission follow it.
//! Neutral schemas come from the same prepared Product catalog as authored
//! providers. Exact declared-signature admission and all-target locator checks
//! still precede attaching the selected target's calling evidence.

use super::NativeBuildSupply;
use crate::provider_planning::trait_name_text;
use crate::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderRowRealization, ProviderSchemaDeclaration,
};
use checked_trees::typed_trees::service_declarations::ServiceCatalog;
use diagnostics::Diagnostic;
use effects::provider_plan::{
    BuildNativeBindingOrigin, EvaluatedBindingEvaluationDigest,
    EvaluatedBindingMaterializationDigest, EvaluatedBindingProducerClosureDigest,
    EvaluatedBindingReceipt, EvaluatedBindingUsage, EvaluatedForeignImport,
    EvaluatedForeignSyscall, ProviderIdentityTable, ProviderPlan, ProviderPlanRow,
};
use semantic_vocabulary::NativeBindingLocator;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBuildRowOrigin {
    pub requirement: symbols::SymbolHandle,
    pub origin: package_compilation::PackageNativeBindingOrigin,
}

pub(crate) fn derive(
    services: ServiceCatalog<'_>,
    calling: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    supplies: &[NativeBuildSupply],
    selected_target: Option<target::TargetProfile>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<Vec<DerivedProviderPlan>, Vec<Diagnostic>> {
    let typed = services.context().typed;
    let mut plans: Vec<DerivedProviderPlan> = Vec::new();
    for supply in supplies {
        let selection = &supply.selection;
        let fail = |reason: String| {
            vec![Diagnostic::error(reason).with_source_span(selection.source_span)]
        };
        let owner = typed
            .trait_definition_by_symbol(selection.subject)
            .filter(|owner| owner.is_boundary)
            .ok_or_else(|| fail("native Build requirement lost its boundary owner".into()))?;
        let signature = typed
            .trait_machine_signature_by_symbol(owner, selection.requirement)
            .ok_or_else(|| {
                fail("native Build requirement lost its exact declared signature".into())
            })?;
        let target =
            target::TargetProfile::from_canonical_target_name(selection.target.canonical_name())
                .map_err(|error| fail(error.to_string()))?;
        let requirement = typed.trait_requirement_callable_identity_text(owner, signature);
        let evaluated = evaluate(supply, &requirement, target, identities).map_err(fail)?;
        let producer = selection.producer.clone();
        let production = package_compilation::NativeBindingProduction {
            producer,
            evaluated,
        };
        // Every authored target's locator is checked under that target. Only
        // the requested realization consumes this compilation's evaluated
        // calling geometry; another target must never inherit its ABI plan.
        if selected_target != Some(target) {
            continue;
        }
        let prepared = services
            .declaration(ProviderSchemaDeclaration::BoundaryTrait(owner.symbol))
            .ok_or_else(|| {
                fail("native Build requirement has no complete service schema".into())
            })?;
        let schema = crate::service_schema::attach_calling(&prepared, calling);
        let group = plans.iter().position(|candidate| {
            candidate.provenance.schema.symbol() == owner.symbol
                && candidate.plan.target == target.target_name()
                && candidate.plan.origin_package_identity == Some(selection.supplier_package)
        });
        let index = match group {
            Some(index) => index,
            None => {
                let supplier = selection
                    .supplier_package
                    .digest()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                let provider = format!("native-build:{supplier}:{}", trait_name_text(&schema));
                let name = crate::satisfies_plan_name(
                    target.target_name(),
                    trait_name_text(&schema),
                    &provider,
                );
                plans.push(DerivedProviderPlan {
                    plan: ProviderPlan {
                        name,
                        provider_type: provider,
                        provider_type_package_identity: Some(selection.supplier_package),
                        provider_application: Default::default(),
                        target: target.target_name().to_owned(),
                        schema,
                        rows: Vec::new(),
                        identities: ProviderIdentityTable::default(),
                        origin_package_identity: Some(selection.supplier_package),
                        origin_package: supplier,
                        digest_memo: Default::default(),
                    },
                    provenance: ProviderPlanProvenance {
                        schema: ProviderSchemaDeclaration::BoundaryTrait(owner.symbol),
                        provider_type: None,
                        named_application: None,
                        row_requirements: Vec::new(),
                        row_realizations: Vec::new(),
                        row_target_machine_origins: Vec::new(),
                        row_placed_accessors: Vec::new(),
                    },
                });
                plans.len() - 1
            }
        };
        let candidate = &mut plans[index];
        if let Some(prior) =
            candidate.plan.rows.iter().position(|row| {
                candidate.plan.identities.text(row.requirement_identity) == requirement
            })
        {
            let ProviderRowRealization::BuildNative(origin) =
                &mut candidate.provenance.row_realizations[prior]
            else {
                return Err(fail(
                    "native Build candidate has a different realization owner".into(),
                ));
            };
            if origin.origin.primary.evaluated.identity_digest()
                != production.evaluated.identity_digest()
            {
                return Err(fail("native Build supplier binds one requirement to conflicting normalized locators".into()));
            }
            // Equal bindings are one semantic choice. Keep every reached
            // source/evaluation origin without installing another provider row.
            Arc::make_mut(origin).origin.equivalents.push(production);
            continue;
        }
        candidate.plan.rows.push(ProviderPlanRow {
            method: signature.name.as_str().to_owned(),
            requirement_identity: candidate.plan.identities.intern(&requirement),
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: production.evaluated.provider_binding(),
        });
        candidate.provenance.row_requirements.push(signature.symbol);
        candidate
            .provenance
            .row_realizations
            .push(ProviderRowRealization::BuildNative(Arc::new(
                NativeBuildRowOrigin {
                    requirement: signature.symbol,
                    origin: package_compilation::PackageNativeBindingOrigin {
                        primary: production,
                        equivalents: Vec::new(),
                    },
                },
            )));
        candidate.provenance.row_target_machine_origins.push(None);
        candidate.provenance.row_placed_accessors.push(None);
    }
    for candidate in &plans {
        let errors = candidate.plan.validate_candidate_against_schema();
        if !errors.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "incomplete native Build provider `{}`: {}",
                candidate.plan.name,
                errors.join("; ")
            ))]);
        }
    }
    Ok(plans)
}

fn evaluate(
    supply: &NativeBuildSupply,
    requirement: &str,
    target: target::TargetProfile,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<crate::evaluated_via_bindings::EvaluatedNativeBinding, String> {
    use crate::evaluated_via_bindings::EvaluatedNativeBinding;
    let selection = &supply.selection;
    let author_identity = &selection.producer.callable_identity;
    let source_occurrence_digest = selection.source_occurrence_digest;
    let (locator, binding_digest) = match &selection.locator {
        NativeBindingLocator::Import(raw) => {
            let locator = target::normalize_foreign_locator(raw.clone(), target)
                .map_err(|reason| format!("native Build locator: {reason}"))?;
            let digest = locator.identity_digest();
            (Some(locator), digest)
        }
        NativeBindingLocator::Syscall { number } => {
            if !matches!(
                target,
                target::TargetProfile::LinuxArm64 | target::TargetProfile::LinuxX64
            ) {
                return Err("native Build syscall is not applicable to its target".into());
            }
            let number = u32::try_from(*number)
                .map_err(|_| "native Build syscall number does not fit u32")?;
            (
                None,
                effects::provider_plan::evaluated_syscall_identity_digest(target, number),
            )
        }
    };
    let mut closure = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-build-producer-closure.v1\0",
    );
    closure.raw(
        supply
            .evaluation
            .terminal_code
            .program_fingerprint
            .as_bytes(),
    );
    closure.raw(&source_occurrence_digest);
    let closure = EvaluatedBindingProducerClosureDigest::from_bytes(closure.finish())?;
    let usage = supply.evaluation.usage;
    let (fuel_schedule, storage_schedule) = match usage.measurement {
        evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule,
            storage_schedule,
        } => (fuel_schedule.marker(), storage_schedule.marker()),
        _ => return Err("native Build evidence has a different evaluator measurement".into()),
    };
    let retained_usage = EvaluatedBindingUsage::from_evaluator(
        usage.usage_schema_version,
        usage.step_schedule_marker,
        usage.fuel_units,
        usage.invocation_fuel_ceiling,
        usage.build_log_bytes,
        usage.filesystem_operation_attempts,
        usage.peak_live_storage_cells,
        usage.peak_live_payload_bytes,
        usage.result_storage_cells,
        usage.result_payload_bytes,
    )?;
    let mut evaluation = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-build-binding-evaluation.v1\0",
    );
    evaluation.raw(&supply.evaluation.identity_digest());
    evaluation.raw(&source_occurrence_digest);
    evaluation.raw(&binding_digest.as_bytes());
    let evaluation = EvaluatedBindingEvaluationDigest::from_bytes(evaluation.finish())?;
    let mut materialization = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-build-binding-materialization.v1\0",
    );
    materialization.bytes(target.target_name().as_bytes());
    materialization.raw(&binding_digest.as_bytes());
    let materialization =
        EvaluatedBindingMaterializationDigest::from_bytes(materialization.finish())?;
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        Some(selection.supplier_package),
        &author_identity,
        identities,
        closure,
        fuel_schedule,
        retained_usage,
        evaluation,
        2,
        materialization,
        binding_digest,
    )?
    .with_build_origin(BuildNativeBindingOrigin {
        requirement_identity: requirement.to_owned(),
        source_occurrence_digest,
        terminal_code: supply.evaluation.terminal_code,
        fuel_schedule,
        storage_schedule,
        observation_digest: supply.evaluation.observation_digest,
    })?;
    Ok(match (locator, &selection.locator) {
        (Some(locator), _) => EvaluatedNativeBinding::Import(
            EvaluatedForeignImport::from_retained_evidence(locator, receipt)?,
        ),
        (None, NativeBindingLocator::Syscall { number }) => EvaluatedNativeBinding::Syscall(
            EvaluatedForeignSyscall::from_retained_evidence(target, *number, receipt)?,
        ),
        _ => return Err("native Build materialization changed locator mechanism".into()),
    })
}

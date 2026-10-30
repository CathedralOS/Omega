//! Whole native maps retain their supplier, exact requirements and completed Build.
//! There is no realization machine: source visibility was admitted on the
//! designated operands, and each irreducible row now refines that requirement's
//! independently evaluated calling application. Candidate construction grants
//! no trust; ordinary selected-provider and executable admission follow it.

use super::NativeBuildSupply;
use crate::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderPlanningContext, ProviderRowRealization,
    ProviderSchemaDeclaration,
};
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
    pub selection: build_time_evaluation::native_bindings::NativeBindingSelection,
    pub origin: package_compilation::PackageNativeBindingOrigin,
}

pub(crate) fn derive(
    context: ProviderPlanningContext<'_>,
    calling: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    supplies: &[NativeBuildSupply],
    selected_target: Option<target::TargetProfile>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<Vec<DerivedProviderPlan>, Vec<Diagnostic>> {
    let typed = context.typed;
    let mut plans: Vec<DerivedProviderPlan> = Vec::new();
    for supply in supplies {
        let selection = &supply.selection;
        let fail = |reason: String| {
            vec![Diagnostic::error(reason).with_source_span(selection.source_span)]
        };
        let target =
            target::TargetProfile::from_canonical_target_name(selection.target.canonical_name())
                .map_err(|error| fail(error.to_string()))?;
        // Psi formed every authored target's complete request. Physical backing
        // is demanded only by the realization consuming that target's choice.
        if selected_target != Some(target) {
            continue;
        }
        let application = selection.application;
        let application_identity = application.identity(typed).map_err(fail)?;
        let schema = crate::service_schema::from_typed_requirement_application(
            context,
            calling,
            application.requirement,
            application.trait_scope,
            application.closed_requirement,
        )
        .ok_or_else(|| {
            fail("native Build requirement lost its exact callable application schema".into())
        })?;
        let [method] = schema.methods.as_slice() else {
            return Err(fail(
                "native binding does not own one exact callable requirement".into(),
            ));
        };
        let requirement = schema.identity_text(method.requirement_identity).to_owned();
        let method_name = method.name.clone();
        let declaration =
            if let Some((owner, _)) = typed.trait_machine_signature(application.requirement) {
                ProviderSchemaDeclaration::BoundaryTrait(owner.symbol)
            } else {
                ProviderSchemaDeclaration::BoundaryRequirement(application.requirement)
            };
        let evaluated = evaluate(typed, supply, &requirement, target, identities).map_err(fail)?;
        let production = package_compilation::NativeBindingProduction {
            producer: selection.producer.clone(),
            evaluated,
        };
        let supplier = selection
            .supplier_package
            .digest()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let provider = format!("native-build:{supplier}:{}", application_identity);
        let name = crate::satisfies_plan_name(
            target.target_name(),
            schema.identity_text(schema.trait_name),
            &provider,
        );
        let index = match plans.iter().position(|candidate| {
            candidate.plan.schema == schema
                && candidate.plan.target == target.target_name()
                && candidate.plan.origin_package_identity == Some(selection.supplier_package)
                && candidate
                    .provenance
                    .row_realizations
                    .first()
                    .and_then(|origin| origin.native())
                    .is_some_and(|origin| {
                        origin.selection.application.identity(typed).ok().as_ref()
                            == Some(&application_identity)
                    })
        }) {
            Some(index) => index,
            None => {
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
                        schema: declaration,
                        provider_type: None,
                        named_application: None,
                        row_requirements: Vec::new(),
                        row_realizations: Vec::new(),

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
                return Err(fail("native Build row changed realization owner".into()));
            };
            if origin.selection.application.identity(typed).map_err(fail)? != application_identity
                || origin.selection.supply != selection.supply
                || origin.selection.same_stack_demand != selection.same_stack_demand
            {
                return Err(fail(
                    "native Build supplier installed conflicting full applications".into(),
                ));
            }
            Arc::make_mut(origin).origin.equivalents.push(production);
            continue;
        }
        let lifetime_partition = typed
            .trait_machine_signature(application.requirement)
            .map_or_else(Vec::new, |(owner, _)| {
                (0..owner.lifetime_parameters.len())
                    .map(|ordinal| ordinal as u32)
                    .collect()
            });
        candidate.plan.rows.push(ProviderPlanRow {
            method: method_name,
            requirement_identity: candidate.plan.identities.intern(&requirement),
            requirement_lifetime_partition: lifetime_partition,
            admitted_same_stack_bytes: selection
                .same_stack_demand
                .as_ref()
                .map_or(0, |demand| demand.bytes),
            binding: production.evaluated.provider_binding(),
        });
        candidate
            .provenance
            .row_requirements
            .push(application.requirement);
        candidate
            .provenance
            .row_realizations
            .push(ProviderRowRealization::BuildNative(Arc::new(
                NativeBuildRowOrigin {
                    requirement: application.requirement,
                    selection: selection.clone(),
                    origin: package_compilation::PackageNativeBindingOrigin {
                        primary: production,
                        equivalents: Vec::new(),
                    },
                },
            )));
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
    _typed: &checked_trees::typed_trees::TypedTrees,
    supply: &NativeBuildSupply,
    requirement: &str,
    target: target::TargetProfile,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<effects::provider_plan::EvaluatedNativeBinding, String> {
    use effects::provider_plan::EvaluatedNativeBinding;
    let selection = &supply.selection;
    let author_identity = &selection.producer.callable_identity;
    let source_occurrence_digest = selection.source_occurrence_digest;
    let (locator, binding_digest) = match &selection.supply {
        build_time_evaluation::native_bindings::NativeBindingSupply::Locator(
            NativeBindingLocator::Import(raw),
        ) => {
            let locator = target::normalize_foreign_locator(raw.clone(), target)
                .map_err(|reason| format!("native Build locator: {reason}"))?;
            let digest = locator.identity_digest();
            (Some(locator), digest)
        }
        build_time_evaluation::native_bindings::NativeBindingSupply::Locator(
            NativeBindingLocator::Syscall { number },
        ) => {
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
        build_time_evaluation::native_bindings::NativeBindingSupply::Table(table) => {
            let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
                b"omega.native-table-designation.v1\0",
            );
            digest.bytes(target.target_name().as_bytes());
            digest.bytes(requirement.as_bytes());
            digest.bytes(_typed.symbols.display_path(table.table, "::").as_bytes());
            digest.bytes(_typed.symbols.display_path(table.field, "::").as_bytes());
            digest.byte(u8::from(table.drops_receiver));
            (
                None,
                target::ForeignLocatorIdentityDigest::from_digest(digest.finish()),
            )
        }
    };
    let mut closure = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-build-producer-closure.v2\0",
    );
    closure.raw(
        supply
            .evaluation
            .terminal_code
            .program_fingerprint
            .as_bytes(),
    );
    closure.raw(&source_occurrence_digest);
    closure.bytes(requirement.as_bytes());
    closure.raw(&binding_digest.as_bytes());
    if let Some(demand) = &selection.same_stack_demand {
        closure.byte(1);
        closure.u64(demand.bytes);
        closure.raw(&demand.source_occurrence_digest);
    } else {
        closure.byte(0);
    }
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
    Ok(match (locator, &selection.supply) {
        (Some(locator), _) => EvaluatedNativeBinding::Import(
            EvaluatedForeignImport::from_retained_evidence(locator, receipt)?,
        ),
        (
            None,
            build_time_evaluation::native_bindings::NativeBindingSupply::Locator(
                NativeBindingLocator::Syscall { number },
            ),
        ) => EvaluatedNativeBinding::Syscall(EvaluatedForeignSyscall::from_retained_evidence(
            target, *number, receipt,
        )?),
        (None, build_time_evaluation::native_bindings::NativeBindingSupply::Table(table)) => {
            EvaluatedNativeBinding::Table {
                target,
                table: _typed.symbols.display_path(table.table, "::"),
                field: _typed.symbols.name(table.field).to_owned(),
                drops_receiver: table.drops_receiver,
                receipt,
            }
        }
        _ => return Err("native Build materialization changed locator mechanism".into()),
    })
}

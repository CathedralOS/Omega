// Build-supplied native binding candidate formation and locator evaluation.
// Whole native maps retain their supplier, exact requirements, and completed
// Build evaluation without a realization machine: source visibility was admitted
// on the designated operands, and each irreducible row refines that
// requirement's independently evaluated calling application. Neutral schemas
// come from the prepared Product catalog, and dormant targets retain an absent
// service surface without turning it into an eager source-preparation error. Every
// authored target's locator is checked under that target, while only the
// requested target realization consumes this compilation's evaluated calling
// geometry; equal bindings for the same requirement collapse into a single row
// with accumulated equivalent origins.

use super::NativeBuildSupply;
use crate::provider_planning::trait_name_text;
use crate::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderRowRealization, ProviderSchemaDeclaration,
};
use checked_trees::typed_trees::service_declarations::RetainedServiceSurface;
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

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBuildRowOrigin {
    pub application: Option<
        arena::Handle<checked_trees::typed_trees::product_operand::ProductOperandApplication>,
    >,
    pub requirement: symbols::SymbolHandle,
    pub origin: package_compilation::PackageNativeBindingOrigin,
}

pub struct PreparedNativeBuildBindings {
    rows: Vec<PreparedNativeBuildBinding>,
}

struct PreparedNativeBuildBinding {
    selection: build_time_evaluation::native_bindings::NativeBindingSelection,
    requirement: Result<PreparedNativeRequirement, Vec<Diagnostic>>,
}

struct PreparedNativeRequirement {
    application: Option<String>,
    owner: symbols::SymbolHandle,
    signature: symbols::SymbolHandle,
    identity: String,
    declared_identity: String,
    calling_signature: Option<semantic_vocabulary::CallingSignatureId>,
    method: String,
    surface: Option<RetainedServiceSurface>,
}

impl PreparedNativeBuildBindings {
    pub fn capture(services: ServiceCatalog<'_>, supplies: &[NativeBuildSupply]) -> Self {
        let typed = services.context().typed;
        let rows = supplies
            .iter()
            .map(|supply| {
                let selection = &supply.selection;
                let fail = |reason| {
                    vec![Diagnostic::error(reason).with_source_span(selection.source_span)]
                };
                let requirement = (|| {
                    let owner = typed
                        .trait_definition_by_symbol(selection.subject)
                        .filter(|owner| owner.is_boundary)
                        .ok_or_else(|| fail("native Build requirement lost its boundary owner"))?;
                    if selection.application.is_none() && (!typed.trait_type_parameters(owner).is_empty() || !owner.lifetime_parameters.is_empty()) {
                        return Err(fail("native Build requirement lost its closed boundary application"));
                    }
                    let selected = typed.closed_requirement_application(selection.requirement);
                    let template = selected.as_ref().map_or(selection.requirement, |selected| selected.template);
                    let (declaring, signature) = typed.trait_machine_signature(template)
                        .filter(|(declaring, _)| declaring.symbol == owner.symbol)
                        .ok_or_else(|| fail("native Build requirement lost its exact declared signature"))?;
                    let application = selection.application.map(|handle| {
                        typed.product_operand_applications.is_valid(handle)
                            .then(|| typed.product_operand_applications.get(handle))
                            .filter(|application| application.role == Some(language_semantics::declaration_selection::ProductOperandRole::Requirement)
                                && application.declaration == selection.requirement)
                            .ok_or_else(|| fail("native Build requirement lost its prepared application"))
                    }).transpose()?;
                    let scope = application.map(|application| application.scope.scope(typed)
                        .filter(|scope| scope.declaration.symbol == declaring.symbol)
                        .ok_or_else(|| fail("native Build requirement changed its applied boundary owner"))).transpose()?;
                    let application_identity = scope.as_ref().map(|scope| scope.portable_application_identity(typed)
                        .and_then(|identity| typed.type_identity_text(identity))
                        .ok_or_else(|| fail("native Build requirement lost its portable outer application"))).transpose()?;
                    let declared_identity = typed.trait_requirement_callable_identity_text(owner, signature);
                    let (identity, calling_signature) = match selected {
                        Some(selected) => {
                            let calling = services.signatures().abi_for_boundary(selected.signature.symbol)
                                .ok_or_else(|| fail("native Build requirement lost its prepared declaration ABI"))?
                                .as_ref().map_err(|reason| fail(reason.as_str()))?;
                            (typed.trait_requirement_callable_identity_text(owner, selected.signature), Some(*calling))
                        }
                        None => (declared_identity.clone(), None),
                    };
                    Ok(PreparedNativeRequirement {
                        application: application_identity,
                        owner: owner.symbol,
                        signature: signature.symbol,
                        identity,
                        declared_identity,
                        calling_signature,
                        method: signature.name.as_str().to_owned(),
                        surface: match scope {
                            Some(scope) => services.retain_application(scope),
                            None => services.retain_declaration(ProviderSchemaDeclaration::BoundaryTrait(owner.symbol)),
                        },
                    })
                })();
                PreparedNativeBuildBinding {
                    selection: selection.clone(),
                    requirement,
                }
            })
            .collect();
        Self { rows }
    }

    pub(crate) fn form(
        &self,
        supplies: &[NativeBuildSupply],
        selected_target: Option<target::TargetProfile>,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> Result<Vec<DerivedProviderPlan>, Vec<Diagnostic>> {
        if self.rows.len() != supplies.len() {
            return Err(vec![Diagnostic::error(
                "native Build supplies differ from the prepared declaration roster",
            )]);
        }
        let mut plans: Vec<DerivedProviderPlan> = Vec::new();
        for (prepared_binding, supply) in self.rows.iter().zip(supplies) {
            let selection = &supply.selection;
            let fail = |reason: String| {
                vec![Diagnostic::error(reason).with_source_span(selection.source_span)]
            };
            if prepared_binding.selection != *selection {
                return Err(fail(
                    "native Build supply differs from its prepared declaration".into(),
                ));
            }
            let requirement = prepared_binding
                .requirement
                .as_ref()
                .map_err(Clone::clone)?;
            let target = target::TargetProfile::from_canonical_target_name(
                selection.target.canonical_name(),
            )
            .map_err(|error| fail(error.to_string()))?;
            let evaluated =
                evaluate(supply, &requirement.identity, target, identities).map_err(fail)?;
            let producer = selection.producer.clone();
            let production = package_compilation::NativeBindingProduction {
                producer,
                evaluated,
            };
            if selected_target != Some(target) {
                continue;
            }
            let prepared = requirement
                .surface
                .as_ref()
                .ok_or_else(|| {
                    fail("native Build requirement has no complete service schema".into())
                })?
                .surface()
                .map_err(|error| vec![error])?;
            let schema = crate::service_schema::attach_calling(&prepared, &[]);
            let group = plans.iter().position(|candidate| {
                candidate.provenance.schema.symbol() == requirement.owner
                    && candidate.plan.target == target.target_name()
                    && candidate.plan.origin_package_identity == Some(selection.supplier_package)
                    && candidate
                        .plan
                        .identities
                        .text(candidate.plan.provider_application)
                        == requirement.application.as_deref().unwrap_or("")
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
                    let mut provider =
                        format!("native-build:{supplier}:{}", trait_name_text(&schema));
                    if let Some(application) = &requirement.application {
                        let digest =
                            crate::digest_encoder::content_identity(application.as_bytes());
                        provider.push(':');
                        for byte in digest {
                            use std::fmt::Write;
                            write!(provider, "{byte:02x}").expect("String formatting");
                        }
                    }
                    let name = crate::satisfies_plan_name(
                        target.target_name(),
                        trait_name_text(&schema),
                        &provider,
                    );
                    let mut plan_identities = ProviderIdentityTable::default();
                    let provider_application = requirement
                        .application
                        .as_deref()
                        .map(|application| plan_identities.intern(application))
                        .unwrap_or_default();
                    plans.push(DerivedProviderPlan {
                        plan: ProviderPlan {
                            name,
                            provider_type: provider,
                            provider_type_package_identity: Some(selection.supplier_package),
                            provider_application,
                            target: target.target_name().to_owned(),
                            schema,
                            rows: Vec::new(),
                            identities: plan_identities,
                            origin_package_identity: Some(selection.supplier_package),
                            origin_package: supplier,
                            digest_memo: Default::default(),
                        },
                        provenance: ProviderPlanProvenance {
                            schema: ProviderSchemaDeclaration::BoundaryTrait(requirement.owner),
                            provider_type: None,
                            named_application: None,
                            row_requirements: Vec::new(),
                            row_realizations: Vec::new(),
                            row_target_machine_origins: Vec::new(),
                            row_accessors: Vec::new(),
                        },
                    });
                    plans.len() - 1
                }
            };
            let candidate = &mut plans[index];
            if let Some(prior) = candidate.plan.rows.iter().position(|row| {
                candidate.plan.identities.text(row.requirement_identity) == requirement.identity
            }) {
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
                Arc::make_mut(origin).origin.equivalents.push(production);
                continue;
            }
            candidate.plan.rows.push(ProviderPlanRow {
                method: requirement.method.clone(),
                requirement_identity: candidate.plan.identities.intern(&requirement.identity),
                declared_requirement_identity: candidate
                    .plan
                    .identities
                    .intern(&requirement.declared_identity),
                calling_signature: requirement.calling_signature,
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: 0,
                binding: production.evaluated.provider_binding(),
            });
            candidate
                .provenance
                .row_requirements
                .push(requirement.signature);
            candidate
                .provenance
                .row_realizations
                .push(ProviderRowRealization::BuildNative(Arc::new(
                    NativeBuildRowOrigin {
                        application: selection.application,
                        requirement: requirement.signature,
                        origin: package_compilation::PackageNativeBindingOrigin {
                            primary: production,
                            equivalents: Vec::new(),
                        },
                    },
                )));
            candidate.provenance.row_target_machine_origins.push(None);
            candidate.provenance.row_accessors.push(None);
        }
        Ok(plans)
    }
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
    let (work_schedule, storage_schedule) = match usage.measurement {
        evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule,
            storage_schedule,
        } => (work_schedule.marker(), storage_schedule.marker()),
        _ => return Err("native Build evidence has a different evaluator measurement".into()),
    };
    let retained_usage = EvaluatedBindingUsage::from_evaluator(
        usage.usage_schema_version,
        usage.step_schedule_marker,
        usage.work_units,
        usage.invocation_work_budget,
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
    let requirement_identity = identities.intern(requirement);
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        Some(selection.supplier_package),
        author_identity,
        identities,
        closure,
        work_schedule,
        retained_usage,
        evaluation,
        2,
        materialization,
        binding_digest,
    )?
    .with_build_origin(
        BuildNativeBindingOrigin {
            requirement_identity,
            source_occurrence_digest,
            terminal_code: supply.evaluation.terminal_code,
            work_schedule,
            storage_schedule,
            observation_digest: supply.evaluation.observation_digest,
        },
        identities.as_table(),
    )?;
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

// We join selected physical ABI applications (`selected_calling_applications`
// and `component_calling_applications`) from canonical declarations. External
// calls and independent checked entries consume the same exact requirement,
// inherited receiver, and policy-root keys without consulting a Source catalog,
// symbol correspondence, or repeated policy validation.

use crate::provider_planning::trait_name_text;
use diagnostics::Diagnostic;
use semantic_vocabulary::{CallingAbiIdentity, CallingDeclarationOwner};

pub(crate) fn selected_calling_applications(
    module: &terminal_psi::TerminalModule,
    explicit: &[super::BoundaryCallingPlanRealization],
    plan: &effects::provider_plan::ProviderPlan,
    row_index: usize,
) -> Result<SelectedCallingApplications, Diagnostic> {
    let row = plan
        .rows
        .get(row_index)
        .ok_or_else(|| Diagnostic::error("selected boundary entry lost its provider row"))?;
    let requirement = plan.identities.text(row.declared_requirement_identity);
    if requirement.is_empty() {
        return Err(Diagnostic::error(
            "selected boundary entry has no exact requirement identity",
        ));
    }
    let mut methods = plan
        .schema
        .declaration
        .methods
        .iter()
        .filter(|method| plan.schema.identity_text(method.requirement_identity) == requirement);
    let method = methods
        .next()
        .ok_or_else(|| Diagnostic::error("selected ABI row has no exact schema requirement"))?;
    if methods.next().is_some() || method.name != row.method {
        return Err(Diagnostic::error(
            "selected ABI row has competing or changed schema correspondence",
        ));
    }
    let declarations = module
        .calling_abi_signatures
        .iter()
        .filter(|declaration| {
            module.identities.text(declaration.requirement_identity) == Some(requirement)
                && row
                    .calling_signature
                    .is_none_or(|signature| signature == declaration.id)
        })
        .collect::<Vec<_>>();
    if row.calling_signature.is_some() && declarations.len() != 1 {
        return Err(Diagnostic::error(
            "selected closed method lost its exact published declaration ABI",
        ));
    }
    for declaration in &declarations {
        declaration
            .physical_owner
            .as_ref()
            .map_err(|reason| Diagnostic::error(reason.clone()))?;
    }
    let calling = plan
        .schema
        .calling_for_requirement(method.requirement_identity)
        .into_iter()
        .filter(|application| {
            row.calling_signature
                .is_none_or(|signature| application.abi.id() == signature)
        })
        .collect::<Vec<_>>();
    if !calling.is_empty()
        && declarations.iter().any(|declaration| {
            declaration.physical_owner == Ok(CallingDeclarationOwner::TopLevelRequirement)
        })
    {
        return Err(Diagnostic::error(
            "selected non-trait boundary retains a trait calling application",
        ));
    }
    let default_abis =
        default_calling_abis(module, plan, method.requirement_identity, &declarations)
            .map_err(Diagnostic::error);
    let mut applications = Vec::with_capacity(calling.len());
    for calling in calling {
        let abi = &calling.abi;
        let declaration = module.calling_abi_signature(abi.id()).ok_or_else(|| {
            Diagnostic::error("selected calling application has no exact published declaration ABI")
        })?;
        let published = declaration
            .identity(&module.identities)
            .map_err(Diagnostic::error)?;
        if &published != abi
            || abi.requirement_identity != requirement
            || !receiver_accepts(module, plan, method.requirement_identity, declaration)?
        {
            return Err(Diagnostic::error(
                "selected calling application differs from its exact inherited receiver",
            ));
        }
        let root = plan.schema.root_application_identity().ok_or_else(|| {
            Diagnostic::error("selected trait ABI has no complete calling owner application")
        })?;
        let mut selected = None;
        for realization in explicit.iter().filter(|realization| {
            realization.calling_owner_application() == root
                && realization.materialized_signature().abi() == abi
        }) {
            if realization.report_fingerprint == 0
                || realization.report_fingerprint != calling.plan.report_fingerprint
                || realization.commitment != calling.plan.commitment
            {
                return Err(Diagnostic::error(
                    "selected ABI application has stale policy custody",
                ));
            }
            let application = realization
                .native_application()
                .map_err(Diagnostic::error)?;
            if selected.as_ref().is_some_and(|prior| prior != &application) {
                return Err(Diagnostic::error(
                    "selected ABI application has competing policy results",
                ));
            }
            selected = Some(application);
        }
        applications.push(selected.ok_or_else(|| {
            Diagnostic::error(format!(
                "selected boundary entry `{}::{}` has no exact calling application",
                trait_name_text(&plan.schema),
                row.method
            ))
        })?);
    }
    Ok(SelectedCallingApplications {
        applications,
        default_abis,
    })
}

fn receiver_accepts(
    module: &terminal_psi::TerminalModule,
    plan: &effects::provider_plan::ProviderPlan,
    requirement: effects::provider_plan::ProviderIdentityId,
    declaration: &terminal_psi::CallingAbiDeclaration,
) -> Result<bool, Diagnostic> {
    match declaration
        .physical_owner
        .as_ref()
        .map_err(|reason| Diagnostic::error(reason.clone()))?
    {
        CallingDeclarationOwner::TopLevelRequirement => {
            Ok(plan.schema.declaration.receiver_applications.is_empty())
        }
        CallingDeclarationOwner::BoundaryTrait => {
            let application = module
                .identities
                .text(declaration.declaring_application)
                .ok_or_else(|| {
                    Diagnostic::error("declaration ABI lost its declaring application")
                })?;
            Ok(plan
                .schema
                .declaration
                .receiver_applications
                .iter()
                .any(|receiver| {
                    receiver.requirement_identities.contains(&requirement)
                        && plan.schema.identity_text(receiver.application) == application
                }))
        }
    }
}

fn default_calling_abis(
    module: &terminal_psi::TerminalModule,
    plan: &effects::provider_plan::ProviderPlan,
    requirement: effects::provider_plan::ProviderIdentityId,
    declarations: &[&terminal_psi::CallingAbiDeclaration],
) -> Result<Vec<CallingAbiIdentity>, String> {
    let mut keys = Vec::new();
    for declaration in declarations {
        if !default_abi_eligible(declaration)
            || !receiver_accepts(module, plan, requirement, declaration)
                .map_err(|error| error.message)?
        {
            continue;
        }
        let key = declaration.identity(&module.identities)?;
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    Ok(keys)
}

fn default_abi_eligible(declaration: &terminal_psi::CallingAbiDeclaration) -> bool {
    let formals = declaration
        .formals
        .iter()
        .filter(|formal| formal.callback.is_none());
    declaration.default_eligible
        && formals.clone().count() == declaration.operands.len()
        && formals.zip(&declaration.operands).all(|(formal, operand)| {
            formal.ordinal == operand.ordinal && formal.kind == operand.kind
        })
}

/// Native bindings use the requirement's published ABI and the calling policy
/// of their actual mechanism. A syscall must not inherit the platform C ABI.
pub fn native_calling_applications(
    profile: target::TargetProfile,
    program: &super::CallingProgram,
    bindings: &[crate::target_supply::NormalizedNativeSupply],
) -> Result<Vec<calling_conventions::BoundaryCallingApplication>, Vec<Diagnostic>> {
    if bindings.is_empty() {
        return Ok(Vec::new());
    }
    let module = program.artifact.semantic_module();
    let failure = |reason: String| vec![Diagnostic::error(reason)];
    let mut applications = Vec::new();
    for supplied in bindings {
        if supplied.binding.target() != profile {
            return Err(failure(
                "native calling application has another target".into(),
            ));
        }
        let declaration = module
            .boundary_machine(supplied.boundary)
            .and_then(|boundary| boundary.declaration.as_ref())
            .ok_or_else(|| {
                failure("native calling application lost its boundary declaration".into())
            })?;
        let requirement = module
            .identities
            .text(declaration.source.callable)
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                failure("native calling application lost its exact callable identity".into())
            })?;
        let name = language_semantics::type_identity::callable_display_path(requirement);
        let signature = module
            .calling_signatures
            .iter()
            .find(|signature| signature.boundary == supplied.boundary)
            .ok_or_else(|| {
                failure(format!(
                    "native boundary `{name}` has no calling declaration"
                ))
            })?;
        let application = *signature
            .application
            .as_ref()
            .map_err(|reason| failure(reason.clone()))?;
        let abi = module
            .calling_abi_signature(application)
            .ok_or_else(|| failure(format!("native boundary `{name}` lost its declaration ABI")))?;
        if !default_abi_eligible(abi) {
            return Err(failure(format!(
                "native boundary `{name}` needs its selected custom calling policy"
            )));
        }
        abi.physical_owner
            .as_ref()
            .map_err(|reason| failure(reason.clone()))?;
        let key = abi.identity(&module.identities).map_err(failure)?;
        if !declaration
            .matches_calling_abi(&module.identities, &key)
            .map_err(failure)?
        {
            return Err(failure(format!(
                "native boundary `{name}` has another declaration's calling ABI"
            )));
        }
        let policy = match &supplied.binding {
            effects::provider_plan::EvaluatedNativeBinding::Import(_) => {
                calling_conventions::CallingPolicy::native_for_target(profile.native_target())
            }
            effects::provider_plan::EvaluatedNativeBinding::Syscall(_) => {
                calling_conventions::CallingPolicy::native_syscall_for_target(
                    profile.native_target(),
                )
                .ok_or_else(|| failure("native syscall has no target calling policy".into()))?
            }
        };
        for application in super::plan_computation::evaluate_default_calling_applications(
            program,
            std::slice::from_ref(&key),
            profile,
            policy,
            None,
        )
        .map_err(|reason| {
            failure(format!(
                "cannot prepare native calling plan for `{name}`: {reason}"
            ))
        })? {
            if !applications.contains(&application) {
                applications.push(application);
            }
        }
    }
    Ok(applications)
}

pub(crate) struct SelectedCallingApplications {
    pub applications: Vec<calling_conventions::BoundaryCallingApplication>,
    pub default_abis: Result<Vec<CallingAbiIdentity>, Diagnostic>,
}

pub fn component_calling_applications(
    profile: target::TargetProfile,
    program: &super::CallingProgram,
    providers: &[&effects::provider_plan::ProviderPlan],
    explicit: &[super::BoundaryCallingPlanRealization],
) -> Result<Vec<calling_conventions::BoundaryCallingApplication>, Vec<Diagnostic>> {
    let native_target = profile.native_realization().ok_or_else(|| {
        vec![Diagnostic::error(
            "component calling application target has no native realization",
        )]
    })?;
    let mut applications = Vec::<calling_conventions::BoundaryCallingApplication>::new();
    for &plan in providers {
        if !plan.rows.iter().any(|row| {
            matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            )
        }) {
            continue;
        }
        let plan_profile = target::TargetProfile::from_canonical_target_name(&plan.target)
            .map_err(|error| vec![error])?;
        let layout_target = layout::LayoutTarget::for_profile(profile)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if plan_profile != profile || layout_target.native_target() != native_target {
            return Err(vec![Diagnostic::error(
                "component ABI request names another selected native target",
            )]);
        }
        for (row_index, row) in plan.rows.iter().enumerate() {
            if !matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            ) {
                continue;
            }
            let trait_name = trait_name_text(&plan.schema);
            let selected = selected_calling_applications(
                program.artifact.semantic_module(),
                explicit,
                plan,
                row_index,
            )
            .map_err(|error| vec![error])?;
            let mut selected_applications = selected.applications;
            if selected_applications.is_empty() {
                selected_applications.extend(
                    super::plan_computation::evaluate_default_calling_applications(
                        program,
                        selected.default_abis.as_ref().map_err(|error| vec![error.clone()])?,
                        profile,
                        calling_conventions::CallingPolicy::native_for_target(native_target),
                        None,
                    )
                    .map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "cannot materialize component calling application for `{trait_name}::{}`: {reason}",
                            row.method
                        ))]
                    })?,
                );
                if selected_applications.is_empty() {
                    return Err(vec![Diagnostic::error(format!(
                        "component boundary `{trait_name}::{}` has no exact materializable declaration application",
                        row.method
                    ))]);
                }
            }
            for application in selected_applications {
                if std::env::var_os("OMEGA_TRACE_COMPONENT_CALLING").is_some() {
                    eprintln!(
                        "component ABI materialized: package={:?} plan={:?} application={application:#?}",
                        plan.origin_package_identity,
                        plan.identity_digest()
                    );
                }
                if !applications.contains(&application) {
                    applications.push(application);
                }
            }
        }
    }
    Ok(applications)
}

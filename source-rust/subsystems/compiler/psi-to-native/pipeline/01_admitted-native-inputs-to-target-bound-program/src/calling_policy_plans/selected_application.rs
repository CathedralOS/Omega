//! Join selected physical ABI applications to canonical declarations and policy roots.

use diagnostics::Diagnostic;

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

/// Materialize the declaration's native default only if its author selected no custom policy.
pub(crate) fn component_calling_application(
    program: &super::CallingProgram,
    profile: target::TargetProfile,
    boundary: semantic_vocabulary::BoundaryMachineId,
    settled: &[calling_conventions::BoundaryCallingApplication],
) -> Result<calling_conventions::BoundaryCallingApplication, String> {
    let module = program.artifact.semantic_module();
    let declaration = module.boundary_calling_abi(boundary)?;
    let key = declaration.identity(&module.identities)?;
    let mut offered = settled
        .iter()
        .filter(|row| row.abi == key && row.target == profile.native_target());
    if let Some(row) = offered.next() {
        if offered.next().is_some() {
            return Err("component callable has competing physical calling applications".into());
        }
        return Ok(row.clone());
    }
    for request in &module.interfaces.calling_policy_requests {
        let instances = request
            .instances
            .as_ref()
            .map_err(|errors| errors.join("; "))?;
        if instances.iter().any(|row| {
            module.identities.text(row.declaring_application)
                == Some(key.declaring_application.as_str())
        }) {
            return Err("component callable lost its selected custom calling application".into());
        }
    }
    if !default_abi_eligible(declaration) {
        return Err("component callable needs an explicitly selected calling policy".into());
    }
    let mut rows = super::plan_computation::evaluate_default_calling_applications(
        program,
        &[key],
        profile,
        calling_conventions::CallingPolicy::native_for_target(profile.native_target()),
        None,
    )?;
    rows.pop()
        .ok_or_else(|| "component callable lost its default calling application".into())
}

/// A row may be needed both for execution and for package review. Reuse only
/// the default ABI computed from this exact artifact, target and binding receipt.
pub(crate) struct NativeCallingResults {
    target: target::TargetProfile,
    artifact: [u8; 32],
    by_binding:
        std::collections::BTreeMap<(semantic_vocabulary::BoundaryMachineId, [u8; 32]), usize>,
    pub(crate) applications: Vec<calling_conventions::BoundaryCallingApplication>,
}

pub(crate) fn native_calling_results(
    profile: target::TargetProfile,
    program: &super::CallingProgram,
    bindings: &[crate::target_supply::NormalizedNativeSupply],
    selected: &[calling_conventions::BoundaryCallingApplication],
    retained: Option<&NativeCallingResults>,
) -> Result<NativeCallingResults, Vec<Diagnostic>> {
    let artifact = *program.artifact.manifest().identity().as_bytes();
    let failure = |reason: String| vec![Diagnostic::error(reason)];
    if retained.is_some_and(|prior| prior.target != profile || prior.artifact != artifact) {
        return Err(failure(
            "retained default calling plans name another program or deployment".into(),
        ));
    }
    let module = program.artifact.semantic_module();
    let mut result = NativeCallingResults {
        target: profile,
        artifact,
        by_binding: Default::default(),
        applications: Vec::with_capacity(bindings.len()),
    };
    for supplied in bindings {
        let origin = (
            supplied.boundary,
            supplied.binding.receipt().identity_digest(),
        );
        if let Some(application) = retained.and_then(|prior| {
            prior
                .by_binding
                .get(&origin)
                .map(|&index| &prior.applications[index])
        }) {
            result.retain(origin, application.clone());
            continue;
        }
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
        let name = application_identity::type_identity::callable_display_path(requirement);
        let signature = module
            .interfaces.calling_signatures
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
        if let Some(application) = selected.iter().find(|application| application.abi == key && application.target == profile.native_target()) {
            result.retain(origin, application.clone());
            continue;
        }
        if !default_abi_eligible(abi) {
            return Err(failure(format!(
                "native boundary `{name}` needs its selected custom calling policy"
            )));
        }
        let policy = match &supplied.binding {
            build_providers::plans::EvaluatedNativeBinding::Import(_)
            | build_providers::plans::EvaluatedNativeBinding::FunctionField(_) => {
                calling_conventions::CallingPolicy::native_for_target(profile.native_target())
            }
            build_providers::plans::EvaluatedNativeBinding::Syscall(_) => {
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
            result.retain(origin, application);
        }
    }
    Ok(result)
}

impl NativeCallingResults {
    fn retain(
        &mut self,
        origin: (semantic_vocabulary::BoundaryMachineId, [u8; 32]),
        application: calling_conventions::BoundaryCallingApplication,
    ) {
        let index = self
            .applications
            .iter()
            .position(|prior| prior == &application)
            .unwrap_or_else(|| {
                self.applications.push(application);
                self.applications.len() - 1
            });
        self.by_binding.insert(origin, index);
    }
}

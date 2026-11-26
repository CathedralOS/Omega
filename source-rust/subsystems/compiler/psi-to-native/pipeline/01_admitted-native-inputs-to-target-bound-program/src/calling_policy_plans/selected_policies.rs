//! Evaluate source-selected Calling policies for reached execution and independent package review.
//! A binding receipt may reuse a settled default. Each selected declaration and signature
//! executes at most once for this artifact, deployment and authorization scope.
use super::{CallingProgram, NativeCallingResults};
use crate::target_supply::NormalizedNativeSupply;
use calling_conventions::BoundaryCallingApplication;
use declaration_scope::DeclarationSelectionAuthority;
use diagnostics::Diagnostic;
use semantic_vocabulary::CallingSignatureId;
use std::collections::BTreeMap;
use target::TargetProfile;
use terminal_psi::{CallingPolicyInstance, TerminalIdentityIndex};

fn refuse(reason: String) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}

// A request with unavailable ownership is still a selected policy request.
// Validate that roster before deciding whether a native binding has no policy.
fn calling_policy_instances(
    module: &terminal_psi::TerminalModule,
) -> Result<
    Vec<(
        terminal_psi::TerminalIdentityIndex,
        &terminal_psi::CallingPolicyInstance,
    )>,
    String,
> {
    let mut instances = Vec::new();
    for request in &module.interfaces.calling_policy_requests {
        let declaration = *request.declaration.as_ref().map_err(Clone::clone)?;
        if module
            .identities
            .text(declaration)
            .is_none_or(str::is_empty)
        {
            return Err("selected Calling policy lost its source declaration".into());
        }
        let applications = request
            .instances
            .as_ref()
            .map_err(|reasons| reasons.join("; "))?;
        if applications.is_empty() {
            return Err("selected Calling policy has no retained application".into());
        }
        instances.extend(applications.iter().map(|instance| (declaration, instance)));
    }
    Ok(instances)
}

fn require_unselected_calling_policy(
    instances: &[(
        terminal_psi::TerminalIdentityIndex,
        &terminal_psi::CallingPolicyInstance,
    )],
    application: terminal_psi::TerminalIdentityIndex,
) -> Result<(), String> {
    if instances
        .iter()
        .any(|(_, instance)| instance.declaring_application == application)
    {
        return Err("native binding has no result from its selected Calling policy".into());
    }
    Ok(())
}

pub(crate) struct SelectedPolicies<'program, 'artifact> {
    calling: &'artifact CallingProgram,
    terminal: &'program terminal_interpreter::TerminalProgram<'artifact>,
    profile: TargetProfile,
    authority: Option<&'program dyn DeclarationSelectionAuthority>,
    requests: Vec<(TerminalIdentityIndex, &'artifact CallingPolicyInstance)>,
    analysis: Option<terminal_interpreter::TerminalCallableAnalysis<'program, 'artifact>>,
    evaluated: BTreeMap<(usize, CallingSignatureId), BoundaryCallingApplication>,
}

impl<'program, 'artifact> SelectedPolicies<'program, 'artifact> {
    pub fn new(
        calling: &'artifact CallingProgram,
        terminal: &'program terminal_interpreter::TerminalProgram<'artifact>,
        profile: TargetProfile,
        authority: Option<&'program dyn DeclarationSelectionAuthority>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let artifact = &calling.artifact;
        artifact
            .publication_metadata()
            .ok_or_else(|| {
                refuse(
                    "saved Psi lacks calling declaration owners; recompile it to publish them"
                        .into(),
                )
            })?
            .require_calling_custody()
            .map_err(refuse)?;
        Ok(Self {
            calling,
            terminal,
            profile,
            authority,
            requests: calling_policy_instances(artifact.semantic_module()).map_err(refuse)?,
            analysis: None,
            evaluated: BTreeMap::new(),
        })
    }

    fn evaluate(
        &mut self,
        index: usize,
        signature: CallingSignatureId,
    ) -> Result<BoundaryCallingApplication, Vec<Diagnostic>> {
        if let Some(application) = self.evaluated.get(&(index, signature)) {
            return Ok(application.clone());
        }
        let (declaration, instance) = self.requests[index];
        let artifact = &self.calling.artifact;
        let requester = artifact
            .calling_declaration_owner(artifact.view().into(), declaration)
            .map_err(refuse)?;
        let providers =
            psi_provider_selections::admit_provider_installation(artifact.view(), &[], &[])
                .map_err(|error| refuse(format!("{error:?}")))?;
        let analysis = self
            .analysis
            .get_or_insert_with(|| self.terminal.callable_analysis(Some(&providers)));
        let admission = evaluation_admission::admit_compiler_evaluation(
            artifact.view(),
            self.profile.target_name(),
            *instance
                .policy_declaration
                .as_ref()
                .map_err(|reason| refuse(reason.clone()))?,
            providers,
            requester,
            self.authority,
        )
        .map_err(refuse)?;
        let application = self
            .calling
            .evaluate(analysis, instance, self.profile, signature, &[], admission)
            .map_err(refuse)?;
        self.evaluated
            .insert((index, signature), application.clone());
        Ok(application)
    }

    pub fn plan(
        &mut self,
        bindings: &[NormalizedNativeSupply],
        extra_requirements: &[&str],
        extra_schemas: &[(TerminalIdentityIndex, TerminalIdentityIndex)],
        retained: Option<&NativeCallingResults>,
    ) -> Result<(Vec<BoundaryCallingApplication>, NativeCallingResults), Vec<Diagnostic>> {
        let calling = self.calling;
        let module = calling.artifact.semantic_module();
        let mut requirements = extra_requirements.to_vec();
        let mut schemas = extra_schemas.to_vec();
        let mut calling_applications = Vec::new();
        for supply in bindings {
            let schema = direct_boundary_bindings::DirectBoundaryLeaf::declared_schema(
                module,
                supply.boundary,
            )
            .map_err(refuse)?;
            schemas.push((schema.declaration, schema.application));
            let abi = module
                .boundary_calling_abi(supply.boundary)
                .map_err(refuse)?;
            requirements.push(
                module
                    .identities
                    .text(abi.requirement_identity)
                    .ok_or_else(|| {
                        refuse("native binding lost its original method template".into())
                    })?,
            );
        }
        requirements.sort_unstable();
        requirements.dedup();
        schemas.sort_unstable();
        schemas.dedup();
        for (schema_declaration, schema_application) in schemas {
            let schema = module
                .service_schema(schema_declaration, schema_application)
                .map_err(refuse)?;
            if !schema.surface.methods.iter().any(|method| {
                requirements
                    .binary_search(&schema.surface.identity_text(method.requirement_identity))
                    .is_ok()
            }) {
                continue;
            }
            // Nominal schema ownership and source-selection custody have different
            // identities. The retained full declaring application joins them; its
            // request still supplies the authenticated semantic source declaration.
            for index in 0..self.requests.len() {
                let (_, instance) = self.requests[index];
                if instance.declaring_application != schema_application {
                    continue;
                }
                for &signature in instance
                    .signatures
                    .as_ref()
                    .map_err(|reason| refuse(reason.clone()))?
                {
                    let abi = module.calling_abi_signature(signature).ok_or_else(|| {
                        refuse("provider policy names an absent ABI declaration".into())
                    })?;
                    let requirement = module
                        .identities
                        .text(abi.requirement_identity)
                        .ok_or_else(|| {
                            refuse("provider ABI lost its requirement identity".into())
                        })?;
                    if requirements.binary_search(&requirement).is_err() {
                        continue;
                    }
                    let application = self.evaluate(index, signature)?;
                    if let Some(previous) = calling_applications.iter().find(
                        |previous: &&calling_conventions::BoundaryCallingApplication| {
                            previous.abi == application.abi
                        },
                    ) {
                        if previous != &application {
                            return Err(refuse(
                                "provider ABI has competing calling policy results".into(),
                            ));
                        }
                    } else {
                        calling_applications.push(application);
                    }
                }
            }
        }
        // Explicit selections remain authoritative. Only an undeclared policy uses
        // the existing native default planner; a failed or missing selected result
        // cannot silently become a default ABI.
        for supply in bindings {
            let abi = direct_boundary_bindings::DirectBoundaryLeaf::declared_calling_abi(
                module,
                supply.boundary,
            )
            .map_err(refuse)?
            .identity(&module.identities)
            .map_err(refuse)?;
            if calling_applications
                .iter()
                .any(|application| application.abi == abi)
            {
                continue;
            }
            let schema = direct_boundary_bindings::DirectBoundaryLeaf::declared_schema(
                module,
                supply.boundary,
            )
            .map_err(refuse)?;
            require_unselected_calling_policy(&self.requests, schema.application)
                .map_err(refuse)?;
        }
        let settled = crate::calling_policy_plans::native_calling_results(
            self.profile,
            self.calling,
            bindings,
            &calling_applications,
            retained,
        )?;
        for application in &settled.applications {
            if !calling_applications.contains(application) {
                calling_applications.push(application.clone());
            }
        }
        Ok::<_, Vec<Diagnostic>>((calling_applications, settled))
    }
}

#[cfg(test)]
mod tests {
    use super::{calling_policy_instances, require_unselected_calling_policy};

    #[test]
    fn unavailable_selected_calling_policy_is_not_an_absent_policy() {
        let mut module = terminal_psi::TerminalModule::declarations();
        assert!(calling_policy_instances(&module).unwrap().is_empty());
        module
            .interfaces
            .calling_policy_requests
            .push(terminal_psi::CallingPolicyRequest {
                declaration: Err("selected owner is unavailable".into()),
                display_name: "selected".into(),
                instances: Ok(Vec::new()),
            });
        assert_eq!(
            calling_policy_instances(&module).unwrap_err(),
            "selected owner is unavailable"
        );
    }
    #[test]
    fn native_default_requires_absence_of_the_exact_policy_application() {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let declaration = identities.intern("semantic declaration");
        let selected = identities.intern("selected full application");
        let other = identities.intern("another full application");
        let mut module = terminal_psi::TerminalModule::declarations();
        module.identities = identities.build();
        require_unselected_calling_policy(&calling_policy_instances(&module).unwrap(), selected)
            .unwrap();
        module
            .interfaces
            .calling_policy_requests
            .push(terminal_psi::CallingPolicyRequest {
                declaration: Ok(declaration),
                display_name: "selected".into(),
                instances: Ok(vec![terminal_psi::CallingPolicyInstance {
                    declaring_application: selected,
                    policy_declaration: Err("unavailable policy declaration".into()),
                    policy: Err("unavailable policy body".into()),
                    signatures: Err("missing evaluated Calling result".into()),
                }]),
            });
        let instances = calling_policy_instances(&module).unwrap();
        assert_eq!(
            require_unselected_calling_policy(&instances, selected).unwrap_err(),
            "native binding has no result from its selected Calling policy"
        );
        require_unselected_calling_policy(&instances, other).unwrap();
    }
}

//! Run the selected entry policy on its actual semantic and physical signatures.
//! Both results stay joined to the same target-owned storage contract.

use super::SelectedProgramEntryCallingPlans;
use crate::calling_policy_plans::CallingProgram;
use program_entry_contracts::{
    ProgramEntryPhysicalContractPackageSourceDigest, ProgramEntryPhysicalContractPlan,
    SelectedProgramStorageEntryPlan,
};
use build_providers::plans::{
    BoundaryCallingPlanCommitment, BoundaryCallingPlanPromise, ServiceCallingApplication,
    ServiceCallingPlan, ServiceSchema,
};
use semantic_vocabulary::CallingSignatureId;
use terminal_psi::{CallingPolicyInstance, TerminalModule};

/// The checked program supplies the exact schema, requirement identities and
/// physical source commitment. Package permission remains a separate input.
pub struct ProgramEntryCallingPlanRequest<'source> {
    pub slot: target::ProgramEntrySlotDeclaration,
    pub schema: ServiceSchema,
    pub declaring_schema: &'source str,
    pub semantic_requirement: &'source str,
    pub physical_requirement: &'source str,
    pub physical_source_digest: [u8; 32],
    pub providers: &'source [build_providers::plans::ProviderPlan],
    pub admission: crate::calling_policy_plans::TerminalCallingPolicyAdmission,
}

pub fn entry_calling_policy<'module>(
    module: &'module TerminalModule,
    declaration: &str,
    owner: &str,
) -> Result<&'module CallingPolicyInstance, String> {
    let mut selected = None;
    for request in &module.interfaces.calling_policy_requests {
        if request
            .declaration
            .as_ref()
            .ok()
            .and_then(|identity| module.identities.text(*identity))
            != Some(declaration)
        {
            continue;
        }
        let instances = request
            .instances
            .as_ref()
            .map_err(|reasons| reasons.join("; "))?;
        for instance in instances {
            if module.identities.text(instance.declaring_application) != Some(owner) {
                continue;
            }
            if selected.replace(instance).is_some() {
                return Err(format!(
                    "entry schema `{owner}` has competing checked calling-policy instances"
                ));
            }
        }
    }
    selected.ok_or_else(|| format!(
        "no product route yet: entry schema `{owner}` has no calling policy in its accepted program",
    ))
}

fn signature(
    module: &TerminalModule,
    policy: &CallingPolicyInstance,
    requirement: &str,
) -> Result<CallingSignatureId, String> {
    let signatures = policy.signatures.as_ref().map_err(Clone::clone)?;
    let mut selected = None;
    for &identity in signatures {
        let declaration = module
            .calling_abi_signature(identity)
            .ok_or("entry calling policy names an absent ABI declaration")?;
        if module.identities.text(declaration.requirement_identity) != Some(requirement) {
            continue;
        }
        if selected.replace(identity).is_some() {
            return Err(format!(
                "entry requirement `{requirement}` has competing checked calling signatures"
            ));
        }
    }
    selected.ok_or_else(|| format!(
        "no product route yet: entry requirement `{requirement}` has no checked calling signature",
    ))
}

impl SelectedProgramEntryCallingPlans {
    pub fn evaluate(
        program: &CallingProgram,
        request: ProgramEntryCallingPlanRequest<'_>,
    ) -> Result<Self, String> {
        let ProgramEntryCallingPlanRequest {
            slot,
            mut schema,
            declaring_schema,
            semantic_requirement,
            physical_requirement,
            physical_source_digest,
            providers,
            admission,
        } = request;
        let package = slot
            .physical_contract_package
            .ok_or("entry slot has no physical contract package")?;
        let source_digest = ProgramEntryPhysicalContractPackageSourceDigest::verify(
            package,
            physical_source_digest,
        )?;
        let owner = schema
            .root_application_identity()
            .ok_or("entry schema has no exact root application identity")?
            .to_owned();
        for requirement in [semantic_requirement, physical_requirement] {
            let mut methods =
                schema.declaration.methods.iter().filter(|method| {
                    schema.identity_text(method.requirement_identity) == requirement
                });
            if methods.next().is_none() || methods.next().is_some() {
                return Err(format!(
                    "entry schema `{owner}` does not contain exactly one `{requirement}`"
                ));
            }
        }
        let module = program.artifact.semantic_module();
        if declaring_schema.is_empty() {
            return Err("entry schema has no canonical source declaration identity".into());
        }
        let policy = entry_calling_policy(module, declaring_schema, &owner)?;
        let semantic = signature(module, policy, semantic_requirement)?;
        let physical = signature(module, policy, physical_requirement)?;
        if semantic == physical {
            return Err("semantic and physical entry signatures must be distinct".into());
        }
        let target = slot.owner;
        let terminal = program
            .terminal_program()
            .map_err(|error| error.to_string())?;
        let analysis = terminal.callable_analysis(Some(admission.providers()));
        let semantic_calling_application = program.evaluate(
            &analysis,
            policy,
            target,
            semantic,
            providers,
            admission.clone(),
        )?;
        let physical_calling_application =
            program.evaluate(&analysis, policy, target, physical, providers, admission)?;
        for application in [&semantic_calling_application, &physical_calling_application] {
            if application.dispatch_only_formal.is_some() {
                return Err("a program entry cannot omit a table lookup argument".into());
            }
            let requirement = schema
                .declaration
                .identities
                .id_at(&application.requirement_identity)
                .ok_or("evaluated entry application names a requirement outside its schema")?;
            if schema.calling_application(&application.abi).is_some() {
                return Err("entry schema already contains this calling application".into());
            }
            schema.calling_applications.push(ServiceCallingApplication {
                requirement,
                abi: application.abi.clone(),
                plan: ServiceCallingPlan {
                    report_fingerprint: application.report_fingerprint,
                    commitment: BoundaryCallingPlanCommitment::from_digest(application.commitment),
                    promise: BoundaryCallingPlanPromise::from_digest(
                        application.plan.contract_commitment_digest(),
                    ),
                },
            });
        }
        let method = schema
            .declaration
            .methods
            .iter()
            .find(|method| {
                schema.identity_text(method.requirement_identity) == physical_requirement
            })
            .ok_or("entry schema lost its physical requirement")?;
        let parameters = method
            .parameter_type_identities
            .iter()
            .map(|identity| schema.identity_text(*identity).to_owned())
            .collect();
        let result = schema.identity_text(method.result_type_identity).to_owned();
        let physical_contract = ProgramEntryPhysicalContractPlan::new(
            slot,
            physical_requirement.to_owned(),
            source_digest,
            parameters,
            result,
            physical_calling_application
                .plan
                .contract_report_fingerprint(),
            physical_calling_application.plan.plan().clone(),
        )?;
        let storage_entry = SelectedProgramStorageEntryPlan::from_target_slot(
            slot,
            schema,
            semantic_requirement.to_owned(),
        )
        .map_err(|reason| reason.to_string())?
        .with_physical_contract(physical_contract)
        .map_err(|reason| reason.to_string())?;
        Ok(Self {
            semantic_calling_application,
            physical_calling_application,
            storage_entry,
        })
    }
}

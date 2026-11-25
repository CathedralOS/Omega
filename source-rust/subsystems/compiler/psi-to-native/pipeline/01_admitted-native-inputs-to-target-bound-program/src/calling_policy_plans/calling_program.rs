// We define `CallingProgram`: the shared calling declarations, executable
// policies, and original review coordinates published by the compiler's ordinary
// Psi production request. Canonical admission precedes each activation, and
// pure compiler-run policies execute under a finite 100,000-unit semantic work
// allowance with independent logical storage metering and no host imports.

use super::{CanonicalCallingSignature, TerminalCallingPolicyInvocation};
use calling_conventions::ValidatedBoundaryEntryPlan;
use evaluation_accounting::BuildEvaluationSponsor;
use evaluation_contracts::{
    BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity, TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::MachineId;
use terminal_codec::CanonicalTerminalArtifact;

const CALLING_EVALUATION_WORK: u64 = 100_000;

enum CallingStorageScope<'account> {
    Policy {
        entry: MachineId,
        account: &'account evaluation_accounting::EvaluationInvocationAccount,
        admission: &'account super::TerminalCallingPolicyAdmission,
    },
    Description,
}

#[derive(Debug)]
pub struct CallingProgram {
    pub artifact: std::sync::Arc<CanonicalTerminalArtifact>,
}

impl PartialEq for CallingProgram {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.artifact, &other.artifact)
    }
}

impl Eq for CallingProgram {}

impl CallingProgram {
    /// The interpreter's view of this artifact. Build one view and one
    /// [`terminal_interpreter::TerminalCallableAnalysis`] per provider
    /// selection, and pass that analysis to every [`Self::evaluate`] under
    /// those providers, so the whole-program callable projection is built once
    /// rather than twice per policy.
    pub fn terminal_program(&self) -> terminal_interpreter::TerminalProgram<'_> {
        terminal_interpreter::TerminalProgram::borrow(self.artifact.view())
    }

    pub fn evaluate(
        &self,
        analysis: &terminal_interpreter::TerminalCallableAnalysis<'_, '_>,
        policy: &terminal_psi::CallingPolicyInstance,
        target: target::TargetProfile,
        declaration: semantic_vocabulary::CallingSignatureId,
        providers: &[provider_plans::ProviderPlan],
        admission: super::TerminalCallingPolicyAdmission,
    ) -> Result<(CanonicalCallingSignature, ValidatedBoundaryEntryPlan), String> {
        let policy = *policy.policy.as_ref().map_err(Clone::clone)?;
        let measurement = EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        };
        let limits = BuildEvaluationSponsorLimits::new(
            CALLING_EVALUATION_WORK,
            1,
            1,
            1,
            1_000_000,
            1_000_000,
            1_000_000,
            1_000_000,
        )
        .ok_or("invalid compiler calling-policy resource limits")?;
        let sponsor = BuildEvaluationSponsor::new_with_measurement(limits, measurement)
            .map_err(|error| error.to_string())?;
        let account = sponsor
            .begin_invocation(measurement, limits.work_budget())
            .map_err(|error| error.to_string())?;
        let selected = psi_target_layouts::terminal_target_context(
            self.artifact.view(),
            target.target_name(),
        );
        let selected_entry = selected
            .entry(
                selected.terminal_psi(),
                self.artifact.semantic_module(),
                policy,
            )
            .map_err(|error| format!("calling policy canonical selection: {error}"))?;
        admission.require_invocation(
            self.artifact.view(),
            target.target_name(),
            terminal_psi::TargetMachineCallable::Machine(selected_entry),
        )?;
        let abi = self
            .artifact
            .semantic_module()
            .calling_abi_signature(declaration)
            .ok_or("calling policy lost its exact declaration ABI")?
            .identity(&self.artifact.semantic_module().identities)?;
        let canonical = self.materialize_storage(
            &abi,
            target,
            CallingStorageScope::Policy {
                entry: selected_entry,
                account: &account,
                admission: &admission,
            },
            providers,
        )?;
        let (plan, _) =
            super::execution::evaluate_terminal_calling_policy(TerminalCallingPolicyInvocation {
                artifact: &self.artifact,
                analysis,
                entry: policy,
                target,
                signature: &canonical.policy,
                account,
                admission,
            })?;
        Ok((canonical, plan))
    }

    pub(crate) fn materialize_default(
        &self,
        declaration: &semantic_vocabulary::CallingAbiIdentity,
        target: target::TargetProfile,
    ) -> Result<CanonicalCallingSignature, String> {
        let row = self
            .artifact
            .semantic_module()
            .calling_abi_signature(declaration.id())
            .ok_or("default calling application has no published ABI declaration")?;
        if !row.default_eligible {
            return Err(
                "default calling application is outside the Product declaration census".into(),
            );
        }
        row.physical_owner.as_ref().map_err(Clone::clone)?;
        self.materialize_storage(declaration, target, CallingStorageScope::Description, &[])
    }

    fn materialize_storage(
        &self,
        declaration: &semantic_vocabulary::CallingAbiIdentity,
        target: target::TargetProfile,
        scope: CallingStorageScope<'_>,
        providers: &[provider_plans::ProviderPlan],
    ) -> Result<CanonicalCallingSignature, String> {
        let demand = psi_scalar_applications::CallingAbiStorageDemand::new(
            self.artifact.view(),
            target.target_name(),
            declaration,
        )?;
        self.with_storage_extents(&demand, target, scope, providers, |extents| {
            CanonicalCallingSignature::materialize(
                self.artifact.closed_module(),
                demand.declaration().id(),
                target,
                extents,
            )
        })
    }

    fn with_storage_extents<Output>(
        &self,
        demand: &psi_scalar_applications::CallingAbiStorageDemand,
        target: target::TargetProfile,
        scope: CallingStorageScope<'_>,
        providers: &[provider_plans::ProviderPlan],
        describe: impl FnOnce(
            &dyn Fn(
                semantic_vocabulary::ScalarApplicationId,
            ) -> Result<u64, terminal_semantics::StorageGeometryError>,
        ) -> Result<Output, String>,
    ) -> Result<Output, String> {
        let mut extents = Vec::with_capacity(demand.applications().len());
        if !demand.applications().is_empty() {
            let default_providers;
            let providers = match &scope {
                CallingStorageScope::Policy { admission, .. } => admission.providers(),
                CallingStorageScope::Description => {
                    default_providers = target_provider_admission::TerminalTargetProviderSelection::from_plans(
                        providers.to_vec(),
                        self.artifact.view(),
                    )?
                    .admit(self.artifact.view(), target.target_name())
                    .map_err(|error| format!("calling ABI scalar providers: {error}"))?;
                    &default_providers
                }
            };
            let (request, mut meter) = match scope {
                CallingStorageScope::Policy { entry, account, .. } => {
                    let declaration = self
                        .artifact
                        .view()
                        .module()
                        .scalars.scalar_applications
                        .proof_scope
                        .callable_declaration(terminal_psi::TargetMachineCallable::Machine(entry))
                        .ok_or("calling policy entry has no exact checked proof declaration")?;
                    (
                        super::scalar_completion::CallingScalarScope::Invocation {
                            declaration,
                            applications: demand.applications(),
                        },
                        evaluation_work::sponsored_meter(account.clone(), None)
                            .map_err(|error| format!("calling ABI scalar work: {error:?}"))?,
                    )
                }
                CallingStorageScope::Description => (
                    super::scalar_completion::CallingScalarScope::AbiStorage(demand),
                    evaluation_work::unsponsored_meter(Some(CALLING_EVALUATION_WORK)),
                ),
            };
            let completed = super::scalar_completion::complete_calling_scalar_inputs(
                &self.artifact,
                target,
                request,
                providers,
                &mut meter,
            )?;
            for &application in demand.applications() {
                let value = match completed.values().value(application) {
                    Some(terminal_semantics::entry_values::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(value),
                        ..
                    }) => u64::try_from(value).map_err(|_| "calling ABI extent exceeds u64")?,
                    Some(terminal_semantics::entry_values::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Signed(value),
                        ..
                    }) => u64::try_from(value)
                        .map_err(|_| "calling ABI extent is negative or exceeds u64")?,
                    _ => return Err("calling ABI extent has no completed integer value".into()),
                };
                extents.push((application, value));
            }
        }
        describe(&|application| {
            extents
                .iter()
                .find(|(identity, _)| *identity == application)
                .map(|(_, value)| *value)
                .ok_or(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                    application,
                ))
        })
    }
}

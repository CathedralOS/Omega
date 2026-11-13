// We define `CallingProgram`: the shared calling declarations, executable
// policies, and original review coordinates published by the compiler's ordinary
// Psi production request. Canonical admission precedes each activation, and
// pure compiler-run policies execute under a finite 100,000-unit semantic work
// allowance with independent logical storage metering and no host imports.

use super::{CallingPolicyEvaluator, CanonicalCallingSignature, TerminalCallingPolicyInvocation};
use calling_conventions::ValidatedBoundaryEntryPlan;
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
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
    pub source_owner: std::sync::Weak<checked_trees::CheckedTrees>, // association only; never keeps source alive
    pub artifact: std::sync::Arc<CanonicalTerminalArtifact>,
    pub source_records: std::sync::Arc<[checked_trees::typed_trees::SourceRecordCorrespondence]>,
    pub source_preparation: std::sync::Arc<super::CallingSourcePreparation>,
    pub evaluator: CallingPolicyEvaluator, // Build evaluation's executors; omitted from catalog equality
}

impl PartialEq for CallingProgram {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Weak::ptr_eq(&self.source_owner, &other.source_owner)
            && self.artifact == other.artifact
            && self.source_records == other.source_records
            && self.source_preparation == other.source_preparation
    }
}

impl Eq for CallingProgram {}

impl CallingProgram {
    /// Whether this boundary trait declares a `Calling<Policy>` relationship,
    /// whatever applications of it were evaluated.
    pub(crate) fn declares_calling(&self, boundary: symbols::SymbolHandle) -> bool {
        self.source_preparation
            .requests
            .iter()
            .any(|request| request.boundary == boundary)
    }

    pub(crate) fn require_owner(
        &self,
        source_owner: &std::sync::Weak<checked_trees::CheckedTrees>,
    ) -> Result<(), String> {
        if std::sync::Weak::ptr_eq(&self.source_owner, source_owner) {
            return Ok(());
        }
        Err("calling-policy code belongs to a different checked source owner".into())
    }

    pub(crate) fn evaluate(
        &self,
        policy: &terminal_psi::CallingPolicyInstance,
        target: target::TargetProfile,
        declaration: semantic_vocabulary::CallingSignatureId,
        providers: &[backend_plan::TerminalTargetProviderPlan],
        requester: semantic_vocabulary::compiler_evaluation::DeclarationCustody,
        authority: Option<
            &dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority,
        >,
    ) -> Result<(CanonicalCallingSignature, ValidatedBoundaryEntryPlan), String> {
        let declaration_root = *policy.policy_declaration.as_ref().map_err(Clone::clone)?;
        let admission = super::admit_terminal_calling_policy(
            &self.artifact,
            declaration_root,
            target,
            providers,
            requester,
            authority,
        )?;
        let policy = *policy.policy.as_ref().map_err(Clone::clone)?;
        let measurement = EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
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
        let selected = terminal_installation::admit_target_machine_selections(
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
        let (plan, _) = (self.evaluator.policy)(TerminalCallingPolicyInvocation {
            artifact: &self.artifact,
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
        providers: &[backend_plan::TerminalTargetProviderPlan],
    ) -> Result<CanonicalCallingSignature, String> {
        let demand = terminal_installation::CallingAbiStorageDemand::new(
            self.artifact.view(),
            target.target_name(),
            declaration,
        )?;
        self.with_storage_extents(&demand, target, scope, providers, |extents| {
            CanonicalCallingSignature::materialize(
                self.artifact.semantic_module(),
                demand.declaration().id(),
                target,
                extents,
            )
        })
    }

    pub(crate) fn describe_borrowed_storage<Output>(
        &self,
        declaration: &semantic_vocabulary::CallingAbiIdentity,
        target: target::TargetProfile,
        ordinal: usize,
        describe: impl FnOnce(
            &dyn Fn(
                semantic_vocabulary::ScalarApplicationId,
            ) -> Result<u64, terminal_semantics::StorageGeometryError>,
        ) -> Result<Output, String>,
    ) -> Result<Output, String> {
        let demand = terminal_installation::CallingAbiStorageDemand::new(
            self.artifact.view(),
            target.target_name(),
            declaration,
        )?
        .with_borrowed_parameter(self.artifact.view(), ordinal)?;
        self.with_storage_extents(
            &demand,
            target,
            CallingStorageScope::Description,
            &[],
            describe,
        )
    }

    fn with_storage_extents<Output>(
        &self,
        demand: &terminal_installation::CallingAbiStorageDemand,
        target: target::TargetProfile,
        scope: CallingStorageScope<'_>,
        providers: &[backend_plan::TerminalTargetProviderPlan],
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
                    default_providers = backend_plan::TerminalTargetProviderSelection::from_plans(
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
                        .scalar_applications
                        .proof_scope
                        .callable_declaration(terminal_psi::TargetMachineCallable::Machine(entry))
                        .ok_or("calling policy entry has no exact checked proof declaration")?;
                    (
                        super::scalar_completion::CallingScalarScope::Invocation {
                            declaration,
                            applications: demand.applications(),
                        },
                        psi_work::WorkMeter::with_invocation(account.clone(), None)
                            .map_err(|error| format!("calling ABI scalar work: {error:?}"))?,
                    )
                }
                CallingStorageScope::Description => (
                    super::scalar_completion::CallingScalarScope::AbiStorage(demand),
                    psi_work::WorkMeter::with_budget(CALLING_EVALUATION_WORK),
                ),
            };
            let completed = super::scalar_completion::complete_calling_scalar_inputs(
                &self.artifact,
                target,
                request,
                providers,
                self.evaluator.scalar_conditions,
                &mut meter,
            )?;
            for &application in demand.applications() {
                let value = match completed.values().value(application) {
                    Some(terminal_installation::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(value),
                        ..
                    }) => u64::try_from(value).map_err(|_| "calling ABI extent exceeds u64")?,
                    Some(terminal_installation::TerminalScalarValue::Integer {
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

//! Normalized activation demands and task-runtime behavior admission.
//!
//! Suspension safety is a local code/effect check. CPU affinity, host-thread
//! affinity, and address stability are demand/behavior joins selected by the
//! runtime's preemption granularity.

macro_rules! normalized_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, TaskPlanDiagnostic> {
                if identity == 0 {
                    return Err(TaskPlanDiagnostic(format!(
                        "normalized {} identity cannot be zero",
                        $label
                    )));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

normalized_id!(MachineContractId, "machine-contract");
normalized_id!(MachineEntryId, "machine-entry");
normalized_id!(ValueLayoutId, "value-layout");
normalized_id!(CallingPlanId, "calling-plan");
normalized_id!(TaskRuntimeId, "task-runtime");
normalized_id!(ActivationAdmissionId, "activation-admission");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameCpuDemand {
    Any,
    Same,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameThreadDemand {
    Any,
    Same,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressStabilityDemand {
    Movable,
    Stable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationDemand {
    pub cpu: SameCpuDemand,
    pub thread: SameThreadDemand,
    pub address: AddressStabilityDemand,
}

impl MigrationDemand {
    pub const fn unconstrained() -> Self {
        Self {
            cpu: SameCpuDemand::Any,
            thread: SameThreadDemand::Any,
            address: AddressStabilityDemand::Movable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistinctActivationRequirement {
    Required,
    InlineCompletionAllowed,
}

/// Provider-independent output of compile-time machine target elaboration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationPlanCandidate {
    pub machine_contract: MachineContractId,
    pub entry: MachineEntryId,
    pub argument_layout: ValueLayoutId,
    pub terminal_outcome_layout: ValueLayoutId,
    pub calling_plan: CallingPlanId,
    pub continuation_bytes: u64,
    pub continuation_alignment: u64,
    pub reaches_suspend: bool,
    /// Result of local canonical-liveness × carry checking at possible parks.
    pub suspension_crossings_safe: bool,
    pub safe_point_migration: MigrationDemand,
    /// Required because asynchronous preemption makes every instruction a
    /// potential provider crossing. `None` means no such analysis was emitted.
    pub asynchronous_migration: Option<MigrationDemand>,
    pub cancellation_required: bool,
    pub activation: DistinctActivationRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedActivationPlan(ActivationPlanCandidate);

impl ValidatedActivationPlan {
    pub const fn candidate(&self) -> &ActivationPlanCandidate {
        &self.0
    }
}

pub fn validate_activation_plan(
    candidate: ActivationPlanCandidate,
) -> Result<ValidatedActivationPlan, TaskPlanDiagnostic> {
    if candidate.continuation_bytes == 0 {
        return Err(TaskPlanDiagnostic(
            "activation continuation size must be nonzero".into(),
        ));
    }
    if candidate.continuation_alignment == 0 || !candidate.continuation_alignment.is_power_of_two()
    {
        return Err(TaskPlanDiagnostic(format!(
            "activation continuation alignment {} is not a nonzero power of two",
            candidate.continuation_alignment
        )));
    }
    if candidate.reaches_suspend && !candidate.suspension_crossings_safe {
        return Err(TaskPlanDiagnostic(
            "a possible Suspend crossing carries a value that forbids suspension".into(),
        ));
    }
    Ok(ValidatedActivationPlan(candidate))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreemptionGranularity {
    SafePoints,
    Asynchronous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuMigrationBehavior {
    MayMigrate,
    Pinned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadMigrationBehavior {
    MayMigrate,
    Pinned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationStorageBehavior {
    Movable,
    Stable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionBehavior {
    Never,
    MayCompleteInline,
}

/// Behavior promised by an admitted provider. Opaque/host claims enter only
/// through provider admission; missing evidence must use `pessimistic`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRuntimeContract {
    pub runtime: TaskRuntimeId,
    pub max_continuation_bytes: u64,
    pub max_continuation_alignment: u64,
    pub preemption: PreemptionGranularity,
    pub cpu_migration: CpuMigrationBehavior,
    pub thread_migration: ThreadMigrationBehavior,
    pub continuation_storage: ContinuationStorageBehavior,
    pub cancellation: bool,
    pub inline_completion: InlineCompletionBehavior,
}

impl TaskRuntimeContract {
    pub const fn pessimistic(runtime: TaskRuntimeId) -> Self {
        Self {
            runtime,
            max_continuation_bytes: 0,
            max_continuation_alignment: 1,
            preemption: PreemptionGranularity::Asynchronous,
            cpu_migration: CpuMigrationBehavior::MayMigrate,
            thread_migration: ThreadMigrationBehavior::MayMigrate,
            continuation_storage: ContinuationStorageBehavior::Movable,
            cancellation: false,
            inline_completion: InlineCompletionBehavior::MayCompleteInline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRuntimeAdmission {
    identity: ActivationAdmissionId,
    runtime: TaskRuntimeId,
    machine_contract: MachineContractId,
    selected_migration_demand: MigrationDemand,
}

impl TaskRuntimeAdmission {
    pub const fn identity(&self) -> ActivationAdmissionId {
        self.identity
    }

    pub const fn runtime(&self) -> TaskRuntimeId {
        self.runtime
    }

    pub const fn selected_migration_demand(&self) -> MigrationDemand {
        self.selected_migration_demand
    }
}

pub fn admit_activation(
    plan: &ValidatedActivationPlan,
    runtime: &TaskRuntimeContract,
    admission: ActivationAdmissionId,
) -> Result<TaskRuntimeAdmission, TaskPlanDiagnostic> {
    let plan = plan.candidate();
    if plan.continuation_bytes > runtime.max_continuation_bytes {
        return Err(TaskPlanDiagnostic(format!(
            "activation needs {} continuation bytes but runtime admits {}",
            plan.continuation_bytes, runtime.max_continuation_bytes
        )));
    }
    if runtime.max_continuation_alignment == 0
        || plan.continuation_alignment > runtime.max_continuation_alignment
        || !runtime
            .max_continuation_alignment
            .is_multiple_of(plan.continuation_alignment)
    {
        return Err(TaskPlanDiagnostic(format!(
            "activation alignment {} is incompatible with runtime alignment {}",
            plan.continuation_alignment, runtime.max_continuation_alignment
        )));
    }
    if plan.cancellation_required && !runtime.cancellation {
        return Err(TaskPlanDiagnostic(
            "activation requires cancellation but runtime does not provide it".into(),
        ));
    }
    if runtime.inline_completion == InlineCompletionBehavior::MayCompleteInline
        && plan.activation == DistinctActivationRequirement::Required
    {
        return Err(TaskPlanDiagnostic(
            "runtime may complete inline but activation contract requires distinct execution"
                .into(),
        ));
    }

    let demand = match runtime.preemption {
        PreemptionGranularity::SafePoints => plan.safe_point_migration,
        PreemptionGranularity::Asynchronous => plan.asynchronous_migration.ok_or_else(|| {
            TaskPlanDiagnostic(
                "asynchronous runtime requires an all-instruction carry analysis".into(),
            )
        })?,
    };
    if demand.cpu == SameCpuDemand::Same && runtime.cpu_migration != CpuMigrationBehavior::Pinned {
        return Err(TaskPlanDiagnostic(
            "activation requires same-CPU execution but runtime may migrate it".into(),
        ));
    }
    if demand.thread == SameThreadDemand::Same
        && runtime.thread_migration != ThreadMigrationBehavior::Pinned
    {
        return Err(TaskPlanDiagnostic(
            "activation requires same-host-thread execution but runtime may migrate it".into(),
        ));
    }
    if demand.address == AddressStabilityDemand::Stable
        && runtime.continuation_storage != ContinuationStorageBehavior::Stable
    {
        return Err(TaskPlanDiagnostic(
            "activation requires stable addresses but runtime may move continuation storage".into(),
        ));
    }

    Ok(TaskRuntimeAdmission {
        identity: admission,
        runtime: runtime.runtime,
        machine_contract: plan.machine_contract,
        selected_migration_demand: demand,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPlanDiagnostic(pub String);

impl std::fmt::Display for TaskPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TaskPlanDiagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, TaskPlanDiagnostic>) -> T {
        constructor(identity).expect("normalized identity")
    }

    fn candidate() -> ActivationPlanCandidate {
        ActivationPlanCandidate {
            machine_contract: id(1, MachineContractId::from_normalized_identity),
            entry: id(2, MachineEntryId::from_normalized_identity),
            argument_layout: id(3, ValueLayoutId::from_normalized_identity),
            terminal_outcome_layout: id(4, ValueLayoutId::from_normalized_identity),
            calling_plan: id(5, CallingPlanId::from_normalized_identity),
            continuation_bytes: 4096,
            continuation_alignment: 16,
            reaches_suspend: true,
            suspension_crossings_safe: true,
            safe_point_migration: MigrationDemand {
                cpu: SameCpuDemand::Same,
                thread: SameThreadDemand::Any,
                address: AddressStabilityDemand::Stable,
            },
            asynchronous_migration: None,
            cancellation_required: true,
            activation: DistinctActivationRequirement::Required,
        }
    }

    fn runtime() -> TaskRuntimeContract {
        TaskRuntimeContract {
            runtime: id(10, TaskRuntimeId::from_normalized_identity),
            max_continuation_bytes: 8192,
            max_continuation_alignment: 64,
            preemption: PreemptionGranularity::SafePoints,
            cpu_migration: CpuMigrationBehavior::Pinned,
            thread_migration: ThreadMigrationBehavior::MayMigrate,
            continuation_storage: ContinuationStorageBehavior::Stable,
            cancellation: true,
            inline_completion: InlineCompletionBehavior::Never,
        }
    }

    #[test]
    fn safe_point_runtime_admits_matching_storage_and_carry_demands() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let admission = admit_activation(
            &plan,
            &runtime(),
            id(20, ActivationAdmissionId::from_normalized_identity),
        )
        .expect("runtime admission");
        assert_eq!(admission.runtime().normalized_identity(), 10);
        assert_eq!(
            admission.selected_migration_demand().cpu,
            SameCpuDemand::Same
        );
    }

    #[test]
    fn suspension_is_rejected_locally_before_runtime_selection() {
        let mut unsafe_plan = candidate();
        unsafe_plan.suspension_crossings_safe = false;
        let error = validate_activation_plan(unsafe_plan).expect_err("unsafe park");
        assert!(error.0.contains("Suspend crossing"));
    }

    #[test]
    fn asynchronous_runtime_requires_its_own_liveness_envelope() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let mut asynchronous = runtime();
        asynchronous.preemption = PreemptionGranularity::Asynchronous;
        let error = admit_activation(
            &plan,
            &asynchronous,
            id(21, ActivationAdmissionId::from_normalized_identity),
        )
        .expect_err("safe-point analysis cannot justify async preemption");
        assert!(error.0.contains("all-instruction"));
    }

    #[test]
    fn migration_storage_and_inline_behavior_fail_closed() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let mut migrating = runtime();
        migrating.cpu_migration = CpuMigrationBehavior::MayMigrate;
        let error = admit_activation(
            &plan,
            &migrating,
            id(22, ActivationAdmissionId::from_normalized_identity),
        )
        .expect_err("same CPU demand");
        assert!(error.0.contains("same-CPU"));

        let mut movable = runtime();
        movable.continuation_storage = ContinuationStorageBehavior::Movable;
        let error = admit_activation(
            &plan,
            &movable,
            id(23, ActivationAdmissionId::from_normalized_identity),
        )
        .expect_err("stable address demand");
        assert!(error.0.contains("stable addresses"));

        let mut inline = runtime();
        inline.inline_completion = InlineCompletionBehavior::MayCompleteInline;
        let error = admit_activation(
            &plan,
            &inline,
            id(24, ActivationAdmissionId::from_normalized_identity),
        )
        .expect_err("distinct activation required");
        assert!(error.0.contains("inline"));
    }

    #[test]
    fn pessimistic_runtime_cannot_accidentally_admit_work() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let runtime =
            TaskRuntimeContract::pessimistic(id(11, TaskRuntimeId::from_normalized_identity));
        assert!(
            admit_activation(
                &plan,
                &runtime,
                id(25, ActivationAdmissionId::from_normalized_identity)
            )
            .is_err()
        );
    }
}

use omega_checked_trees::{
    CheckedTrees, SuspensionCrossingStorage, TaskActivationPlanFact, TaskStartOperation,
};
use omega_core::diagnostics::Diagnostic;
use omega_core::semantics::{
    CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension,
};
use omega_layout::TypeLayout;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_task_plans::{
    ActivationPlanCandidate, AddressStabilityDemand, CallingPlanId, DistinctActivationRequirement,
    MachineContractId, MachineEntryId, MigrationDemand, SameCpuDemand, SameThreadDemand,
    ValueLayoutId, validate_activation_plan,
};

/// Elaborate every concrete `TaskRuntime::{start,try_start}<M>` specialization
/// into a provider-independent activation demand. The source classifier is
/// deliberately nominal and closed: an unrelated method named `start` does
/// not become a task operation.
pub(super) fn elaborate_task_activation_plans(
    program: &mut CheckedTrees,
    target: NativeTarget,
) -> Result<(), Vec<Diagnostic>> {
    let specializations = program
        .machine_specializations
        .iter()
        .filter(|specialization| {
            program
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.instance)
                .is_some_and(|machine| task_start_operation(program, machine).is_some())
        })
        .cloned()
        .collect::<Vec<_>>();
    if specializations.is_empty() {
        program.facts.contract_plans.task_activations.clear();
        return Ok(());
    }
    let layouts = omega_layout::build_layout_plan(program, target).map_err(|error| vec![error])?;
    let mut activations = Vec::new();

    for specialization in specializations {
        let Some(start_machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
        else {
            return Err(vec![Diagnostic::error(format!(
                "static-machine specialization 0x{:016x} names unknown concrete instance {:?}",
                specialization.fingerprint, specialization.instance
            ))]);
        };
        let Some(operation) = task_start_operation(program, start_machine) else {
            continue;
        };
        let [target_entry] = specialization.machine_arguments.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "`{}` task-start specialization must select exactly one target machine, got {}",
                start_machine.name,
                specialization.machine_arguments.len()
            ))]);
        };
        let Some((target_machine, entry)) = program.machines().iter().find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == *target_entry)
                .map(|state| (machine, state))
        }) else {
            return Err(vec![Diagnostic::error(format!(
                "`{}` task-start specialization selects unknown entry symbol {:?}",
                start_machine.name, target_entry
            ))]);
        };
        if !program.machine_type_parameters(target_machine).is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "task activation target `{}` is still generic after specialization",
                target_machine.name
            ))]);
        }

        let Some(contract) = program
            .facts
            .contract_plans
            .for_machine(target_machine.symbol)
        else {
            return Err(vec![Diagnostic::error(format!(
                "task activation target `{}` has no checked machine contract",
                target_machine.name
            ))]);
        };
        let machine_contract = normalized_id(
            contract.fingerprint,
            MachineContractId::from_normalized_identity,
        )?;
        let entry_identity = entry_identity(program, target_machine, entry);
        let entry_id = normalized_id(entry_identity, MachineEntryId::from_normalized_identity)?;
        let argument_layout_identity = signature_layout_identity(
            program,
            target,
            program
                .state_parameters(entry)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| parameter.type_reference),
        )?;
        let argument_layout = normalized_id(
            argument_layout_identity,
            ValueLayoutId::from_normalized_identity,
        )?;
        let outcome_layout_identity =
            signature_layout_identity(program, target, std::iter::once(entry.return_type))?;
        let terminal_outcome_layout = normalized_id(
            outcome_layout_identity,
            ValueLayoutId::from_normalized_identity,
        )?;
        let calling_plan_identity = calling_plan_identity(
            target,
            entry_identity,
            argument_layout_identity,
            outcome_layout_identity,
        );
        let calling_plan = normalized_id(
            calling_plan_identity,
            CallingPlanId::from_normalized_identity,
        )?;

        let reaches_suspend = contract.suspension.checked_may_suspend;
        let crossings = program
            .facts
            .carry
            .suspension_crossings
            .iter()
            .filter(|crossing| crossing.machine == target_machine.symbol)
            .collect::<Vec<_>>();
        // A transparent target that reaches Suspend must have canonical facts
        // for its possible park sites. Missing facts are not evidence of
        // safety: keep the plan validator fail-closed.
        let suspension_crossings_safe = !reaches_suspend
            || (!crossings.is_empty()
                && crossings
                    .iter()
                    .all(|crossing| crossing.effective.suspension == CarrySuspension::Allowed));
        let safe_point_policy = crossings
            .iter()
            .fold(CarryPolicy::PERMISSIVE, |policy, crossing| {
                policy.intersect(crossing.effective)
            });
        let asynchronous_migration = program
            .facts
            .carry
            .preemption_for_machine(target_machine.symbol)
            .filter(|fact| fact.analysis_complete)
            .map(|fact| migration_demand(fact.effective));
        let (continuation_bytes, continuation_alignment) =
            continuation_layout(program, target, &layouts, target_machine, &crossings)?;

        let plan = validate_activation_plan(ActivationPlanCandidate {
            machine_contract,
            entry: entry_id,
            argument_layout,
            terminal_outcome_layout,
            calling_plan,
            continuation_bytes,
            continuation_alignment,
            reaches_suspend,
            suspension_crossings_safe,
            safe_point_migration: migration_demand(safe_point_policy),
            // An asynchronous provider can preempt at every instruction. The
            // checker therefore joins every storage/call/transient value that
            // can exist at an instruction boundary. Incomplete type coverage
            // stays explicit as `None`, so provider admission fails closed.
            asynchronous_migration,
            // `Task<T>` always carries cancellation-request authority. A
            // runtime that cannot accept that request cannot establish this
            // lifecycle claim, even if this compilation unit never happens
            // to call `request_cancel` itself.
            cancellation_required: true,
            activation: DistinctActivationRequirement::Required,
        })
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;

        activations.push(TaskActivationPlanFact {
            start_instance: start_machine.symbol,
            target_machine: target_machine.symbol,
            target_entry: entry.symbol,
            specialization_fingerprint: specialization.fingerprint,
            operation,
            plan,
        });
    }

    program.facts.contract_plans.task_activations = activations;
    Ok(())
}

fn task_start_operation(
    program: &CheckedTrees,
    machine: &omega_checked_trees::machine::Machine,
) -> Option<TaskStartOperation> {
    let attached = machine.attached_data.as_ref()?;
    if attached.as_str().rsplit("::").next() != Some("TaskRuntime")
        || !matches!(
            machine.supply_mode,
            omega_core::semantics::MachineSupplyMode::Boundary
                | omega_core::semantics::MachineSupplyMode::Accepted
        )
        || !program.data_definitions().iter().any(|definition| {
            definition.name == *attached
                && definition.supply_mode == omega_core::semantics::DataSupplyMode::BoundaryOpaque
        })
    {
        return None;
    }
    let entry = program.machine_states(machine).first()?;
    let result = program.display_type_reference(entry.return_type);
    match (machine.name.as_str().rsplit("::").next(), result.as_str()) {
        (Some("start"), result) if result.starts_with("Task<") => Some(TaskStartOperation::Start),
        (Some("try_start"), result) if result.starts_with("StartOutcome<") => {
            Some(TaskStartOperation::TryStart)
        }
        _ => None,
    }
}

fn continuation_layout(
    program: &CheckedTrees,
    target: NativeTarget,
    layouts: &omega_layout::LayoutPlan,
    machine: &omega_checked_trees::machine::Machine,
    crossings: &[&omega_checked_trees::SuspensionCrossingCarryFact],
) -> Result<(u64, u64), Vec<Diagnostic>> {
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find_map(|(_, layout)| (layout.symbol == machine.symbol).then_some(layout.layout))
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "task activation target `{}` has no concrete machine layout",
                machine.name
            ))]
        })?;
    // Every activation needs an explicit resume-state word even when the
    // selected machine has no stored fields and never parks.
    let state_word = TypeLayout {
        size: target.pointer_size,
        alignment: target.pointer_alignment,
    };
    let mut base_size = 0usize;
    let mut base_alignment = 1usize;
    append_layout(&mut base_size, &mut base_alignment, state_word)?;
    append_layout(&mut base_size, &mut base_alignment, machine_layout)?;
    base_size = align_to(base_size, base_alignment)?;

    let mut maximum_size = base_size;
    let mut maximum_alignment = base_alignment;
    for crossing in crossings {
        let mut size = base_size;
        let mut alignment = base_alignment;
        for live in &crossing.live_values {
            if live.storage == SuspensionCrossingStorage::Persistent {
                continue;
            }
            let layout = omega_layout::layout_type_reference(program, target, live.type_reference)
                .map_err(|error| vec![error])?;
            append_layout(&mut size, &mut alignment, layout)?;
        }
        size = align_to(size, alignment)?;
        maximum_size = maximum_size.max(size);
        maximum_alignment = maximum_alignment.max(alignment);
    }
    maximum_size = align_to(maximum_size, maximum_alignment)?;
    Ok((
        u64::try_from(maximum_size)
            .map_err(|_| vec![Diagnostic::error("task continuation size exceeds u64")])?,
        u64::try_from(maximum_alignment)
            .map_err(|_| vec![Diagnostic::error("task continuation alignment exceeds u64")])?,
    ))
}

fn append_layout(
    size: &mut usize,
    alignment: &mut usize,
    layout: TypeLayout,
) -> Result<(), Vec<Diagnostic>> {
    let field_alignment = layout.alignment.max(1);
    *size = align_to(*size, field_alignment)?;
    *size = size
        .checked_add(layout.size)
        .ok_or_else(|| vec![Diagnostic::error("task continuation layout size overflow")])?;
    *alignment = (*alignment).max(field_alignment);
    Ok(())
}

fn align_to(value: usize, alignment: usize) -> Result<usize, Vec<Diagnostic>> {
    value
        .checked_add(alignment.saturating_sub(1))
        .map(|rounded| rounded / alignment * alignment)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task continuation layout alignment overflow",
            )]
        })
}

fn migration_demand(policy: CarryPolicy) -> MigrationDemand {
    MigrationDemand {
        cpu: match policy.cpu {
            CarryCpu::Origin => SameCpuDemand::Same,
            CarryCpu::Any => SameCpuDemand::Any,
        },
        thread: match policy.host_thread {
            CarryHostThread::Origin => SameThreadDemand::Same,
            CarryHostThread::Any => SameThreadDemand::Any,
        },
        address: match policy.address {
            CarryAddress::Stable => AddressStabilityDemand::Stable,
            CarryAddress::Movable => AddressStabilityDemand::Movable,
        },
    }
}

fn signature_layout_identity(
    program: &CheckedTrees,
    target: NativeTarget,
    types: impl IntoIterator<Item = omega_checked_trees::types::TypeReferenceHandle>,
) -> Result<u64, Vec<Diagnostic>> {
    let mut hash = StableHash::new();
    hash.byte(0x51);
    for type_reference in types {
        let layout = omega_layout::layout_type_reference(program, target, type_reference)
            .map_err(|error| vec![error])?;
        hash.string(&program.display_type_reference_with_constraints(type_reference));
        hash.usize(layout.size);
        hash.usize(layout.alignment);
        hash.byte(0xff);
    }
    Ok(hash.finish())
}

fn entry_identity(
    program: &CheckedTrees,
    machine: &omega_checked_trees::machine::Machine,
    entry: &omega_checked_trees::state::State,
) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(0x45);
    hash.string(machine.name.as_str());
    hash.string(entry.name.as_str());
    for parameter in program.state_parameters(entry) {
        hash.byte(u8::from(parameter.is_self));
        hash.byte(u8::from(parameter.is_mutable));
        hash.string(&program.display_type_reference_with_constraints(parameter.type_reference));
    }
    hash.byte(0xfe);
    hash.string(&program.display_type_reference_with_constraints(entry.return_type));
    hash.finish()
}

fn calling_plan_identity(target: NativeTarget, entry: u64, arguments: u64, outcome: u64) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    hash.byte(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    hash.usize(target.pointer_size);
    hash.usize(target.pointer_alignment);
    hash.u64(entry);
    hash.u64(arguments);
    hash.u64(outcome);
    hash.finish()
}

fn normalized_id<T>(
    identity: u64,
    constructor: impl FnOnce(u64) -> Result<T, omega_task_plans::TaskPlanDiagnostic>,
) -> Result<T, Vec<Diagnostic>> {
    constructor(identity.max(1)).map_err(|error| vec![Diagnostic::error(error.to_string())])
}

struct StableHash(u64);

impl StableHash {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.byte(*byte);
        }
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
        self.byte(0);
    }

    fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn finish(self) -> u64 {
        self.0.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_mapping_preserves_each_restrictive_axis() {
        assert_eq!(
            migration_demand(CarryPolicy {
                suspension: CarrySuspension::Allowed,
                cpu: CarryCpu::Origin,
                host_thread: CarryHostThread::Origin,
                address: CarryAddress::Stable,
            }),
            MigrationDemand {
                cpu: SameCpuDemand::Same,
                thread: SameThreadDemand::Same,
                address: AddressStabilityDemand::Stable,
            }
        );
    }

    #[test]
    fn concrete_task_start_specialization_elaborates_a_validated_plan() {
        let source = r#"
            data Task<T> [linear] { provider: u64; activation: u64; }
            machine Task::settle<T>(self) {}
            data StartRejection { code: i32; }
            data StartOutcome<T, Arguments> {
                case Started(task: Task<T>);
                case Rejected(arguments: Arguments, reason: StartRejection);
            }
            boundary data TaskRuntime;
            boundary machine TaskRuntime::start<T, Arguments, machine Target>(
                &self,
                arguments: Arguments
            ) -> Task<T>
            where machine Target(arguments: Arguments) -> T suspends; blocks;
            ensures true;
            boundary machine TaskRuntime::try_start<T, Arguments, machine Target>(
                &self,
                arguments: Arguments
            ) -> StartOutcome<T, Arguments>
            where machine Target(arguments: Arguments) -> T suspends; blocks;
            ensures true;

            boundary data Sleeper;
            boundary machine Sleeper::park(token: i32) suspends;
            data Job { value: i32; }
            data Worker {}
            machine Worker::run(job: Job) -> i32 suspends; {
                let value: i32 = job.value;
                Sleeper::park(value);
                value
            }
            data Main { runtime: &TaskRuntime; }
            machine Main::run(&mut self) {
                let job: Job = Job { value: 7 };
                let task: Task<i32> = self.runtime.start<Worker::run>(job);
                Task::settle(task);
                let retry: Job = Job { value: 9 };
                let outcome: StartOutcome<i32, Job> =
                    self.runtime.try_start<Worker::run>(retry);
                let retry_task: Task<i32> = outcome.task;
                Task::settle(retry_task);
            }
        "#;
        let tokens = omega_source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax = omega_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve");
        let typed =
            omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type");
        let mut checked = omega_typed_trees_to_checked_trees::lower_typed_trees(typed)
            .expect("check and specialize task start");

        elaborate_task_activation_plans(&mut checked, NativeTarget::macos_arm64())
            .expect("elaborate activation plan");
        let activations = checked.facts.contract_plans.task_activations.as_slice();
        assert_eq!(activations.len(), 2);
        let activation = activations
            .iter()
            .find(|activation| activation.operation == TaskStartOperation::Start)
            .expect("start activation plan");
        assert!(
            activations
                .iter()
                .any(|activation| activation.operation == TaskStartOperation::TryStart)
        );
        let target = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == activation.target_machine)
            .expect("target machine");
        assert_eq!(target.name.as_str(), "Worker::run");
        let plan = activation.plan.candidate();
        assert_eq!(plan.continuation_bytes, 16);
        assert_eq!(plan.continuation_alignment, 8);
        assert!(plan.reaches_suspend);
        assert_eq!(
            plan.asynchronous_migration,
            Some(MigrationDemand::unconstrained())
        );
        assert!(plan.cancellation_required);
        assert_ne!(plan.machine_contract.normalized_identity(), 0);
        assert_ne!(plan.argument_layout.normalized_identity(), 0);
        assert_ne!(plan.terminal_outcome_layout.normalized_identity(), 0);
        assert_ne!(
            activation.plan.normalized_identity().normalized_identity(),
            0
        );

        let manifest = omega_visualizations::task_activation_manifest_json(&checked);
        assert!(manifest.contains("\"operation\": \"start\""));
        assert!(manifest.contains("\"operation\": \"try_start\""));
        assert!(manifest.contains("\"target_machine\": \"Worker::run\""));
        assert!(manifest.contains("\"continuation\": {\"bytes\": 16, \"alignment\": 8}"));
        assert!(manifest.contains(
            "\"asynchronous_migration\": {\"cpu\": \"any\", \"thread\": \"any\", \"address\": \"movable\"}"
        ));
        assert!(manifest.contains("\"cancellation_required\": true"));
        assert!(manifest.contains("\"activation_plan_id\": \"0x"));
        assert!(manifest.contains("\"runtime_admission\": {\"status\": \"pending_provider\"}"));
    }
}

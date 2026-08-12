use omega_layout::TypeLayout;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_task_plans::{
    ActivationCarryObligations, ActivationPlanCandidate, CallingPlanId,
    CanonicalSuspensionCrossing, MachineContractId, MachineEntryId,
    SelectedTaskRuntimeProviderFact, StackPlan, StackRepresentationId, SuspensionCrossingId,
    TaskActivationPlanFact, TaskActivationPlanSet, TaskRuntimeId, TaskStartOperation,
    ValueLayoutId, validate_activation_plan,
};
use psi_checked_trees::{CheckedTrees, SuspensionCrossingStorage};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::{CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};

/// Elaborate every concrete `TaskRuntime::{start,try_start}<M>` specialization
/// into a provider-independent activation demand. The source classifier is
/// deliberately nominal and closed: an unrelated method named `start` does
/// not become a task operation.
pub(super) fn elaborate_task_activation_plans(
    program: &CheckedTrees,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    target: NativeTarget,
) -> Result<TaskActivationPlanSet, Vec<Diagnostic>> {
    let selections = task_start_selections(program)?;
    if selections.is_empty() {
        return Ok(TaskActivationPlanSet::default());
    }
    let layouts = omega_layout::build_layout_plan(program, target).map_err(|error| vec![error])?;
    let mut activations = Vec::new();

    for selection in selections {
        let selected_runtime =
            selected_task_runtime_provider(program, selected_provider_plans, &selection)?;
        let target_entry = selection.target_entry;
        let Some((target_machine, entry)) = program.machines().iter().find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == target_entry)
                .map(|state| (machine, state))
        }) else {
            return Err(vec![Diagnostic::error(format!(
                "TaskRuntime start specialization selects unknown entry symbol {:?}",
                target_entry
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

        let may_suspend = contract.suspension.checked_may_suspend;
        let may_block = contract.blocking.checked_may_block;
        let crossings = activation_carry_crossings(program, target_machine.symbol);
        let canonical_suspension_crossings = crossings
            .subtree
            .iter()
            .map(|crossing| canonical_suspension_crossing(program, crossing))
            .collect::<Result<Vec<_>, _>>()?;
        let activation_wide_carry = program
            .facts
            .carry
            .activation_carry_for_machine(target_machine.symbol)
            .filter(|fact| fact.analysis_complete)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "task activation target `{}` has incomplete activation-wide CPU/thread carry analysis",
                    target_machine.name
                ))]
            })?;
        let carry_obligations = carry_obligations(activation_wide_carry.effective);
        let (stack_bytes, stack_alignment) =
            fixed_stack_layout(program, target, &layouts, target_machine, &crossings.root)?;
        let stack_representation = normalized_id(
            stack_representation_identity(target),
            StackRepresentationId::from_normalized_identity,
        )?;

        let plan = validate_activation_plan(ActivationPlanCandidate {
            machine_contract,
            entry: entry_id,
            argument_layout,
            terminal_outcome_layout,
            calling_plan,
            stack_plan: StackPlan {
                bytes: stack_bytes,
                alignment: stack_alignment,
                representation: stack_representation,
            },
            may_suspend,
            may_block,
            // Missing or locally unsafe crossings remain visible so the plan
            // validator rejects them fail-closed.
            canonical_suspension_crossings,
            carry_obligations,
            // `Task<T>` always carries cancellation-request authority. A
            // selected provider must establish that operation later.
            cancellation_required: true,
        })
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;

        activations.push(TaskActivationPlanFact {
            start_requirement: selection.requirement,
            target_machine: target_machine.symbol,
            target_entry: entry.symbol,
            specialization_fingerprint: selection.fingerprint,
            operation: selection.operation,
            selected_runtime,
            plan,
        });
    }

    Ok(TaskActivationPlanSet { activations })
}

fn selected_task_runtime_provider(
    program: &CheckedTrees,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    selection: &TaskStartSelection,
) -> Result<SelectedTaskRuntimeProviderFact, Vec<Diagnostic>> {
    let Some(authored_requirement_identity) = program.traits().iter().find_map(|definition| {
        program
            .trait_machine_signatures(definition)
            .iter()
            .find(|signature| signature.symbol == selection.requirement)
            .map(|signature| {
                program
                    .normalized_trait_requirement_overload_identity(definition, signature)
                    .identity()
            })
    }) else {
        return Err(vec![Diagnostic::error(
            "TaskRuntime activation names an unknown authored requirement",
        )]);
    };
    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| {
            plan.schema.trait_name.rsplit("::").next() == Some("TaskRuntime")
                && plan
                    .schema
                    .methods
                    .iter()
                    .any(|method| method.requirement_identity == authored_requirement_identity)
        })
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(match matches.len() {
            0 => "TaskRuntime boundary slot has no retained selected provider plan".to_owned(),
            count => format!(
                "TaskRuntime boundary slot matches {count} retained selected provider plans"
            ),
        })]);
    };
    let requirement_name = match selection.operation {
        TaskStartOperation::Start => "start",
        TaskStartOperation::TryStart => "try_start",
    };
    let requirements = plan
        .schema
        .methods
        .iter()
        .filter(|method| {
            method.name == requirement_name
                && method.requirement_identity == authored_requirement_identity
        })
        .collect::<Vec<_>>();
    let [method] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(match requirements.len() {
            0 => format!(
                "selected TaskRuntime provider plan `{}` has no `{requirement_name}` requirement",
                plan.name
            ),
            count => format!(
                "selected TaskRuntime provider plan `{}` has {count} `{requirement_name}` requirements; operation binding must be exact",
                plan.name
            ),
        })]);
    };
    if method.requirement_identity != authored_requirement_identity {
        return Err(vec![Diagnostic::error(format!(
            "selected TaskRuntime provider plan `{}` binds `{requirement_name}` as `{}`, but the activation call requires `{authored_requirement_identity}`",
            plan.name, method.requirement_identity
        ))]);
    }
    let covering_rows = plan
        .rows
        .iter()
        .filter(|row| plan.schema.row_binds_method(row, method))
        .count();
    if covering_rows != 1 {
        return Err(vec![Diagnostic::error(format!(
            "selected TaskRuntime provider plan `{}` binds `{requirement_name}` {covering_rows} times; exactly one operation realization is required",
            plan.name
        ))]);
    }
    let runtime = TaskRuntimeId::from_normalized_identity(plan.identity_fingerprint())
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    Ok(SelectedTaskRuntimeProviderFact {
        runtime,
        provider_plan_name: plan.name.clone(),
        requirement_identity: method.requirement_identity.clone(),
    })
}

struct ActivationCarryCrossings<'program> {
    root: Vec<&'program psi_checked_trees::SuspensionCrossingCarryFact>,
    subtree: Vec<&'program psi_checked_trees::SuspensionCrossingCarryFact>,
}

fn activation_carry_crossings(
    program: &CheckedTrees,
    root: psi_symbols::SymbolHandle,
) -> ActivationCarryCrossings<'_> {
    let subtree_machines = program.facts.carry.machine_subtree_symbols(root);
    let subtree = program
        .facts
        .carry
        .suspension_crossings
        .iter()
        .filter(|crossing| subtree_machines.contains(&crossing.machine))
        .collect::<Vec<_>>();
    let root = subtree
        .iter()
        .copied()
        .filter(|crossing| crossing.machine == root)
        .collect();
    ActivationCarryCrossings { root, subtree }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TaskStartSelection {
    requirement: psi_symbols::SymbolHandle,
    target_entry: psi_symbols::SymbolHandle,
    fingerprint: u64,
    operation: TaskStartOperation,
}

fn task_start_selections(
    program: &CheckedTrees,
) -> Result<Vec<TaskStartSelection>, Vec<Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    for (_, expression) in program.expression_table.iter_expressions() {
        let psi_checked_trees::expression::ExpressionNode::Call(call) = expression else {
            continue;
        };
        append_task_start_selection(
            program,
            call.target_symbol,
            call.target.as_str(),
            &call.machine_arguments,
            &mut selections,
            &mut diagnostics,
        );
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let psi_checked_trees::statement::StatementNode::Call(call) = statement else {
                    continue;
                };
                append_task_start_selection(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    &call.machine_arguments,
                    &mut selections,
                    &mut diagnostics,
                );
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    selections.sort_by_key(|selection| selection.fingerprint);
    selections.dedup();
    Ok(selections)
}

fn append_task_start_selection(
    program: &CheckedTrees,
    requirement: psi_symbols::SymbolHandle,
    target_name: &str,
    machine_arguments: &[psi_checked_trees::expression::StaticMachineArgument],
    selections: &mut Vec<TaskStartSelection>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((definition, signature, operation)) = program.traits().iter().find_map(|definition| {
        if !definition.is_boundary
            || definition.name.as_str().rsplit("::").next() != Some("TaskRuntime")
        {
            return None;
        }
        program
            .trait_machine_signatures(definition)
            .iter()
            .find(|signature| signature.symbol == requirement)
            .and_then(|signature| {
                let result = program.display_type_reference(signature.return_type);
                match (signature.name.as_str(), result.as_str()) {
                    ("start", result) if result.starts_with("Task<") => {
                        Some((definition, signature, TaskStartOperation::Start))
                    }
                    ("try_start", result) if result.starts_with("StartOutcome<") => {
                        Some((definition, signature, TaskStartOperation::TryStart))
                    }
                    _ => None,
                }
            })
    }) else {
        return;
    };
    let [target] = machine_arguments else {
        diagnostics.push(Diagnostic::error(format!(
            "TaskRuntime requirement `{target_name}` must select exactly one static target machine, got {}",
            machine_arguments.len()
        )));
        return;
    };
    let Some((target_machine, target_entry)) = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target.symbol)
            .map(|entry| (machine, entry))
    }) else {
        // A generic wrapper may forward its own machine parameter. Its cloned
        // concrete specialization contributes the eventual activation row.
        if target.symbol.is_valid()
            && program
                .machines()
                .iter()
                .flat_map(|machine| program.machine_type_parameters(machine))
                .any(|parameter| parameter.symbol == target.symbol)
        {
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "TaskRuntime requirement `{target_name}` selects an unresolved static target `{}`",
            target
                .path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        )));
        return;
    };
    let mut hash = StableHash::new();
    hash.string("task-runtime-requirement-specialization-v1");
    hash.string(
        program
            .normalized_trait_requirement_overload_identity(definition, signature)
            .identity()
            .as_str(),
    );
    hash.u64(entry_identity(program, target_machine, target_entry));
    let selection = TaskStartSelection {
        requirement,
        target_entry: target_entry.symbol,
        fingerprint: hash.finish(),
        operation,
    };
    if !selections.contains(&selection) {
        selections.push(selection);
    }
}

/// Current fixed-stack layout bridge: persistent machine storage plus the
/// largest canonical park frontier and entry resume overhead. Whole-call-graph
/// WCSU composition will replace this local sizing pass before provider stack
/// reservation is enabled.
fn fixed_stack_layout(
    program: &CheckedTrees,
    target: NativeTarget,
    layouts: &omega_layout::LayoutPlan,
    machine: &psi_checked_trees::machine::Machine,
    crossings: &[&psi_checked_trees::SuspensionCrossingCarryFact],
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

fn carry_obligations(policy: CarryPolicy) -> ActivationCarryObligations {
    ActivationCarryObligations {
        preserve_cpu: policy.cpu == CarryCpu::Origin,
        preserve_host_thread: policy.host_thread == CarryHostThread::Origin,
    }
}

fn canonical_suspension_crossing(
    program: &CheckedTrees,
    crossing: &psi_checked_trees::SuspensionCrossingCarryFact,
) -> Result<CanonicalSuspensionCrossing, Vec<Diagnostic>> {
    let mut hash = StableHash::new();
    hash.byte(0x73);
    hash.string(symbol_identity(program, crossing.machine));
    hash.string(symbol_identity(program, crossing.state));
    hash.usize(crossing.statement_index);
    hash.usize(crossing.call_ordinal);
    hash.string(symbol_identity(program, crossing.target));
    hash.byte(u8::from(
        crossing.effective.suspension == CarrySuspension::Allowed,
    ));
    hash.byte(u8::from(crossing.effective.cpu == CarryCpu::Origin));
    hash.byte(u8::from(
        crossing.effective.host_thread == CarryHostThread::Origin,
    ));
    hash.byte(match crossing.effective.address {
        psi_language_semantics::CarryAddress::Movable => 1,
        psi_language_semantics::CarryAddress::Stable => 2,
    });
    for live in &crossing.live_values {
        hash.string(
            program
                .normalized_type_identity(live.type_reference)
                .as_str(),
        );
        hash.byte(match live.storage {
            SuspensionCrossingStorage::Persistent => 1,
            SuspensionCrossingStorage::Parameter => 2,
            SuspensionCrossingStorage::Local => 3,
            SuspensionCrossingStorage::CallArgument => 4,
        });
        hash.byte(u8::from(
            live.effective.suspension == CarrySuspension::Allowed,
        ));
        hash.byte(u8::from(live.effective.cpu == CarryCpu::Origin));
        hash.byte(u8::from(
            live.effective.host_thread == CarryHostThread::Origin,
        ));
        hash.byte(match live.effective.address {
            psi_language_semantics::CarryAddress::Movable => 1,
            psi_language_semantics::CarryAddress::Stable => 2,
        });
    }
    Ok(CanonicalSuspensionCrossing {
        identity: normalized_id(
            hash.finish(),
            SuspensionCrossingId::from_normalized_identity,
        )?,
        suspension_allowed: crossing.effective.suspension == CarrySuspension::Allowed,
        preserve_cpu: crossing.effective.cpu == CarryCpu::Origin,
        preserve_host_thread: crossing.effective.host_thread == CarryHostThread::Origin,
    })
}

fn symbol_identity(program: &CheckedTrees, symbol: psi_symbols::SymbolHandle) -> &str {
    for machine in program.machines() {
        if machine.symbol == symbol {
            return machine.name.as_str();
        }
        if let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == symbol)
        {
            return state.name.as_str();
        }
    }
    "<unknown>"
}

fn stack_representation_identity(target: NativeTarget) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(0x53);
    hash.string("fixed-nonmoving-stack-v1");
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
    hash.finish()
}

fn signature_layout_identity(
    program: &CheckedTrees,
    target: NativeTarget,
    types: impl IntoIterator<Item = psi_checked_trees::types::TypeReferenceHandle>,
) -> Result<u64, Vec<Diagnostic>> {
    let mut hash = StableHash::new();
    hash.byte(0x51);
    for type_reference in types {
        let layout = omega_layout::layout_type_reference(program, target, type_reference)
            .map_err(|error| vec![error])?;
        hash.string(program.normalized_type_identity(type_reference).as_str());
        hash.usize(layout.size);
        hash.usize(layout.alignment);
        hash.byte(0xff);
    }
    Ok(hash.finish())
}

fn entry_identity(
    program: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    entry: &psi_checked_trees::state::State,
) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(0x45);
    hash.string(machine.name.as_str());
    hash.string(entry.name.as_str());
    for parameter in program.state_parameters(entry) {
        hash.byte(u8::from(parameter.is_self));
        hash.byte(u8::from(parameter.is_mutable));
        hash.string(
            program
                .normalized_type_identity(parameter.type_reference)
                .as_str(),
        );
    }
    hash.byte(0xfe);
    hash.string(program.normalized_type_identity(entry.return_type).as_str());
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
    fn preservation_mapping_keeps_only_cpu_and_thread_obligations() {
        assert_eq!(
            carry_obligations(CarryPolicy {
                suspension: CarrySuspension::Allowed,
                cpu: CarryCpu::Origin,
                host_thread: CarryHostThread::Origin,
                address: psi_language_semantics::CarryAddress::Stable,
            }),
            ActivationCarryObligations {
                preserve_cpu: true,
                preserve_host_thread: true,
            }
        );
    }

    #[test]
    fn activation_crossings_include_contained_machine_crossings() {
        let root = psi_symbols::SymbolHandle::from_arena_index(1);
        let child = psi_symbols::SymbolHandle::from_arena_index(2);
        let field = psi_symbols::SymbolHandle::from_arena_index(3);
        let data = psi_symbols::SymbolHandle::from_arena_index(4);
        let mut program = CheckedTrees::default();
        let targets = program
            .facts
            .carry
            .contained_targets
            .insert_many([psi_checked_trees::ContainedMachineTargetFact { machine: child }]);
        let fields = program.facts.carry.contained_fields.insert_many([
            psi_checked_trees::ContainedMachineFieldFact {
                field,
                data,
                type_reference: psi_checked_trees::types::TypeReferenceHandle::invalid(),
                targets,
            },
        ]);
        program.facts.carry.machine_topologies.insert(
            psi_checked_trees::MachineCarryTopologyFact {
                machine: root,
                fields,
            },
        );
        program.facts.carry.machine_topologies.insert(
            psi_checked_trees::MachineCarryTopologyFact {
                machine: child,
                fields: psi_arena::HandleSpan::empty(),
            },
        );
        let child_policy = CarryPolicy {
            suspension: CarrySuspension::Allowed,
            cpu: CarryCpu::Origin,
            host_thread: CarryHostThread::Any,
            address: psi_language_semantics::CarryAddress::Movable,
        };
        program.facts.carry.suspension_crossings.push(
            psi_checked_trees::SuspensionCrossingCarryFact {
                machine: child,
                state: psi_symbols::SymbolHandle::invalid(),
                statement_index: 0,
                call_ordinal: 0,
                target: psi_symbols::SymbolHandle::invalid(),
                effective: child_policy,
                live_values: Vec::new(),
            },
        );

        let crossings = activation_carry_crossings(&program, root);

        assert!(crossings.root.is_empty());
        assert_eq!(crossings.subtree.len(), 1);
        assert!(
            crossings
                .subtree
                .iter()
                .all(|crossing| crossing.effective.suspension == CarrySuspension::Allowed)
        );
        assert_eq!(crossings.subtree[0].effective, child_policy);
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
            boundary trait TaskRuntime {
                machine start<T, Arguments, machine Target>(
                    &self,
                    arguments: Arguments
                ) -> Task<T>
                where machine Target(arguments: Arguments) -> T suspends; blocks;
                ensures true;
                machine try_start<T, Arguments, machine Target>(
                    &self,
                    arguments: Arguments
                ) -> StartOutcome<T, Arguments>
                where machine Target(arguments: Arguments) -> T suspends; blocks;
                ensures true;
            }

            data LocalTaskRuntime { }
            LocalTaskRuntimeTaskRuntime: LocalTaskRuntime satisfies TaskRuntime;
            machine LocalTaskRuntime::start<T, Arguments, machine Target>(
                &self,
                arguments: Arguments
            ) -> Task<T>
            where machine Target(arguments: Arguments) -> T suspends; blocks;
            satisfies TaskRuntime::start
            via Binding::CompilerIntrinsic("TaskRuntime::start");
            machine LocalTaskRuntime::try_start<T, Arguments, machine Target>(
                &self,
                arguments: Arguments
            ) -> StartOutcome<T, Arguments>
            where machine Target(arguments: Arguments) -> T suspends; blocks;
            satisfies TaskRuntime::try_start
            via Binding::CompilerIntrinsic("TaskRuntime::try_start");

            boundary data Sleeper;
            boundary machine Sleeper::park(token: i32) suspends;
            data Job { value: i32; }
            data Worker {}
            machine Worker::run(job: Job) -> i32 suspends; {
                let value: i32 = job.value;
                suspend Sleeper::park(value);
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
        let tokens = psi_source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type");
        let provider_plans =
            crate::pipeline::provider_plans::derive_satisfies_plans(&syntax, &typed, None);
        assert_eq!(provider_plans.len(), 1);
        assert!(
            crate::pipeline::provider_plans::validate_provider_plan_candidates(
                &typed,
                &provider_plans,
            )
            .is_empty()
        );
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            &provider_plans,
            &[provider_plans[0].name.clone()],
        )
        .expect("select complete TaskRuntime provider");
        let checked = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)
            .expect("check and specialize task start");

        let task_activations =
            elaborate_task_activation_plans(&checked, &selected, NativeTarget::macos_arm64())
                .expect("elaborate activation plan");
        let activations = task_activations.as_slice();
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
        assert_eq!(plan.stack_plan.bytes, 16);
        assert_eq!(plan.stack_plan.alignment, 8);
        assert!(plan.may_suspend);
        assert!(!plan.may_block);
        assert_eq!(plan.canonical_suspension_crossings.len(), 1);
        assert_eq!(plan.carry_obligations, ActivationCarryObligations::none());
        assert!(plan.cancellation_required);
        assert_ne!(plan.machine_contract.normalized_identity(), 0);
        assert_ne!(plan.argument_layout.normalized_identity(), 0);
        assert_ne!(plan.terminal_outcome_layout.normalized_identity(), 0);
        assert_ne!(
            activation.plan.normalized_identity().normalized_identity(),
            0
        );
        assert_eq!(
            activation.selected_runtime.provider_plan_name,
            "LocalTaskRuntime::satisfies::TaskRuntime"
        );
        assert_eq!(
            activation.selected_runtime.runtime.normalized_identity(),
            selected
                .plans()
                .first()
                .expect("selected runtime plan")
                .identity_fingerprint()
        );
        assert!(
            activation
                .selected_runtime
                .requirement_identity
                .contains("TaskRuntime")
        );

        let manifest =
            omega_visualizations::task_activation_manifest_json(&checked, &task_activations);
        assert!(manifest.contains("\"operation\": \"start\""));
        assert!(manifest.contains("\"operation\": \"try_start\""));
        assert!(manifest.contains("\"target_machine\": \"Worker::run\""));
        assert!(manifest.contains("\"selected_runtime\": {"));
        assert!(
            manifest.contains("\"provider_plan\": \"LocalTaskRuntime::satisfies::TaskRuntime\"")
        );
        assert!(manifest.contains("\"stack_plan\": {\"bytes\": 16, \"alignment\": 8"));
        assert!(manifest.contains("\"canonical_suspension_crossings\": ["));
        assert!(manifest.contains(
            "\"cpu_thread_preservation\": {\"preserve_cpu\": false, \"preserve_host_thread\": false}"
        ));
        assert!(manifest.contains("\"cancellation_required\": true"));
        assert!(manifest.contains("\"activation_plan_id\": \"0x"));
        assert!(!manifest.contains("\"runtime_admission\""));
    }
}

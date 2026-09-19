//! Fixtures shared by the task plan tests: activation fixtures, error
//! helpers and the concrete task start fixture.

use crate::ProviderPlanDerivation;
mod activation_crossings_and_task_starts;
mod activation_operational_and_requirements;
mod activation_targets_and_topology;
mod call_target_bindings;
mod stack_graphs;

use crate::task_plans::carry_crossings::{
    activation_carry_crossings, exact_activation_carry_subtree,
};
use crate::task_plans::runtime_requirements::exact_task_runtime_requirement;
use crate::task_plans::start_selections::TaskStartSelection;
use crate::task_plans::{CheckedTrees, Diagnostic};
use language_semantics::{CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use task_plans::TaskStartOperation;

fn activation_operational_fixture() -> (CheckedTrees, symbols::SymbolHandle, symbols::SymbolHandle)
{
    let target = symbols::SymbolHandle::from_arena_index(1);
    let unrelated = symbols::SymbolHandle::from_arena_index(2);
    let mut program = CheckedTrees::default();
    program.facts.contract_plans.machines = vec![
        checked_trees::MachineContractPlan {
            machine: target,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0x1111,
            commitment: checked_trees::MachineContractCommitment::from_digest([1; 32]),
        },
        checked_trees::MachineContractPlan {
            machine: unrelated,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0x2222,
            commitment: checked_trees::MachineContractCommitment::from_digest([2; 32]),
        },
    ];
    program.facts.suspensions.machines = vec![
        checked_trees::MachineSuspensionFact {
            machine: target,
            plan: language_semantics::SuspensionPlan {
                interface: language_semantics::SuspensionInterface::InternalInferred,
                checked_may_suspend: true,
            },
        },
        checked_trees::MachineSuspensionFact {
            machine: unrelated,
            plan: language_semantics::SuspensionPlan {
                interface: language_semantics::SuspensionInterface::PublishedMaySuspend(false),
                checked_may_suspend: false,
            },
        },
    ];
    program.facts.blocking.machines = vec![
        checked_trees::MachineBlockingFact {
            machine: target,
            plan: language_semantics::BlockingPlan {
                interface: language_semantics::BlockingInterface::PublishedMayBlock(false),
                checked_may_block: false,
            },
        },
        checked_trees::MachineBlockingFact {
            machine: unrelated,
            plan: language_semantics::BlockingPlan {
                interface: language_semantics::BlockingInterface::InternalInferred,
                checked_may_block: true,
            },
        },
    ];
    (program, target, unrelated)
}

fn operational_error<T>(result: Result<T, Vec<Diagnostic>>) -> String {
    match result {
        Ok(_) => panic!("invalid exact operational rows must fail closed"),
        Err(diagnostics) => diagnostics
            .first()
            .expect("operational diagnostic")
            .message
            .clone(),
    }
}

struct ActivationRequirementFixture {
    program: CheckedTrees,
    selection: TaskStartSelection,
    other_owner: symbols::SymbolHandle,
    other_requirement: symbols::SymbolHandle,
    private_owner: symbols::SymbolHandle,
    private_requirement: symbols::SymbolHandle,
}

fn activation_requirement_fixture(duplicate_requirement: bool) -> ActivationRequirementFixture {
    let owner = symbols::SymbolHandle::from_arena_index(1);
    let requirement = symbols::SymbolHandle::from_arena_index(2);
    let other_owner = symbols::SymbolHandle::from_arena_index(3);
    let other_requirement = symbols::SymbolHandle::from_arena_index(4);
    let private_owner = symbols::SymbolHandle::from_arena_index(5);
    let private_requirement = symbols::SymbolHandle::from_arena_index(6);
    let mut program = CheckedTrees::default();

    let mut task_runtime = checked_trees::trait_definition::TraitDefinition {
        symbol: owner,
        is_boundary: true,
        name: checked_trees::name::Identifier::generated("core::TaskRuntime"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut task_runtime,
        checked_trees::signature::StateSignature {
            symbol: requirement,
            name: checked_trees::name::Identifier::generated("start"),
            ..Default::default()
        },
    );
    if duplicate_requirement {
        program.typed.push_trait_machine_signature(
            &mut task_runtime,
            checked_trees::signature::StateSignature {
                symbol: requirement,
                name: checked_trees::name::Identifier::generated("duplicate"),
                ..Default::default()
            },
        );
    }
    program.typed.push_trait_definition(task_runtime);

    let mut other = checked_trees::trait_definition::TraitDefinition {
        symbol: other_owner,
        is_boundary: true,
        name: checked_trees::name::Identifier::generated("other::TaskRuntime"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut other,
        checked_trees::signature::StateSignature {
            symbol: other_requirement,
            name: checked_trees::name::Identifier::generated("start"),
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(other);

    let mut private = checked_trees::trait_definition::TraitDefinition {
        symbol: private_owner,
        is_boundary: false,
        name: checked_trees::name::Identifier::generated("PrivateRuntime"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut private,
        checked_trees::signature::StateSignature {
            symbol: private_requirement,
            name: checked_trees::name::Identifier::generated("start"),
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(private);

    ActivationRequirementFixture {
        program,
        selection: TaskStartSelection {
            requirement_owner: owner,
            requirement,
            target_machine: symbols::SymbolHandle::from_arena_index(7),
            target_entry: symbols::SymbolHandle::from_arena_index(8),
            report_fingerprint: 1,
            operation: TaskStartOperation::Start,
        },
        other_owner,
        other_requirement,
        private_owner,
        private_requirement,
    }
}

fn requirement_error(program: &CheckedTrees, selection: &TaskStartSelection) -> String {
    match exact_task_runtime_requirement(program, selection) {
        Ok(_) => panic!("invalid exact requirement custody must fail closed"),
        Err(diagnostics) => diagnostics
            .first()
            .expect("requirement diagnostic")
            .message
            .clone(),
    }
}

fn activation_target_fixture() -> (
    CheckedTrees,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
) {
    let first_machine = symbols::SymbolHandle::from_arena_index(1);
    let first_entry = symbols::SymbolHandle::from_arena_index(2);
    let second_machine = symbols::SymbolHandle::from_arena_index(3);
    let second_entry = symbols::SymbolHandle::from_arena_index(4);
    let mut program = CheckedTrees::default();

    let mut first = checked_trees::machine::Machine {
        symbol: first_machine,
        name: checked_trees::name::Identifier::generated("First::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut first,
        checked_trees::state::State {
            symbol: first_entry,
            name: checked_trees::name::Identifier::generated("run"),
            ..Default::default()
        },
    );
    program.typed.push_machine(first);

    let mut second = checked_trees::machine::Machine {
        symbol: second_machine,
        name: checked_trees::name::Identifier::generated("Second::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut second,
        checked_trees::state::State {
            symbol: second_entry,
            name: checked_trees::name::Identifier::generated("run"),
            ..Default::default()
        },
    );
    program.typed.push_machine(second);

    (
        program,
        first_machine,
        first_entry,
        second_machine,
        second_entry,
    )
}

fn activation_crossing_validation_fixture() -> (
    CheckedTrees,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
) {
    let root = symbols::SymbolHandle::from_arena_index(1);
    let root_state = symbols::SymbolHandle::from_arena_index(2);
    let child = symbols::SymbolHandle::from_arena_index(3);
    let child_state = symbols::SymbolHandle::from_arena_index(4);
    let field = symbols::SymbolHandle::from_arena_index(5);
    let data = symbols::SymbolHandle::from_arena_index(6);
    let mut program = CheckedTrees::default();
    let contained_type =
        program
            .typed
            .type_reference_table
            .insert(checked_trees::types::TypeReferenceNode::Named {
                symbol: data,
                name: checked_trees::name::Identifier::generated("ChildData"),
            });

    let mut root_machine = checked_trees::machine::Machine {
        symbol: root,
        name: checked_trees::name::Identifier::generated("Root::run"),
        ..Default::default()
    };
    let mut root_state_definition = checked_trees::state::State {
        symbol: root_state,
        name: checked_trees::name::Identifier::generated("run"),
        ..Default::default()
    };
    program.typed.statement_table.push_statement(
        &mut root_state_definition.statement_nodes,
        Default::default(),
    );
    program
        .typed
        .push_machine_state(&mut root_machine, root_state_definition);
    program.typed.push_machine(root_machine);

    let mut child_machine = checked_trees::machine::Machine {
        symbol: child,
        name: checked_trees::name::Identifier::generated("Child::run"),
        ..Default::default()
    };
    let mut child_state_definition = checked_trees::state::State {
        symbol: child_state,
        name: checked_trees::name::Identifier::generated("run"),
        ..Default::default()
    };
    program.typed.statement_table.push_statement(
        &mut child_state_definition.statement_nodes,
        Default::default(),
    );
    program
        .typed
        .push_machine_state(&mut child_machine, child_state_definition);
    program.typed.push_machine(child_machine);

    let mut calls = arena::HandleSpan::empty();
    program.facts.flow.control.calls.append_to_span(
        &mut calls,
        checked_trees::FlowCallFact {
            statement_index: 0,
            call_ordinal: 0,
            target_symbol: root_state,
            suspension: language_semantics::SuspensionSummary {
                direct_may_suspend: true,
                transitive_may_suspend: false,
            },
            ..Default::default()
        },
    );
    program.facts.flow.control.calls.append_to_span(
        &mut calls,
        checked_trees::FlowCallFact {
            statement_index: 0,
            call_ordinal: 1,
            target_symbol: child_state,
            suspension: language_semantics::SuspensionSummary {
                direct_may_suspend: false,
                transitive_may_suspend: true,
            },
            ..Default::default()
        },
    );
    program
        .facts
        .flow
        .control
        .states
        .append(checked_trees::FlowStateFact {
            machine_symbol: child,
            state_symbol: child_state,
            calls,
            ..Default::default()
        });

    let targets = program
        .facts
        .carry
        .contained_targets
        .insert_many([checked_trees::ContainedMachineTargetFact { machine: child }]);
    let fields = program.facts.carry.contained_fields.insert_many([
        checked_trees::ContainedMachineFieldFact {
            field,
            data,
            type_reference: contained_type,
            targets,
        },
    ]);
    program
        .facts
        .carry
        .machine_topologies
        .insert(checked_trees::MachineCarryTopologyFact {
            machine: root,
            fields,
        });
    program
        .facts
        .carry
        .machine_topologies
        .insert(checked_trees::MachineCarryTopologyFact {
            machine: child,
            fields: arena::HandleSpan::empty(),
        });
    program
        .facts
        .carry
        .suspension_crossings
        .push(checked_trees::SuspensionCrossingCarryFact {
            machine: child,
            state: child_state,
            statement_index: 0,
            call_ordinal: 0,
            target: root_state,
            receiver: None,
            effective: CarryPolicy {
                suspension: CarrySuspension::Allowed,
                cpu: CarryCpu::Origin,
                host_thread: CarryHostThread::Any,
                address: language_semantics::CarryAddress::Movable,
            },
            live_values: Vec::new(),
        });
    program
        .facts
        .carry
        .suspension_crossings
        .push(checked_trees::SuspensionCrossingCarryFact {
            machine: child,
            state: child_state,
            statement_index: 0,
            call_ordinal: 1,
            target: child_state,
            receiver: None,
            effective: CarryPolicy::PERMISSIVE,
            live_values: Vec::new(),
        });
    (program, root, root_state, child_state)
}

fn crossing_error(program: &CheckedTrees, root: symbols::SymbolHandle) -> String {
    match activation_carry_crossings(program, root) {
        Ok(_) => panic!("invalid crossing must fail closed"),
        Err(diagnostics) => diagnostics
            .first()
            .expect("crossing diagnostic")
            .message
            .clone(),
    }
}

fn topology_error(program: &CheckedTrees, root: symbols::SymbolHandle) -> String {
    match exact_activation_carry_subtree(program, root) {
        Ok(_) => panic!("invalid topology must fail closed"),
        Err(diagnostics) => diagnostics
            .first()
            .expect("topology diagnostic")
            .message
            .clone(),
    }
}

fn concrete_task_start_fixture() -> (
    CheckedTrees,
    effects::SelectedProviderPlanFacts,
    Vec<effects::provider_plan::ProviderPlan>,
) {
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
        via Binding::CompilerIntrinsic;
        machine LocalTaskRuntime::try_start<T, Arguments, machine Target>(
            &self,
            arguments: Arguments
        ) -> StartOutcome<T, Arguments>
        where machine Target(arguments: Arguments) -> T suspends; blocks;
        satisfies TaskRuntime::try_start
        via Binding::CompilerIntrinsic;

        pub boundary data Sleeper;
        boundary machine Sleeper::park(token: i32) suspends;
        data Job { value: i32; }
        data Worker {}
        machine Worker::run(job: Job) -> i32 suspends; {
            let value: i32 = job.value;
            suspend Sleeper::park(value);
            value
        }
        data Main { runtime: &TaskRuntime; }
        machine Main::run(&mut self) reaches TaskRuntime {
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
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let provider_plans = crate::provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    assert_eq!(provider_plans.len(), 1);
    assert!(
        crate::provider_planning::validate_provider_plan_candidates(&typed, &provider_plans,)
            .is_empty()
    );
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        &provider_plans,
        &[provider_plans[0].name.clone()],
    )
    .expect("select complete TaskRuntime provider");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check and specialize task start");

    (checked, selected, provider_plans)
}

/// A task target whose checked body calls another checked machine that parks
/// inside the nested frame. The activation's whole-call-graph demand must
/// cover both frames, and its canonical roster must cover the callee crossing.
fn nested_task_call_fixture() -> (
    CheckedTrees,
    effects::SelectedProviderPlanFacts,
    Vec<effects::provider_plan::ProviderPlan>,
) {
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
        via Binding::CompilerIntrinsic;
        machine LocalTaskRuntime::try_start<T, Arguments, machine Target>(
            &self,
            arguments: Arguments
        ) -> StartOutcome<T, Arguments>
        where machine Target(arguments: Arguments) -> T suspends; blocks;
        satisfies TaskRuntime::try_start
        via Binding::CompilerIntrinsic;

        pub boundary data Sleeper;
        boundary machine Sleeper::park(token: i32) suspends;
        data Job { value: i32; }
        data Worker {}
        data Helper {}
        machine Helper::work(token: i32) -> i32 suspends; {
            suspend Sleeper::park(token);
            token
        }
        machine Worker::run(job: Job) -> i32 suspends; {
            let value: i32 = job.value;
            let helped: i32 = suspend Helper::work(value);
            suspend Sleeper::park(helped);
            helped
        }
        data Main { runtime: &TaskRuntime; }
        machine Main::run(&mut self) reaches TaskRuntime {
            let job: Job = Job { value: 7 };
            let task: Task<i32> = self.runtime.start<Worker::run>(job);
            Task::settle(task);
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let provider_plans = crate::provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    assert_eq!(provider_plans.len(), 1);
    assert!(
        crate::provider_planning::validate_provider_plan_candidates(&typed, &provider_plans,)
            .is_empty()
    );
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        &provider_plans,
        &[provider_plans[0].name.clone()],
    )
    .expect("select complete TaskRuntime provider");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check and specialize nested task call");

    (checked, selected, provider_plans)
}

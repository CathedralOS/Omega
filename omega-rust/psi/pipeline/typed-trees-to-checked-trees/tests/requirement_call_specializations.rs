//! A generic requirement call keeps the specialization MP2b derived at its
//! static machine arguments as a checked fact: the `Type` substitutions the
//! selected callable's shape proved plus the exact provider entry each
//! `machine` binder admitted. Structural contracts emit no nominal
//! satisfaction row, so this fact is the only checked record of what such a
//! call instantiated.

use typed_trees::data::TypeParameterKind;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::TypeReferenceNode;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )?;
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
    typed_trees_to_checked_trees::lower_typed_trees(typed)
}

// The smallest program that calls the generic TaskRuntime::start boundary
// requirement with a concrete static machine through a runtime capability
// carried on `self`: `start<T, Arguments, machine Target>` binds `T` and
// `Arguments` from the selected `Worker::run` callable's shape and `Target`
// to its entry.
const ROUTED_TASK_START_DECLS: &str = r#"
    data Task<T> [linear] {
        provider: u64;
        activation: u64;
    }

    boundary trait TaskRuntime {
        machine start<T, Arguments, machine Target>(
            &self,
            arguments: Arguments
        ) -> Task<T>
        where machine Target(arguments: Arguments) -> T suspends; blocks;
        ensures true;
    }

    data CanaryTaskRuntime { }

    machine CanaryTaskRuntime::start<T, Arguments, machine Target>(
        &self,
        arguments: Arguments
    ) -> Task<T>
    where machine Target(arguments: Arguments) -> T suspends; blocks;
    satisfies TaskRuntime::start
    via Binding::CompilerIntrinsic;

    data Token {
        id: u64;
    }

    data Worker { }
    machine Worker::run(token: Token) -> Token suspends; {
        token
    }

    machine Task::settle<T>(self) { }
"#;

#[test]
fn a_routed_requirement_call_retains_its_derived_specialization() {
    let checked = check(&format!(
        "{ROUTED_TASK_START_DECLS}
         data Main {{
             runtime: TaskRuntime;
         }}
         machine Main::probe(&mut self, token: Token) reaches TaskRuntime {{
             let task: Task<Token> = self.runtime.start<Worker::run>(token);
             Task::settle(task);
         }}
         machine Main::main(&mut self) {{ }}"
    ))
    .expect("the routed start call checks");
    let start_requirement = checked
        .traits()
        .iter()
        .flat_map(|definition| checked.trait_machine_signatures(definition))
        .find(|signature| signature.name.as_str() == "start")
        .expect("TaskRuntime::start requirement is retained");
    let token = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Token")
        .expect("Token is declared")
        .symbol;
    let worker_machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Worker::run")
        .expect("Worker::run is declared");
    let worker = worker_machine.symbol;
    let worker_entry = checked
        .machine_states(worker_machine)
        .first()
        .expect("Worker::run entry is retained")
        .symbol;
    let (call_handle, _) = checked
        .expression_table
        .iter_expressions()
        .find(|(_, expression)| {
            matches!(expression, ExpressionNode::Call(call)
                if call.target_symbol == start_requirement.symbol)
        })
        .expect("the start call is retained in the checked expression table");

    let [specialization] = checked
        .facts
        .requirement_call_specializations
        .specializations
        .as_slice()
    else {
        panic!(
            "exactly one requirement-call specialization is retained; found {}",
            checked
                .facts
                .requirement_call_specializations
                .specializations
                .len()
        );
    };
    assert_eq!(
        specialization.site,
        checked_trees::NominalMachineUseSite::Expression(call_handle)
    );
    assert_eq!(
        specialization.registration_operation,
        start_requirement.symbol
    );

    // `Target` is start's only `machine` parameter: ordinal 0 binds the
    // `Worker` machine's `run` entry.
    let type_parameters = checked.state_signature_type_parameters(start_requirement);
    let target_parameter = type_parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == "Target")
        .expect("start declares a Target machine parameter");
    assert!(matches!(
        target_parameter.kind,
        TypeParameterKind::Machine { .. }
    ));
    let [selection] = specialization.machine_selections.as_slice() else {
        panic!("Target's selection is the only admitted machine binder");
    };
    assert_eq!(selection.static_machine_ordinal, 0);
    assert_eq!(selection.parameter, target_parameter.symbol);
    assert_eq!(selection.selected_machine, worker);
    assert_eq!(selection.selected, worker_entry);

    // `Arguments` binds to Token through `start`'s `arguments` parameter and
    // `T` through its `Task<T>` return, both judged against `Worker::run`'s
    // callable shape `(token: Token) -> Token`.
    for name in ["T", "Arguments"] {
        let parameter = type_parameters
            .iter()
            .find(|parameter| parameter.name.as_str() == name)
            .unwrap_or_else(|| panic!("start declares generic parameter {name}"));
        assert!(matches!(parameter.kind, TypeParameterKind::Type));
        let binding = specialization
            .type_bindings
            .iter()
            .find(|binding| binding.parameter == parameter.symbol)
            .unwrap_or_else(|| panic!("{name} has a retained substitution"));
        let TypeReferenceNode::Named { symbol, .. } =
            checked.type_reference_table.type_reference(binding.actual)
        else {
            panic!("{name} binds a named actual type");
        };
        assert_eq!(*symbol, token, "{name} binds Token");
    }
}

#[test]
fn a_call_without_static_machine_arguments_retains_no_specialization() {
    let checked = check(&format!(
        "{ROUTED_TASK_START_DECLS}
         data Main {{ }}
         machine Main::run(token: Token) -> Token {{ Worker::run(token) }}
         machine Main::main(&mut self) {{ }}"
    ))
    .expect("an ordinary concrete call checks");
    assert!(
        checked
            .facts
            .requirement_call_specializations
            .specializations
            .is_empty(),
        "no call named a static machine argument, so no specialization is retained"
    );
}

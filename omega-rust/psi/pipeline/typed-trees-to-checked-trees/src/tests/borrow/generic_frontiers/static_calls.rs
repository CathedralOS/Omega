use super::typed_program;
use typed_trees::expression::ExpressionNode;

#[test]
fn stale_frontier_nodes_never_supply_an_empty_result_proof() {
    use crate::borrow::view_link::{
        DeclarationLifetimeFrontier, declaration_lifetime_frontier, substituted_result_is_view_free,
    };
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    let mut program = typed_program(
        r#"
        data Envelope<T> { value: T; }
        trait Produces<T> { machine result(value: T) -> Envelope<T>; }
    "#,
    );
    let definition = &program.traits()[0];
    let parameter = program.trait_type_parameters(definition)[0].symbol;
    let result = program.trait_machine_signatures(definition)[0].return_type;
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let stale = arena::Handle::from_parts(unit.arena_index(), unit.generation() + 1);
    assert!(substituted_result_is_view_free(&program, unit, &[]));
    assert!(substituted_result_is_view_free(
        &program,
        Default::default(),
        &[]
    ));
    assert!(!substituted_result_is_view_free(&program, stale, &[]));
    assert_eq!(
        declaration_lifetime_frontier(&program, stale, &[]),
        DeclarationLifetimeFrontier::Incomplete
    );
    assert!(substituted_result_is_view_free(
        &program,
        result,
        &[(parameter, unit)]
    ));
    assert!(!substituted_result_is_view_free(
        &program,
        result,
        &[(parameter, stale)]
    ));
    assert!(
        !substituted_result_is_view_free(&program, unit, &[(parameter, stale)]),
        "even a phantom substitution must retain a live type handle"
    );
    let nested = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: stale,
            length: FixedArrayLength::Literal(1),
        });
    assert!(!substituted_result_is_view_free(
        &program,
        result,
        &[(parameter, nested)]
    ));
}

fn source(view: bool) -> String {
    let field = if view { "&i32" } else { "i32" };
    format!(
        r#"
        data Ticket<T> {{ identifier: u64; }}
        data Outcome<T, Arguments> {{
            case Started(ticket: Ticket<T>);
            case Rejected(arguments: Arguments);
        }}
        boundary trait Requests {{
            machine submit<T, Arguments, machine Target>(&self, arguments: Arguments) -> Outcome<T, Arguments>
            where machine Target(arguments: Arguments) -> T;
            ensures true;
        }}
        data Job {{ value: {field}; }}
        machine work(arguments: Job) -> i32 {{ 0 }}
        data Client {{ requests: &Requests; }}
        trait Other<Arguments> {{}}
        machine Client::run(&mut self, job: Job) {{
            let outcome: Outcome<i32, Job> = self.requests.submit<work>(job);
        }}
    "#
    )
}

#[test]
fn exact_static_callable_substitution_allows_only_closed_view_free_results() {
    crate::lower_typed_trees(typed_program(&source(false)))
        .unwrap_or_else(|diagnostics| panic!("closed arbitrary service result: {diagnostics:#?}"));
    let direct = source(false).replace("submit<work>(job)", "submit<work>(Job { value: 9 })");
    crate::lower_typed_trees(typed_program(&direct))
        .unwrap_or_else(|diagnostics| panic!("exact direct constructor: {diagnostics:#?}"));
    let diagnostics = crate::lower_typed_trees(typed_program(&source(true)))
        .expect_err("a rejected job would carry a view requiring caller-specific loans");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("template-dependent returned-carrier lifetime frontier")),
        "{diagnostics:#?}"
    );
}

#[test]
fn closed_bindings_reject_missing_selection_conflicting_arguments_and_borrow_erasure() {
    let mut program = typed_program(&source(false));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Client::run")
        .unwrap()
        .clone();
    let state = program.machine_states(&machine)[0].clone();
    let call = program
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == "submit" => Some(call.clone()),
            _ => None,
        })
        .unwrap();
    let signature = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .find(|signature| signature.symbol == call.target_symbol)
        .unwrap()
        .clone();
    let arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .to_vec();
    let project = |program: &typed_trees::TypedTrees, selected: &[_], arguments: &[_]| {
        validation::closed_static_call_type_bindings(
            program, &machine, &state, &signature, selected, arguments,
        )
    };
    assert!(project(&program, &call.machine_arguments, &arguments).is_some());
    assert!(project(&program, &[], &arguments).is_none());
    assert!(project(&program, &call.machine_arguments, &[]).is_none());
    let mut missing = call.machine_arguments.to_vec();
    missing[0].symbol = Default::default();
    assert!(project(&program, &missing, &arguments).is_none());
    let scalar = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(7),
    ));
    assert!(project(&program, &call.machine_arguments, &[scalar]).is_none());
    let borrowed = program.expression_table.insert(ExpressionNode::Borrow(
        typed_trees::expression::TableBorrowExpression {
            target: arguments[0],
            access: language_semantics::ReferenceAccess::Shared,
        },
    ));
    assert!(project(&program, &call.machine_arguments, &[borrowed]).is_none());
    let foreign = program
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Other")
        .unwrap();
    let foreign = program.trait_type_parameters(foreign)[0].symbol;
    let requirement = program
        .state_signature_type_parameters(&signature)
        .iter()
        .find_map(|parameter| {
            let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
                return None;
            };
            Some(
                program
                    .machine_parameter_contract_view(contract)
                    .unwrap()
                    .signature()
                    .clone(),
            )
        })
        .unwrap();
    let reference = program.state_signature_parameters(&requirement)[0].type_reference;
    program.type_reference_table.substitute_node(
        reference,
        typed_trees::types::TypeReferenceNode::Named {
            symbol: foreign,
            name: "Arguments".into(),
        },
    );
    assert!(
        project(&program, &call.machine_arguments, &arguments).is_none(),
        "a same-spelled binder in another declaration cannot supply this substitution"
    );
}

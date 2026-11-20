#[path = "support/known_gap.rs"]
mod known_gap;
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, CallArgument, CallRequirement, CallSite, LoweredPackage, LoweringError, OperationKind, Parameter, PredicateOperator, PredicateStep,
};

fn lower(source: &str) -> Result<LoweredPackage, LoweringError> {
    sources::lower(&[("contracts.omg", source)])
}

#[test]
fn scalar_contracts_name_the_formals_and_result_and_preserve_each_call_requirement() {
    use PredicateOperator::{Equal, GreaterThan};
    use PredicateStep::{Apply, Formal, Integer, Result};
    let package = lower(
        "machine checked(first:u8, second:u8, flag:bool)->u8
             requires first > 0
             requires flag
             ensures result == second
             ensures flag {second}
         machine ordinary(a:u8, b:u8, flag:bool)->u8{checked(b, a, flag)}
         machine terminal(a:u8, b:u8, flag:bool)->u8{transition flag {true -> checked(b, a, flag), false -> b,}}",
    )
    .unwrap();
    let [checked, ordinary, terminal] = package.machines.as_slice() else {
        panic!("expected three machines");
    };
    assert_eq!(checked.declaration.path, "checked");
    assert_eq!(checked.contract.requires[0].predicate, [Formal(0), Integer(0.into()), Apply(GreaterThan)]);
    assert_eq!(checked.contract.requires[1].predicate, [Formal(2)]);
    assert_eq!(checked.contract.ensures[0].predicate, [Result, Formal(1), Apply(Equal)]);
    assert_eq!(checked.contract.ensures[1].predicate, [Formal(2)]);
    assert_eq!(checked.scalar_result.unwrap().owner, checked.id);

    let expected_arguments = |parameters: &[Parameter]| {
        let [
            Parameter::Scalar(a),
            Parameter::Scalar(b),
            Parameter::Scalar(flag),
        ] = parameters
        else {
            panic!("expected three scalar formals");
        };
        [
            CallArgument::Scalar(*b),
            CallArgument::Scalar(*a),
            CallArgument::Scalar(*flag),
        ]
    };
    let questions = |site| {
        [
            CallRequirement { site, clause: 0 },
            CallRequirement { site, clause: 1 },
        ]
    };
    let [operation] = ordinary.body.operations.as_slice() else {
        panic!("expected one ordinary call");
    };
    let OperationKind::Call {
        callee: omega_lowering::CallTarget::Machine(callee),
        arguments,
        requirements,
    } = &operation.kind
    else {
        panic!("expected an ordinary call operation");
    };
    assert_eq!(*callee, checked.id);
    assert_eq!(*arguments, expected_arguments(&ordinary.body.parameters));
    assert_eq!(*requirements, questions(CallSite::Operation(operation.id)));

    let (id, callee, arguments, requirements) = terminal
        .body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            BlockTerminator::TerminalCall {
                id,
                callee: omega_lowering::CallTarget::Machine(callee),
                arguments,
                requirements,
            ..
            } => Some((*id, *callee, arguments, requirements)),
            _ => None,
        })
        .expect("expected a terminal call edge");
    assert_eq!(callee, checked.id);
    assert_eq!(*arguments, expected_arguments(&terminal.body.parameters));
    assert_eq!(*requirements, questions(CallSite::Edge(id)));
    assert!(terminal.body.operations.is_empty());
}

#[test]
fn unsupported_predicate_call_is_refused_at_its_occurrence() {
    let source = "machine predicate()->bool reaches none crashes none terminates {true}
                  machine guarded() requires predicate() {}";
    let error = lower(source).unwrap_err();
    assert_eq!(error.site.span.slice(source.as_bytes()), Some(b"predicate()".as_slice()));
    assert_eq!(error.message, "unsupported typed contract predicate record");
    known_gap::report("call in a contract predicate", "The clause needs the predicate and its selected call", source, &error);
}

#[test]
fn signed_contract_bounds_preserve_negative_exact_values() {
    use PredicateOperator::GreaterOrEqual;
    use PredicateStep::{Apply, Formal, Integer, Result};
    let package = lower(
        "public machine bounded(value:i8)->i8
             requires value >= (-128)
             ensures result >= -1 {0i8}",
    )
    .unwrap();
    let contract = &package.machines[0].contract;
    assert_eq!(contract.requires[0].predicate, [Formal(0), Integer((-128).into()), Apply(GreaterOrEqual)]);
    assert_eq!(contract.ensures[0].predicate, [Result, Integer((-1).into()), Apply(GreaterOrEqual)]);
}

#[test]
fn folded_exact_operator_supplies_one_landed_constant_to_the_predicate() {
    use PredicateOperator::GreaterThan;
    use PredicateStep::{Apply, Formal, Integer};
    // The grouping carries the landing, so the folded subtraction inherits it.
    // Without the grouping, the subtraction carries its own landing.
    for source in [
        "machine f(x:i8)->bool requires x > (1 - 2) {true}",
        "machine f(x:i8)->bool requires x > 1 - 2 {true}",
    ] {
        let package = lower(source).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        let contract = &package.machines[0].contract;
        assert_eq!(
            contract.requires[0].predicate,
            [Formal(0), Integer((-1).into()), Apply(GreaterThan)],
            "{source}"
        );
    }
}

#[test]
fn contract_ordering_keeps_same_carrier_subjects_and_strictness() {
    use PredicateOperator::{LessOrEqual, LessThan};
    use PredicateStep::{Apply, Formal, Result};
    let package = lower(
        "machine forward(first:u8, second:u8)->u8
             requires first < second
             ensures (result < second) {first}
         machine swapped(first:u8, second:u8)->u8
             requires second < first
             ensures (result < first) {second}
         machine inclusive(first:u8, second:u8)->u8
             requires first <= second
             ensures (result <= second) {first}",
    )
    .unwrap();
    let machine = |path| {
        package
            .machines
            .iter()
            .find(|machine| machine.declaration.path == path)
            .unwrap()
    };
    let forward = machine("forward");
    let swapped = machine("swapped");
    let inclusive = machine("inclusive");

    assert_eq!(forward.contract.requires[0].predicate, [Formal(0), Formal(1), Apply(LessThan)]);
    assert_eq!(swapped.contract.requires[0].predicate, [Formal(1), Formal(0), Apply(LessThan)]);
    assert_eq!(inclusive.contract.requires[0].predicate, [Formal(0), Formal(1), Apply(LessOrEqual)]);
    assert_eq!(forward.contract.ensures[0].predicate, [Result, Formal(1), Apply(LessThan)]);
    assert_eq!(swapped.contract.ensures[0].predicate, [Result, Formal(0), Apply(LessThan)]);
    assert_eq!(inclusive.contract.ensures[0].predicate, [Result, Formal(1), Apply(LessOrEqual)]);
}

#[test]
fn equality_and_inequality_keep_their_distinct_predicates() {
    use PredicateOperator::{Equal, NotEqual};
    use PredicateStep::{Apply, Formal, Result};

    for (spelling, operator) in [("==", Equal), ("!=", NotEqual)] {
        let source = format!("machine compare(a:u8,b:u8)->bool requires a {spelling} b ensures result {spelling} false {{true}}");
        let package = lower(&source).unwrap();
        let contract = &package.machines[0].contract;
        assert_eq!(contract.requires[0].predicate, [Formal(0), Formal(1), Apply(operator)]);
        assert_eq!(contract.ensures[0].predicate, [Result, PredicateStep::Boolean(false), Apply(operator)]);
    }
}

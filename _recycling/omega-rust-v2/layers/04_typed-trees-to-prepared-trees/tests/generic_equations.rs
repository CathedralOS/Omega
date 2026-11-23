use omega_preparation::{PreparationAnswers, PreparationFailure, prepare_source_package_diagnosed, prepare_typing_pass};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{
    SpecializedIndexValue, StaticValue, StaticValueDemand, StaticValueRequest, TypingEnvironment, TypingProgress, ValueType,
    resolve_source_package, type_source_package, type_source_package_for_preparation,
};

fn package(source: &str) -> omega_typer::SourcePackage<'_> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let syntax = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &syntax,
        },
    }])
    .unwrap()
}

const RECORD_EQUATION: &str = "
    data Configuration [copy] { minimum: u64; }
    const SEVEN: Configuration = Configuration { minimum: 7 };
    const SAME: Configuration = Configuration { minimum: 7 };
    const EIGHT: Configuration = Configuration { minimum: 8 };
    data Token<const Setting: Configuration> [copy] { marker: u8; }
    machine read<Item [copy], const Setting: Configuration>(item: Item) -> u64
    requires Item == Token::<Setting>
    { Setting.minimum }
";

#[test]
fn record_equations_compare_canonical_values_when_typing_resumes() {
    for (arguments, accepted) in [
        ("::<Token<SEVEN>, SEVEN>", true),
        ("::<Token<SEVEN>, SAME>", true),
        ("", true),
        ("::<Token<SEVEN>, EIGHT>", false),
    ] {
        let source = format!("{RECORD_EQUATION} machine f(item: Token<SEVEN>) -> u64 {{ read{arguments}(item) }}");
        let package = package(&source);
        let refusal = type_source_package(&package).unwrap_err();
        assert_eq!(refusal.message, "static expression requires an admitted evaluation result");
        match prepare_source_package_diagnosed(&package) {
            Ok(prepared) if accepted => {
                let call = prepared.typed().calls.iter().find(|call| call.machine.path == "read").unwrap();
                let [ValueType::Applied(_, indices), selected] = call.type_arguments.as_slice() else {
                    panic!("expected a type argument and a record constant");
                };
                assert_eq!(indices.as_slice(), std::slice::from_ref(selected));
                assert!(matches!(selected, ValueType::StaticValue { .. }));
            }
            Err(PreparationFailure::ResumedTyping(error)) if !accepted => {
                assert_eq!(error.message, "conflicting type equation for read");
                let selection = format!("read{arguments}");
                assert_eq!(error.span.slice(source.as_bytes()), Some(selection.as_bytes()));
            }
            other => panic!("unexpected preparation outcome for {arguments}: {other:?}"),
        }
    }
}

#[test]
fn ordinary_typing_checks_closed_equations_but_preserves_open_preconditions() {
    let definitions = "
        data Flag<const Enabled: bool> [copy] {}
        machine use_flag<T [copy], const Enabled: bool>(item: T)
        requires T == Flag::<Enabled> {}
    ";
    let source = format!("{definitions} machine f(item: Flag<true>) {{ use_flag::<Flag<true>, false>(item); }}");
    let error = type_source_package(&package(&source)).unwrap_err();
    assert_eq!(error.message, "conflicting type equation for use_flag");

    let source = format!(
        "{definitions}
         machine pass<T [copy], const Enabled: bool>(item: T)
         requires T == Flag::<Enabled> {{ use_flag::<T, Enabled>(item); }}
         machine infer<const Enabled: bool>(item: Flag<Enabled>) {{ use_flag(item); }}"
    );
    let product = type_source_package(&package(&source)).unwrap();
    let calls: Vec<_> = product.calls.iter().filter(|call| call.machine.path == "use_flag").collect();
    assert_eq!(calls.len(), 2);
    for call in calls {
        assert!(matches!(call.type_arguments[1], ValueType::Parameter { .. }));
        assert!(product.obligations.iter().any(|obligation| {
            obligation.node == call.node
                && matches!(&obligation.kind, omega_typer::ObligationKind::CallPrecondition(machine) if machine == &call.machine)
        }));
    }
}

#[test]
fn alternative_and_dependent_equations_remain_preconditions() {
    let source = "
        data Slot<N:u64> [copy] {}
        machine take<T [copy], N:u64>(item:T) requires T == Slot::<N> {}
        machine pass(first:u64, second:u64, item:Slot<first>)
        requires first == second
        { take::<Slot<first>, second>(item); }
        machine choose<T [copy]>(item: T) requires (T == u8 || T == u16) {}
        machine select() { choose(4u16); }
    ";
    let product = type_source_package(&package(source)).unwrap();
    let call = product.calls.iter().find(|call| call.machine.path == "take").unwrap();
    let [ValueType::Applied(_, indices), ValueType::Subject { declaration: second, .. }] = call.type_arguments.as_slice() else {
        panic!("expected both runtime subjects");
    };
    let [ValueType::Subject { declaration: first, .. }] = indices.as_slice() else {
        panic!("expected the subject fixed in Slot");
    };
    assert_ne!(first, second);
    for name in ["take", "choose"] {
        let call = product.calls.iter().find(|call| call.machine.path == name).unwrap();
        assert!(product.obligations.iter().any(|obligation| {
            obligation.node == call.node
                && matches!(&obligation.kind, omega_typer::ObligationKind::CallPrecondition(machine) if machine == &call.machine)
        }));
    }
}

#[test]
fn an_equation_can_infer_either_bare_type_binder_without_collecting_value_equality() {
    let source = "
        machine infer_left<A [copy], B [copy]>(item:B) requires A == B {}
        machine infer_right<A [copy], B [copy]>(item:A) requires A == B {}
        machine require_value<const N:u64>(input:u64) requires N == input {}
        machine f() { infer_left(1u8); infer_right(1u16); require_value::<1>(1u64); }
    ";
    let product = type_source_package(&package(source)).unwrap();
    for name in ["infer_left", "infer_right"] {
        let call = product.calls.iter().find(|call| call.machine.path == name).unwrap();
        assert_eq!(call.type_arguments[0], call.type_arguments[1]);
    }
    let call = product.calls.iter().find(|call| call.machine.path == "require_value").unwrap();
    assert!(product.generic_parameters[&call.machine].equations.is_empty());
}

#[test]
fn an_open_argument_cannot_equate_two_known_nominal_heads() {
    let source = "
        data Flag<const Enabled:bool> [copy] {}
        data Other<const Enabled:bool> [copy] {}
        machine use_flag<T [copy], const Enabled:bool>(item:T) requires T == Flag::<Enabled> {}
        machine f<const Enabled:bool>(item:Other<Enabled>) { use_flag::<Other<Enabled>, Enabled>(item); }
    ";
    let error = type_source_package(&package(source)).unwrap_err();
    assert_eq!(error.message, "conflicting type equation for use_flag");
}

#[test]
fn a_cached_signature_index_cannot_admit_another_demand_before_its_equation_closes() {
    let source = "
        data Flag<const B:bool>[copy]{}
        machine condition(v:bool)->bool { CONDITION }
        machine identity(v:bool)->bool { v }
        machine guarded<Item [copy], const B:bool>(item:Item)->Flag<identity(B)>
        requires Item == Flag::<condition(B)>
        reaches none crashes none
        { Flag::<identity(B)>{} }
        data Callback = machine(item:Flag<true>)->Flag<true> reaches none crashes none;
        machine width<const F:Callback>()->u64 { 8u64 }
        data Word { buffer:u<width::<guarded::<Flag<true>,true>>() >; }
    ";
    for (condition, agrees) in [("v", true), ("!v", false)] {
        let source = source.replace("CONDITION", condition);
        let package = package(&source);
        let file = package.trees().next().unwrap().file;
        let flag = package.declarations.iter().find(|declaration| declaration.path == "Flag").unwrap();
        let yes = ValueType::StaticValue { carrier: Box::new(ValueType::Bool), value: StaticValue::Bool(true) };
        let arguments = vec![ValueType::Applied(Box::new(flag.clone()), vec![yes.clone()]), yes];
        let start = source.find("identity(B)").unwrap();
        let span = omega_lexer::SourceSpan::new(start, start + "identity(B)".len());
        let recipe = file.nodes.iter().enumerate().find(|(node, syntax)| {
            syntax.kind == omega_parser::NodeKind::Call && file.span(*node) == span
        }).unwrap().0;
        let mut receipts = vec![SpecializedIndexValue {
            request: StaticValueRequest::at(file, recipe, &ValueType::Bool),
            arguments: arguments.clone(),
            value: StaticValue::Bool(true),
        }];
        let apply = |specialized_indices: &[SpecializedIndexValue]| {
            type_source_package_for_preparation(&package, &TypingEnvironment { specialized_indices, ..Default::default() })
        };
        let typing = apply(&receipts).unwrap();
        let TypingProgress::Pending(pending) = &typing else { panic!("expected the equation operand to need evaluation"); };
        let [StaticValueDemand::Specialized { request, arguments: owner_arguments }] = pending.demands() else {
            panic!("only the equation operand is ready: {:?}", pending.demands());
        };
        assert_eq!(request.expression.span.slice(source.as_bytes()), Some(b"condition(B)".as_slice()));
        assert_eq!(owner_arguments, &arguments);
        let pass = prepare_typing_pass(&typing, &[], None, Default::default()).unwrap();
        let PreparationAnswers::Pending(answers) = pass.answers else { panic!("expected one answer for the equation"); };
        assert_eq!(answers, [(pending.demands()[0].clone(), StaticValue::Bool(agrees))]);
        receipts.push(SpecializedIndexValue {
            request: request.clone(),
            arguments,
            value: StaticValue::Bool(agrees),
        });
        match apply(&receipts) {
            Ok(TypingProgress::Pending(ready)) if agrees => {
                let [StaticValueDemand::Closed(width)] = ready.demands() else { panic!("the matching equation makes width ready"); };
                assert_eq!(width.expression.span.slice(source.as_bytes()), Some(b"width::<guarded::<Flag<true>,true>>()".as_slice()));
            }
            Err(error) if !agrees => {
                assert_eq!(error.message, "conflicting type equation for guarded");
                assert_eq!(error.span.slice(source.as_bytes()), Some(b"guarded::<Flag<true>,true>".as_slice()));
            }
            other => panic!("unexpected result after supplying the equation operand: {other:?}"),
        }
    }
}

#[test]
fn an_inferred_recursive_application_preserves_the_callers_binder() {
    let source = "
        data Shell<T> [copy] {}
        machine grow<A [copy], B>(item:A) requires A == Shell::<B> {
            grow(Shell::<Shell<B>>{});
        }
    ";
    let typed = type_source_package(&package(source)).unwrap();
    let call = typed.calls.iter().find(|call| call.machine.path == "grow").unwrap();
    let [ValueType::Applied(shell, index), recovered @ ValueType::Applied(inner, caller)] = call.type_arguments.as_slice() else {
        panic!("expected two nested applications");
    };
    assert_eq!(shell, inner);
    assert_eq!(index, std::slice::from_ref(recovered));
    assert_eq!(caller, &[ValueType::Parameter { owner: Box::new(call.machine.clone()), index: 1 }]);

    let unanchored = "machine tie<A,B>() requires A == B && B == A {} machine use_tie() { tie(); }";
    let error = type_source_package(&package(unanchored)).unwrap_err();
    assert_eq!(error.message, "cannot infer all generic arguments");
}

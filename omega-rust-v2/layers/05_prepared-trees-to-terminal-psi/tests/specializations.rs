#[path = "support/source.rs"]
mod sources;
use omega_lowering::{BlockTerminator, OperationKind, Parameter, ReturnEdge, ScalarType, TypeShape};

#[test]
fn closed_calls_reuse_the_substituted_body_and_keep_distinct_type_arguments() {
    let defs = ("defs.omg", "public machine id<T>(x:T)->T {x} public machine wrap<T>(x:T)->T {id::<T>(x)}");
    let uses = (
        "uses.omg",
        "machine flag(x:bool)->bool {wrap::<bool>(x)} machine number(x:u8)->u8 {id::<u8>(x)} machine repeat(x:u8)->u8 {wrap::<u8>(x)}",
    );
    let pkg = sources::lower(&[defs, uses]).unwrap();
    assert_eq!(pkg, sources::lower(&[uses, defs]).unwrap());
    let machine = |name: &str| {
        pkg.machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
    };
    let target = |m: &omega_lowering::LoweredMachine| {
        let [op] = m.body.operations.as_slice() else {
            panic!("expected one direct call")
        };
        let OperationKind::Call {
            callee: omega_lowering::CallTarget::Machine(callee),
            ..
        } = &op.kind
        else {
            panic!("expected call")
        };
        &pkg.machines[callee.0.get() as usize - 1]
    };
    let bool_wrap = target(machine("flag"));
    let byte_wrap = target(machine("repeat"));
    let bool_id = target(bool_wrap);
    let byte_id = target(byte_wrap);
    assert_eq!(target(machine("number")).id, byte_id.id);
    assert_ne!(bool_id.id, byte_id.id);
    assert_ne!(bool_wrap.id, byte_wrap.id);
    for (id, wrap, ty) in [
        (bool_id, bool_wrap, ScalarType::Boolean),
        (byte_id, byte_wrap, ScalarType::Integer { signed: false, width: 8 }),
    ] {
        assert_eq!(id.declaration.path, "id");
        assert_eq!(wrap.declaration.path, "wrap");
        for m in [id, wrap] {
            assert_eq!(m.arguments, [omega_lowering::StaticArgument::Type(m.parameter_types[0])]);
            assert_eq!(m.result_type, Some(m.parameter_types[0]));
            assert_eq!(pkg.types.get(m.parameter_types[0]), Some(&TypeShape::PrimitiveScalar(ty)));
        }
        let [Parameter::Scalar(x)] = id.body.parameters[..] else {
            panic!("expected formal")
        };
        assert!(matches!(id.body.blocks[0].terminator,BlockTerminator::Return{result:ReturnEdge::Scalar(v),..} if v==x));
    }
    assert_eq!(pkg.machines.len(), 7);
}

#[test]
fn same_application_recursion_uses_the_registered_machine() {
    let text = "machine again<T>(flag:bool,x:T)->T {match flag { true -> x, false -> again::<T>(true,x), }}
              machine run(x:u8)->u8{again::<u8>(false,x)}";
    let package = sources::lower(&[("recursive.omg", text)]).unwrap();
    let again = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "again")
        .unwrap();
    assert_eq!(package.machines.len(), 2);
    assert!(
        again
            .body
            .operations
            .iter()
            .any(|op| matches!(&op.kind,OperationKind::Call{callee: omega_lowering::CallTarget::Machine(callee),..} if *callee==again.id))
    );
}

#[test]
fn an_instantiated_return_keeps_the_distinction_between_bits_and_a_borrowed_descriptor() {
    let lower = |carrier: &str| {
        let text = format!("machine keep<T>(v:T)->T{{v}} machine relay(v:{carrier})->{carrier}{{keep::<{carrier}>(v)}}");
        sources::lower(&[("return.omg", &text)])
    };
    let pkg = lower("[u8;2]").unwrap();
    let keep = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "keep")
        .unwrap();
    assert!(matches!(
        keep.body.blocks[0].terminator,
        BlockTerminator::Return {
            result: ReturnEdge::Structural { .. },
            ..
        }
    ));
    for (carrier, byte) in [("@[u8]", true), ("@[u32]", false)] {
        let package = lower(carrier).unwrap();
        let keep = package
            .machines
            .iter()
            .find(|machine| machine.declaration.path == "keep")
            .unwrap();
        let omega_lowering::Parameter::Structural(input) = keep.body.parameters[0] else {
            panic!("view descriptor")
        };
        assert!(matches!(keep.body.blocks[0].terminator,BlockTerminator::Return{result:ReturnEdge::Structural{source,..},..} if source==input));
        assert!(matches!(
            (byte, package.types.get(keep.parameter_types[0])),
            (true, Some(omega_lowering::TypeShape::SharedByteView)) | (false, Some(omega_lowering::TypeShape::SharedElementView { .. }))
        ));
        let relay = package
            .machines
            .iter()
            .find(|machine| machine.declaration.path == "relay")
            .unwrap();
        assert!(
            matches!(relay.body.operations.as_slice(),[op] if matches!(&op.kind,OperationKind::Call{callee:omega_lowering::CallTarget::Machine(id),arguments,..} if *id==keep.id && matches!(arguments.as_slice(),[omega_lowering::CallArgument::CopyWhole(source)] if relay.body.parameters[0]==omega_lowering::Parameter::Structural(*source))))
        );
    }
}

#[test]
fn a_selected_suspending_callback_retains_both_closed_call_sites() {
    let text = "public data SuspendingTransform=machine(value:u32)->u32 suspends;
              public machine apply<const F:SuspendingTransform>(value:u32)->u32 suspends {suspend F(value)}
              public machine local(value:u32)->u32 suspends {value}
              public machine remote(value:u32)->u32 suspends {value}
              public machine example()->u32 suspends {suspend apply::<local>(42)}
              public machine elsewhere()->u32 suspends {suspend apply::<remote>(42)}";
    let package = sources::lower(&[("pause.omg", text)]).unwrap();
    let machine = |name| {
        package
            .machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
    };
    let by_id = |id: omega_lowering::MachineId| package.machines.iter().find(|m| m.id == id).unwrap();
    // Each owner has exactly one suspending crossing, to the expected callee.
    let crossing = |owner: &omega_lowering::LoweredMachine| {
        let [row] = owner.body.suspension_sites.as_slice() else {
            panic!("missing closed callback crossing")
        };
        assert!(
            owner
                .body
                .call_acknowledgements
                .iter()
                .any(|a| a.call == row.call && a.effects.suspend)
        );
        assert_eq!(psi_handoff_consumer::suspending_local_calls(&package, &owner.body), [row.call]);
        let omega_lowering::CallTarget::Machine(callee) = row.callee else {
            panic!("expected a local callee")
        };
        let target = by_id(callee);
        assert!(target.contract.realized_pause.suspend);
        target
    };
    // Two closed applications of `apply` select different callbacks. Each
    // instance's callback invocation calls exactly the machine its application chose.
    let mut instances = Vec::new();
    for (caller, selected) in [("example", "local"), ("elsewhere", "remote")] {
        let instance = crossing(machine(caller));
        assert_eq!(instance.declaration.path, "apply");
        let callback = crossing(instance);
        spec_corpus::observe_handoff!("static-callback-selection/consumer", assert_eq!(callback.id, machine(selected).id));
        instances.push(instance.id);
    }
    assert_ne!(instances[0], instances[1]);
}

/// An open template publishes the sum of two callback results as three steps:
/// both binder calls in source order, then the `+` that reads their results.
/// The `+` carries the same primitive, policy and checker question as the
/// closed body below. A caller who selects `F` can evaluate the template from
/// Psi alone, and an input whose sum exceeds `u64` makes that question false,
/// which the checker, not lowering, must judge.
#[test]
fn open_callback_calls_preserve_operands_and_publish_their_sum() {
    use omega_lowering::{
        ArithmeticPolicy, IntegerObligation, IntegerOperation, IntegerPrimitive, TemplateCallTarget, TemplateOperand, TemplateResult, TemplateStep,
    };
    use psi_handoff_consumer::machine_templates::{Application, Evaluation, evaluate};
    let single = "data Combine=machine(x:u64,y:u64)->u64; machine flip<const F:Combine>(a:u64,b:u64)->u64{F(b,a)}";
    let pkg = sources::lower(&[("template.omg", single)]).unwrap();
    let m = &pkg.machine_templates[0];
    let [TemplateStep::Call(call)] = m.body.as_slice() else {
        panic!("expected the one binder call")
    };
    assert_eq!(call.target, TemplateCallTarget::StaticBinder(0));
    assert_eq!(call.operands, [1, 0].map(TemplateOperand::Formal));
    assert_eq!(m.result, Some(TemplateResult::Operand(TemplateOperand::Step(0))));

    // The exact bytes of the readiness fixture `handoff/open callback body beyond straight-line calls`.
    let source = "data Combine=machine(x:u64,y:u64)->u64; machine flip<const F:Combine>(a:u64,b:u64)->u64{F(b,a) + F(a,b)}";
    let pkg = sources::lower(&[("template.omg", source)]).unwrap();
    assert!(pkg.machines.is_empty());
    let [flip] = pkg.machine_templates.as_slice() else {
        panic!("expected the open flip")
    };
    let [
        TemplateStep::Call(first),
        TemplateStep::Call(second),
        TemplateStep::Integer(sum),
    ] = flip.body.as_slice()
    else {
        panic!("expected two binder calls and their sum, got {:?}", flip.body)
    };
    let spelled = |site: &omega_lowering::Site| {
        site.span
            .slice(source.as_bytes())
            .map(|b| std::str::from_utf8(b).unwrap())
    };
    let calls = [first, second].map(|c| (&c.target, c.operands.as_slice(), spelled(&c.site)));
    assert_eq!(
        calls,
        [
            (
                &TemplateCallTarget::StaticBinder(0),
                [1, 0].map(TemplateOperand::Formal).as_slice(),
                Some("F(b,a)")
            ),
            (
                &TemplateCallTarget::StaticBinder(0),
                [0, 1].map(TemplateOperand::Formal).as_slice(),
                Some("F(a,b)")
            ),
        ]
    );
    let exact_add = IntegerOperation {
        primitive: IntegerPrimitive::Add,
        policy: ArithmeticPolicy::Exact,
        obligations: vec![IntegerObligation::ResultFitsCarrier],
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/open-callback-sum",
        assert_eq!(
            (&sum.operation, sum.operands.as_slice(), &flip.result),
            (&exact_add, [TemplateOperand::Step(0), TemplateOperand::Step(1)].as_slice(), &Some(TemplateResult::Operand(TemplateOperand::Step(2))))
        ),
    );
    assert_eq!(spelled(&sum.site), Some("F(b,a) + F(a,b)"));
    assert_eq!(sum.result_type, flip.signature.parameters[0]);
    assert_eq!(flip.invokes, [0]);

    // A specializer that selects `pick(x,y) = x` computes pick(b,a) + pick(a,b) = b + a.
    let pick = |_: usize, args: &[u64]| args[0];
    let apply = |formals: &[u64]| {
        evaluate(
            flip,
            &Application {
                formals,
                statics: &[],
                select: &pick,
            },
        )
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/open-callback-sum-consumer",
        assert_eq!(
            (apply(&[3, 5]), apply(&[u64::MAX, 1])),
            (Some(Evaluation::Returned(8)), Some(Evaluation::QuestionFails { step: 2, question: IntegerObligation::ResultFitsCarrier }))
        ),
    );

    let constant = source.replace("F(b,a) + F(a,b)", "F(b,a) + 1");
    let pkg = sources::lower(&[("template.omg", &constant)]).unwrap();
    let [flip] = pkg.machine_templates.as_slice() else { panic!("one open flip") };
    let [TemplateStep::Call(_), TemplateStep::Integer(sum)] = flip.body.as_slice() else { panic!("callback and sum") };
    spec_corpus::observe_handoff!(psi "psi-output/template-integer-literal", assert_eq!(
        (&sum.operation, &sum.operands, &sum.result_type),
        (&exact_add, &vec![TemplateOperand::Step(0), TemplateOperand::Constant(omega_psi::ScalarLiteral::Integer(1.into()))],
         &omega_psi::TypeExpression(vec![omega_psi::TypeExpressionStep::Scalar(omega_psi::ScalarType::Integer { signed: false, width: 64 })]))
    ));
    let apply = |formals: &[u64]| evaluate(flip, &Application { formals, statics: &[], select: &pick });
    spec_corpus::observe_handoff!(psi "psi-output/template-integer-literal-consumer", assert_eq!(
        (apply(&[3,5]), apply(&[0,u64::MAX])),
        (Some(Evaluation::Returned(6)), Some(Evaluation::QuestionFails { step: 1, question: IntegerObligation::ResultFitsCarrier }))
    ));
}

/// Typing records both binder invocations and the sum of the open template above, and
/// the same body lowers once a caller selects the callback. The closed `+` is the same
/// Psi `IntegerOperation` that the open template publishes for its integer step.
#[test]
fn an_open_callback_sum_has_complete_typed_inputs_and_lowers_once_closed() {
    use omega_lowering::{ArithmeticPolicy, CallArgument, CallTarget, IntegerObligation, IntegerOperation, IntegerPrimitive, OperationResult};
    let open = "data Combine=machine(x:u64,y:u64)->u64; machine flip<const F:Combine>(a:u64,b:u64)->u64{F(b,a) + F(a,b)}";
    let resolved = sources::resolve(&[("template.omg", open)]);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let typed = prepared.typed();
    let u64 = omega_typed::ValueType::Integer {
        signed: false,
        width: Some(64),
    };
    let invocations: Vec<_> = typed
        .indirect_calls
        .iter()
        .filter(|c| matches!(&c.entry, omega_typed::InvocationTarget::StaticParameter { owner, index: 0 } if owner.path == "flip"))
        .map(|c| (c.caller.as_ref().map(|d| d.path.as_str()), c.site.span.slice(open.as_bytes()).unwrap()))
        .collect();
    assert_eq!(invocations, [(Some("flip"), &b"F(b,a)"[..]), (Some("flip"), &b"F(a,b)"[..])]);
    let [sum] = typed.operations.as_slice() else { panic!("expected the one sum") };
    assert_eq!(
        (sum.token.as_str(), sum.operands.as_slice(), &sum.result),
        ("+", [u64.clone(), u64.clone()].as_slice(), &u64)
    );
    assert_eq!(sum.site.span.slice(open.as_bytes()), Some(&b"F(b,a) + F(a,b)"[..]));

    let closed = format!("{open} machine pick(x:u64,y:u64)->u64{{x}} machine run(a:u64,b:u64)->u64{{flip::<pick>(a,b)}}");
    let pkg = sources::lower(&[("template.omg", &closed)]).unwrap();
    assert!(pkg.machine_templates.is_empty());
    let machine = |name: &str| {
        pkg.machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
    };
    let (flip, pick) = (machine("flip"), machine("pick"));
    let [Parameter::Scalar(a), Parameter::Scalar(b)] = flip.body.parameters[..] else {
        panic!("expected two scalar formals")
    };
    let [first, second, add] = flip.body.operations.as_slice() else {
        panic!("expected two callback calls and their sum")
    };
    let result = |op: &omega_lowering::PsiOperation| match op.result {
        OperationResult::Scalar(v) => v,
        _ => panic!("expected a scalar result"),
    };
    for (op, order) in [(first, [b, a]), (second, [a, b])] {
        assert!(
            matches!(&op.kind, OperationKind::Call{callee: CallTarget::Machine(id), arguments, ..} if *id == pick.id && *arguments == order.map(CallArgument::Scalar))
        );
    }
    assert_eq!(
        add.kind,
        OperationKind::Integer {
            operation: IntegerOperation {
                primitive: IntegerPrimitive::Add,
                policy: ArithmeticPolicy::Exact,
                obligations: vec![IntegerObligation::ResultFitsCarrier]
            },
            operands: vec![result(first), result(second)],
        }
    );
    assert!(matches!(flip.body.blocks[0].terminator, BlockTerminator::Return{result: ReturnEdge::Scalar(v), ..} if v == result(add)));
    let open = sources::lower(&[("template.omg", open)]).unwrap();
    let [.., omega_lowering::TemplateStep::Integer(sum)] = open.machine_templates[0].body.as_slice() else {
        panic!("expected the open sum")
    };
    assert!(matches!(&add.kind, OperationKind::Integer { operation, .. } if *operation == sum.operation));
}

#[test]
fn a_generic_machine_keeps_a_guarded_crash_distinct_from_an_unconditional_ceiling() {
    use omega_lowering::{CrashCause, PredicateOperator, PredicateStep};
    let text = "data NumberStep=machine(x:u64)->u64; machine relay<const F:NumberStep>(x:u64)->u64 crashes Trap x == 0 {F(x)}";
    let pkg = sources::lower(&[("guard.omg", text)]).unwrap();
    let signature = &pkg.machine_templates[0].signature;
    assert_eq!(signature.crash_ceiling.as_deref(), Some(&[CrashCause::Trap][..]));
    let [route] = signature.crash_routes.as_slice() else {
        panic!("expected the guarded route")
    };
    assert_eq!(route.cause, CrashCause::Trap);
    let guard = route
        .guard
        .as_ref()
        .expect("the guard stays on the template interface");
    assert_eq!(guard.site.span.slice(text.as_bytes()), Some(&b"x == 0"[..]));
    assert_eq!(
        guard.predicate,
        [
            PredicateStep::Formal(0),
            PredicateStep::Integer(0.into()),
            PredicateStep::Apply(PredicateOperator::Equal)
        ]
    );
    let plain = sources::lower(&[("guard.omg", &text.replace(" x == 0", ""))]).unwrap();
    let [route] = plain.machine_templates[0].signature.crash_routes.as_slice() else {
        panic!("expected one route")
    };
    assert_eq!((route.cause, route.guard.as_ref()), (CrashCause::Trap, None));
}

/// The abstract-premise sample after replacing its V1 crash statement and its
/// retired `requires machine` binder. `run` selects `witness` for `apply`.
const PREMISE: &str = "machine premise(kept:bool)->bool {true}
data Observer=machine(kept:bool)->bool
    requires premise(kept);
machine witness(kept:bool)->bool requires premise(kept) {premise(kept)}
machine apply<const Observe:Observer>(kept:bool)->bool requires premise(kept) {Observe(kept)}
machine run(kept:bool)->bool requires premise(kept) {apply::<witness>(kept)}";

fn named<'a>(package: &'a omega_lowering::LoweredPackage, name: &str) -> &'a omega_lowering::LoweredMachine {
    let [found] = package.machines.iter().filter(|m| m.declaration.path == name).collect::<Vec<_>>()[..] else {
        panic!("expected one lowered {name}")
    };
    found
}

/// A clause's steps with each helper application's site replaced by the text it
/// covers, so clauses written at different places can be compared.
fn spelled(clause: &omega_lowering::ContractClause, source: &str) -> Vec<String> {
    clause
        .predicate
        .iter()
        .map(|step| match step {
            omega_lowering::PredicateStep::MachineApplication { callee, operands, result, requirements, progress, site } => {
                let text = std::str::from_utf8(site.span.slice(source.as_bytes()).unwrap()).unwrap();
                format!("{}{operands:?}->{result:?} {requirements:?} {progress:?} at {text}", callee.path)
            }
            other => format!("{other:?}"),
        })
        .collect()
}

fn selection(apply: &omega_lowering::LoweredMachine) -> &omega_lowering::StaticCallableSelection {
    let [omega_lowering::StaticArgument::Callable(selection)] = apply.arguments.as_slice() else {
        panic!("expected one callable selection, found {:?}", apply.arguments)
    };
    selection
}

/// Inside the closed `apply<witness>` the generic body owes the alias's premise
/// at its call through `Observe`, under that call's argument. The call still
/// executes `witness` directly. The selection publishes the alias premise beside
/// `witness`'s own, so the checker can judge that the first implies the second.
/// Lowering decides neither question.
#[test]
fn a_closed_callback_under_a_premise_bearing_contract_owes_the_bound_and_publishes_the_refinement() {
    use omega_lowering::{BlockTerminator, CallArgument, CallSite, CallTarget};
    let package = sources::lower(&[("premise.omg", PREMISE)]).unwrap();
    let (witness, apply, run) = (named(&package, "witness"), named(&package, "apply"), named(&package, "run"));
    let selection = selection(apply);
    assert_eq!(selection.machine, witness.id);
    // The bound's clause belongs to the alias and names the alias's own formal.
    let [bound] = selection.requires.as_slice() else {
        panic!("expected the alias premise, found {:?}", selection.requires)
    };
    let [provided] = witness.contract.requires.as_slice() else { panic!("witness keeps its own premise") };
    assert_eq!(bound.site.span.slice(PREMISE.as_bytes()), Some(&b"requires premise(kept)"[..]));
    assert_eq!(bound.site.span.get_start_offset(), sources::unique_span(PREMISE, "requires premise(kept);").start);
    assert_ne!(bound.site, provided.site);
    // Both clauses apply `premise` to the clause owner's first formal, each at its own occurrence.
    assert_eq!(spelled(bound, PREMISE), ["Formal(0)", "premise[TypeExpression([Scalar(Boolean)])]->Boolean 0..0 CheckPrivateBody at premise(kept)"]);
    assert_eq!(spelled(bound, PREMISE), spelled(provided, PREMISE));
    let inside = |inner: &omega_lowering::Site, outer: &omega_lowering::Site| {
        outer.span.get_start_offset() <= inner.span.get_start_offset() && inner.span.get_end_offset() <= outer.span.get_end_offset()
    };
    let application = |c: &omega_lowering::ContractClause| match &c.predicate[1] {
        omega_lowering::PredicateStep::MachineApplication { site, .. } => site.clone(),
        other => panic!("expected the helper application, found {other:?}"),
    };
    assert!(inside(&application(bound), &bound.site) && inside(&application(provided), &provided.site));

    // `Observe(kept)` is a direct call to `witness` whose one requirement is the
    // binder's clause; the call is listed as an invocation of binder 0.
    let [omega_lowering::Parameter::Scalar(kept)] = apply.body.parameters[..] else { panic!("one formal") };
    let [call] = apply.body.operations.as_slice() else { panic!("one operation") };
    let OperationKind::Call { callee, arguments, requirements } = &call.kind else { panic!("expected the callback call") };
    assert_eq!((*callee, arguments.as_slice()), (CallTarget::Machine(witness.id), &[CallArgument::Scalar(kept)][..]));
    let site = CallSite::Operation(call.id);
    assert_eq!(requirements, &[omega_lowering::CallRequirement { site, clause: 0 }]);
    assert_eq!(apply.body.callback_invocations, [omega_lowering::CallbackInvocation { call: site, binder: 0 }]);
    assert!(matches!(apply.body.blocks[0].terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(v), .. } if call.result == omega_lowering::OperationResult::Scalar(v)));

    // An independent consumer reads the same two questions from Psi alone.
    let premises = psi_handoff_consumer::callback_premises(&package, apply).expect("well-formed callback listing");
    let [premises] = premises.as_slice() else { panic!("one callback invocation") };
    spec_corpus::observe_handoff!(psi "psi-output/closed-callback-owes-bound-premise",
        assert_eq!((premises.selected.id, premises.arguments, premises.owed.as_slice(), premises.provided),
                   (witness.id, &[CallArgument::Scalar(kept)][..], &[bound][..], &witness.contract.requires[..])));
    // The caller's own call to `apply` is ordinary: it owes `apply`'s clause and lists no invocation.
    assert!(run.body.callback_invocations.is_empty());
    assert!(psi_handoff_consumer::callback_premises(&package, run).is_some_and(|rows| rows.is_empty()));
}

/// Each change touches one input. Changing only `witness`'s premise changes only
/// the refinement's right side. Removing only the alias premise leaves the call
/// owing nothing while `witness` keeps its premise, so the refinement is still
/// the checker's to judge. Negating only the argument keeps the owed clause and
/// changes the actual. A bound guarantee and a target guarantee remain independent.
#[test]
fn premise_bearing_callback_twins_change_only_their_own_input() {
    use omega_lowering::CallArgument;
    let base = sources::lower(&[("premise.omg", PREMISE)]).unwrap();
    let questions = |package: &omega_lowering::LoweredPackage, source: &str| {
        let apply = named(package, "apply");
        let rows = psi_handoff_consumer::callback_premises(package, apply).expect("well-formed callback listing");
        let [row] = rows.as_slice() else { panic!("one callback invocation") };
        let owed: Vec<_> = row.owed.iter().map(|c| spelled(c, source)).collect();
        let provided: Vec<_> = row.provided.iter().map(|c| spelled(c, source)).collect();
        (owed, provided, row.arguments.to_vec(), selection(apply).requires.len())
    };
    let (owed, provided, arguments, _) = questions(&base, PREMISE);

    let weaker = PREMISE.replace("witness(kept:bool)->bool requires premise(kept)", "witness(kept:bool)->bool requires !kept");
    let twin = sources::lower(&[("premise.omg", &weaker)]).unwrap();
    let (twin_owed, twin_provided, twin_arguments, _) = questions(&twin, &weaker);
    assert_eq!((&twin_owed, &twin_arguments), (&owed, &arguments));
    assert_eq!(twin_provided, [["Formal(0)", "Apply(Not)"]]);

    let open = PREMISE.replace("machine(kept:bool)->bool\n    requires premise(kept);", "machine(kept:bool)->bool;");
    let twin = sources::lower(&[("premise.omg", &open)]).unwrap();
    let (twin_owed, twin_provided, twin_arguments, count) = questions(&twin, &open);
    assert_eq!((twin_owed.len(), count, &twin_provided, &twin_arguments), (0, 0, &provided, &arguments));

    let negated = PREMISE.replace("{Observe(kept)}", "{Observe(!kept)}");
    let twin = sources::lower(&[("premise.omg", &negated)]).unwrap();
    let (twin_owed, twin_provided, twin_arguments, _) = questions(&twin, &negated);
    assert_eq!((&twin_owed, &twin_provided), (&owed, &provided));
    let apply = named(&twin, "apply");
    let [omega_lowering::Parameter::Scalar(kept)] = apply.body.parameters[..] else { panic!("one formal") };
    assert_ne!(twin_arguments, arguments);
    assert!(matches!(twin_arguments[..], [CallArgument::Scalar(v)] if v != kept));

    let promised = PREMISE.replace("    requires premise(kept);", "    requires premise(kept)\n    ensures result == kept;");
    let facts = |source: &str| {
        let package = sources::lower(&[("premise.omg", source)]).unwrap();
        let apply = named(&package, "apply");
        let rows = psi_handoff_consumer::callback_premises(&package, apply).unwrap();
        let [row] = rows.as_slice() else { panic!("one callback invocation") };
        let clauses = |list: &[omega_psi::ContractClause]| list.iter().map(|c| c.predicate.clone()).collect::<Vec<_>>();
        assert_eq!(row.selected.declaration.path, "witness");
        let [omega_psi::Parameter::Scalar(kept)] = apply.body.parameters[..] else { panic!("one formal") };
        assert_eq!(row.arguments, [CallArgument::Scalar(kept)]);
        let omega_psi::CallSite::Operation(id) = row.call else { panic!("an ordinary invocation") };
        let omega_psi::OperationResult::Scalar(value) = apply.body.operations[id.index()].result else { panic!("normal callback result") };
        assert!(matches!(apply.body.blocks[0].terminator, omega_psi::BlockTerminator::Return { result: omega_psi::ReturnEdge::Scalar(returned), .. } if returned == value));
        assert!(apply.contract.ensures.is_empty());
        (clauses(row.promised), clauses(row.target_guarantees))
    };
    use omega_psi::{PredicateOperator, PredicateStep};
    let relation = vec![PredicateStep::Result, PredicateStep::Formal(0), PredicateStep::Apply(PredicateOperator::Equal)];
    let with_target = promised.replace("requires premise(kept) {premise(kept)}", "requires premise(kept) ensures result == kept {premise(kept)}");
    spec_corpus::observe_handoff!(psi "psi-output/closed-callback-guarantees", assert_eq!(
        (facts(&promised), facts(&with_target)), ((vec![relation.clone()], vec![]), (vec![relation.clone()], vec![relation]))
    ));
}

/// A callback fact in a closed instance owes its own bound's premises under
/// this occurrence's ordered predicate terms and retains the selected target.
#[test]
fn a_callback_fact_owes_the_bound_premise_at_its_own_application() {
    let open = "machine premise(kept:bool)->bool {true}
data Observer=machine(kept:bool)->bool terminates reaches none crashes none;
machine witness(kept:bool)->bool terminates reaches none crashes none {kept}
machine apply<const Observe:Observer>(kept:bool)->bool requires premise(kept) requires Observe(kept) {kept}
machine run(kept:bool)->bool requires premise(kept) requires witness(kept) {apply::<witness>(kept)}";
    let premised = open.replace("machine(kept:bool)->bool terminates", "machine(kept:bool)->bool requires premise(kept) terminates");
    let debts = |source: &str| {
        let package = sources::lower(&[("fact.omg", source)]).unwrap();
        let (terms, owed, witness) = psi_handoff_consumer::predicate_callback(&package, named(&package, "apply"), 1).unwrap();
        assert_eq!(witness.declaration.path, "witness");
        let clauses = |rows: &[omega_psi::ContractClause]| rows.iter().map(|c| c.site.span.slice(source.as_bytes()).unwrap().to_vec()).collect::<Vec<_>>();
        (terms.to_vec(), clauses(owed), clauses(&witness.contract.requires))
    };
    use omega_psi::PredicateStep::Formal;
    spec_corpus::observe_handoff!(psi "psi-output/callback-predicate-premises", assert_eq!(
        (debts(open), debts(&premised)),
        ((vec![Formal(0)], vec![], vec![]), (vec![Formal(0)], vec![b"requires premise(kept)".to_vec()], vec![]))
    ));
    let stronger = premised.replace("machine witness(kept:bool)->bool terminates", "machine witness(kept:bool)->bool requires kept terminates");
    assert_eq!(debts(&stronger), (vec![Formal(0)], vec![b"requires premise(kept)".to_vec()], vec![b"requires kept".to_vec()]));
}

/// The consumer reads the listing, not a guess: dropping the owed clause,
/// redirecting the call or pointing the listing at a non-callable binder each
/// makes the same package unreadable.
#[test]
fn the_callback_premise_reader_rejects_a_listing_that_disagrees_with_the_call() {
    use omega_lowering::{CallTarget, OperationKind};
    let index = |package: &omega_lowering::LoweredPackage| package.machines.iter().position(|m| m.declaration.path == "apply").unwrap();
    let premise = |package: &omega_lowering::LoweredPackage| named(package, "premise").id;
    let edits: [(&str, fn(&mut omega_lowering::LoweredMachine, omega_lowering::MachineId)); 3] = [
        ("drop the owed clause", |apply, _| {
            let OperationKind::Call { requirements, .. } = &mut apply.body.operations[0].kind else { unreachable!() };
            requirements.clear();
        }),
        ("redirect the call", |apply, other| {
            let OperationKind::Call { callee, .. } = &mut apply.body.operations[0].kind else { unreachable!() };
            *callee = CallTarget::Machine(other);
        }),
        ("name another binder", |apply, _| apply.body.callback_invocations[0].binder = 1),
    ];
    for (edit, change) in edits {
        let mut package = sources::lower(&[("premise.omg", PREMISE)]).unwrap();
        let (at, other) = (index(&package), premise(&package));
        assert!(psi_handoff_consumer::callback_premises(&package, &package.machines[at]).is_some_and(|rows| rows.len() == 1));
        change(&mut package.machines[at], other);
        assert!(psi_handoff_consumer::callback_premises(&package, &package.machines[at]).is_none(), "{edit}");
    }
}

/// Combined control for three repairs that meet in one exported wrapper. An
/// exported generic machine keeps its open template beside the closed instance
/// its local caller selects. The closed `apply<witness>` owes the alias premise
/// at its callback call, listed as an invocation of binder 0. The open `apply`
/// is a template step calling binder 0 with formal 0; its premise stays on the
/// binder's published signature, the only place an open call can find it.
/// Neither product decides whether the premise holds.
#[test]
fn an_exported_premise_bearing_wrapper_keeps_its_template_beside_the_owing_instance() {
    use omega_lowering::{CallRequirement, CallSite, CallbackInvocation, MachineTemplateBinder, TemplateCallTarget, TemplateOperand, TemplateStep};
    let exported = PREMISE
        .replace("machine apply<", "public machine apply<")
        .replace("data Observer=", "public data Observer=")
        .replace("machine premise(kept:bool)->bool {true}", "public machine premise(kept:bool)->bool crashes none terminates {true}");
    let package = sources::lower(&[("premise.omg", &exported)]).unwrap();
    let apply = named(&package, "apply");
    let [bound] = selection(apply).requires.as_slice() else { panic!("expected the alias premise on the selection") };
    let [call] = apply.body.operations.as_slice() else { panic!("one operation") };
    let OperationKind::Call { requirements, .. } = &call.kind else { panic!("expected the callback call") };
    let site = CallSite::Operation(call.id);
    assert_eq!(requirements, &[CallRequirement { site, clause: 0 }]);
    assert_eq!(apply.body.callback_invocations, [CallbackInvocation { call: site, binder: 0 }]);

    let [template] = package.machine_templates.as_slice() else { panic!("expected the exported open template") };
    assert!(template.public && template.declaration == apply.declaration);
    let [MachineTemplateBinder::Callable { signature, .. }] = template.parameters.as_slice() else { panic!("expected the callable binder") };
    let [open_bound] = signature.requires.as_slice() else { panic!("the binder signature keeps the alias premise") };
    assert_eq!((open_bound.site.clone(), open_bound.predicate.len()), (bound.site.clone(), bound.predicate.len()));
    let [TemplateStep::Call(open_call)] = template.body.as_slice() else { panic!("one call step") };
    assert_eq!((&open_call.target, open_call.operands.as_slice()), (&TemplateCallTarget::StaticBinder(0), &[TemplateOperand::Formal(0)][..]));
    assert_eq!(template.invokes, [0]);
}

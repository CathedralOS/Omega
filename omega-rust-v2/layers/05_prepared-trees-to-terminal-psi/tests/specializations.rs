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

#[test]
fn open_callback_calls_preserve_operands_and_reject_an_unlowered_body() {
    let source = "data Combine=machine(x:u64,y:u64)->u64; machine flip<const F:Combine>(a:u64,b:u64)->u64{F(b,a)}";
    let pkg = sources::lower(&[("template.omg", source)]).unwrap();
    let m = &pkg.machine_templates[0];
    assert_eq!(m.body[0].target, omega_lowering::TemplateCallTarget::StaticBinder(0));
    assert_eq!(m.body[0].operands, [1, 0]);
    assert_eq!(m.result, Some(0));
    let bad = source.replace("F(b,a)", "F(b,a) + F(a,b)");
    let err = sources::lower(&[("template.omg", &bad)]).unwrap_err();
    assert_eq!(err.message, "open generic machine requires a closed application before lowering");
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

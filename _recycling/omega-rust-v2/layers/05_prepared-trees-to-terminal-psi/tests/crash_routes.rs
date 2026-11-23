#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    CallArgument, CallTarget, CrashCause, CrashRoute, LoweredMachine, LoweredPackage, OperationKind, Parameter, PredicateOperator, PredicateStep, Site,
};
use PredicateOperator::{Equal, NotEqual};
use PredicateStep::{Apply, Formal, Integer};

fn lower(source: &str) -> LoweredPackage {
    sources::lower(&[("route.omg", source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"))
}

fn machine<'p>(
    package: &'p LoweredPackage,
    path: &str,
) -> &'p LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == path)
        .unwrap_or_else(|| panic!("missing machine {path}"))
}

fn spelling<'s>(
    source: &'s str,
    site: &Site,
) -> &'s str {
    std::str::from_utf8(site.span.slice(source.as_bytes()).unwrap()).unwrap()
}

fn guard(route: &CrashRoute) -> &[PredicateStep] {
    &route.guard.as_ref().expect("a guarded route").predicate
}

#[test]
fn a_guarded_machine_route_keeps_its_cause_guard_and_entry_formal() {
    let source = "machine divide(left:u8, right:u8)->u8 crashes Trap right == 0 {left}";
    let package = lower(source);
    let divide = machine(&package, "divide");
    assert!(!divide.contract.has_empty_crash_ceiling);
    let [route] = divide.contract.crash_routes.as_slice() else {
        panic!("expected one published route")
    };
    assert_eq!(route.cause, CrashCause::Trap);
    assert_eq!(spelling(source, &route.site), "crashes Trap right == 0");
    assert_eq!(spelling(source, &route.guard.as_ref().unwrap().site), "right == 0");
    assert_eq!(guard(route), [Formal(1), Integer(0.into()), Apply(Equal)]);

    // Each authored difference reaches the published route.
    for (original, replacement, cause, expected) in [
        ("right == 0", "left == 0", CrashCause::Trap, [Formal(0), Integer(0.into()), Apply(Equal)]),
        ("right == 0", "right != 0", CrashCause::Trap, [Formal(1), Integer(0.into()), Apply(NotEqual)]),
        ("right == 0", "right == 7", CrashCause::Trap, [Formal(1), Integer(7.into()), Apply(Equal)]),
        ("Trap", "Abort", CrashCause::Abort, [Formal(1), Integer(0.into()), Apply(Equal)]),
    ] {
        let changed = source.replace(original, replacement);
        let package = lower(&changed);
        let [route] = machine(&package, "divide").contract.crash_routes.as_slice() else {
            panic!("{changed}: expected one route")
        };
        assert_eq!(route.cause, cause, "{changed}");
        assert_eq!(guard(route), expected, "{changed}");
    }
}

#[test]
fn routes_keep_authored_order_and_an_unguarded_route_is_truth() {
    let source = "machine pick(a:u8, b:u8)->u8 crashes Abort b == 0 crashes Trap {a}";
    let package = lower(source);
    let [abort, trap] = machine(&package, "pick").contract.crash_routes.as_slice() else {
        panic!("expected two routes")
    };
    assert_eq!(abort.cause, CrashCause::Abort);
    assert_eq!(guard(abort), [Formal(1), Integer(0.into()), Apply(Equal)]);
    assert_eq!((trap.cause, trap.guard.as_ref()), (CrashCause::Trap, None));
    assert_eq!(spelling(source, &trap.site), "crashes Trap");

    let swapped = "machine pick(a:u8, b:u8)->u8 crashes Trap crashes Abort b == 0 {a}";
    let package = lower(swapped);
    let [trap, abort] = machine(&package, "pick").contract.crash_routes.as_slice() else {
        panic!("expected two routes")
    };
    assert_eq!((trap.cause, trap.guard.as_ref()), (CrashCause::Trap, None));
    assert_eq!((abort.cause, guard(abort)), (CrashCause::Abort, &[Formal(1), Integer(0.into()), Apply(Equal)][..]));

    // `crashes none` publishes an empty ceiling with no route.
    let package = lower("machine pick(a:u8, b:u8)->u8 crashes none {a}");
    let pick = machine(&package, "pick");
    assert!(pick.contract.has_empty_crash_ceiling);
    assert!(pick.contract.crash_routes.is_empty());
}

#[test]
fn a_call_keeps_its_routed_callee_and_ordered_operands() {
    let declarations = "machine pick(x:u8, y:u8)->u8 crashes Trap y == 0 {x}";
    for (operands, swapped) in [("a, b", false), ("b, a", true)] {
        let source = format!("{declarations} machine outer(a:u8, b:u8)->u8 crashes Trap {{pick({operands})}}");
        let package = lower(&source);
        let pick = machine(&package, "pick");
        let outer = machine(&package, "outer");
        let [route] = pick.contract.crash_routes.as_slice() else {
            panic!("expected the callee route")
        };
        assert_eq!(route.cause, CrashCause::Trap);
        assert_eq!(guard(route), [Formal(1), Integer(0.into()), Apply(Equal)]);
        let [outer_route] = outer.contract.crash_routes.as_slice() else {
            panic!("expected the caller route")
        };
        assert_eq!((outer_route.cause, outer_route.guard.as_ref()), (CrashCause::Trap, None));

        let [Parameter::Scalar(a), Parameter::Scalar(b)] = outer.body.parameters[..] else {
            panic!("expected two scalar formals")
        };
        let [call] = outer.body.operations.as_slice() else {
            panic!("expected one call")
        };
        let OperationKind::Call {
            callee: CallTarget::Machine(callee),
            arguments,
            ..
        } = &call.kind
        else {
            panic!("expected a direct call")
        };
        // The route's formal 1 is instantiated by the second actual at this exact site.
        assert_eq!(*callee, pick.id);
        let expected = if swapped {
            [CallArgument::Scalar(b), CallArgument::Scalar(a)]
        } else {
            [CallArgument::Scalar(a), CallArgument::Scalar(b)]
        };
        assert_eq!(*arguments, expected, "{source}");
    }
}

#[test]
fn a_callee_guard_names_its_own_formal_not_the_caller_spelling() {
    // The callee guard is fixed by its declaration; the caller's own route is separate.
    let source = "machine check(value:u8)->u8 crashes Abort value == 3 {value}
                  machine outer(value:u8, other:u8)->u8 crashes Abort other == 3 {check(value)}";
    let package = lower(source);
    let check = machine(&package, "check");
    let outer = machine(&package, "outer");
    assert_eq!(guard(&check.contract.crash_routes[0]), [Formal(0), Integer(3.into()), Apply(Equal)]);
    assert_eq!(guard(&outer.contract.crash_routes[0]), [Formal(1), Integer(3.into()), Apply(Equal)]);
    assert_eq!(check.contract.crash_routes[0].cause, CrashCause::Abort);
    assert_eq!(outer.contract.crash_routes[0].cause, CrashCause::Abort);

    // At check(value), the callee's guard reads value, not outer's guarded other.
    let [call] = outer.body.operations.as_slice() else {
        panic!("expected one call to check")
    };
    let OperationKind::Call { callee: CallTarget::Machine(callee), arguments, .. } = &call.kind else {
        panic!("expected a direct call to check")
    };
    assert_eq!(*callee, check.id);
    psi_handoff_consumer::product_witness::call_reads_formals(&outer.body, arguments, &[0]).expect("check reads outer's value");
}

#[test]
fn a_generic_instance_keeps_the_route_of_its_declaration() {
    let source = "machine keep<T>(value:T, count:u8)->T crashes Trap count == 0 {value}
                  machine run(x:u8, n:u8)->u8 crashes Trap {keep::<u8>(x, n)}";
    let package = lower(source);
    let keep = machine(&package, "keep");
    let [route] = keep.contract.crash_routes.as_slice() else {
        panic!("expected the instance route")
    };
    assert_eq!(route.cause, CrashCause::Trap);
    assert_eq!(guard(route), [Formal(1), Integer(0.into()), Apply(Equal)]);
    let run = machine(&package, "run");
    let [Parameter::Scalar(x), Parameter::Scalar(n)] = run.body.parameters[..] else {
        panic!("expected two scalar formals")
    };
    let [call] = run.body.operations.as_slice() else {
        panic!("expected one call")
    };
    assert!(
        matches!(&call.kind, OperationKind::Call { callee: CallTarget::Machine(callee), arguments, .. }
            if *callee == keep.id && *arguments == [CallArgument::Scalar(x), CallArgument::Scalar(n)]),
        "{call:?}"
    );
}

#[test]
fn a_nominal_member_keeps_the_selected_crash_guard_and_actual_under_an_empty_ceiling() {
    for clause in ["", " crashes Trap v == 0 "] {
        let source = format!(
            "trait Step {{ machine next(v:u8)->u8; }}
             machine risky(v:u8)->u8{clause}{{v}}
             Local satisfies Step {{ Step::next=risky; }}
             machine use_step<const E:Step>(n:u8)->u8{{E::next(n)}}
             machine outer(a:u8)->u8 crashes none {{use_step::<Local>(a)}}"
        );
        let package = lower(&source);
        let outer = machine(&package, "outer");
        assert!(outer.contract.has_empty_crash_ceiling && outer.contract.crash_routes.is_empty());
        let [call] = outer.body.operations.as_slice() else {
            panic!("expected the application of use_step")
        };
        let OperationKind::Call {
            callee: CallTarget::Machine(instance),
            arguments,
            ..
        } = &call.kind
        else {
            panic!("expected the selected local machine")
        };
        psi_handoff_consumer::product_witness::call_reads_formals(&outer.body, arguments, &[0]).unwrap();
        let wrapper = machine(&package, "use_step");
        assert_eq!(*instance, wrapper.id);
        let [call] = wrapper.body.operations.as_slice() else {
            panic!("expected the call to Step::next")
        };
        let (selection, arguments) = psi_handoff_consumer::member_call(&package, &call.kind).unwrap();
        assert_eq!(selection.requirement.declaration.path, "Step::next");
        psi_handoff_consumer::product_witness::call_reads_formals(&wrapper.body, arguments, &[0]).unwrap();
        let omega_lowering::MemberRealization::Conformance {
            declaration,
            machine: selected,
        } = &selection.target
        else {
            panic!("expected the implementation supplied by Local")
        };
        assert_eq!(declaration.path, "Local");
        let risky = machine(&package, "risky");
        assert_eq!(*selected, risky.id);
        if clause.is_empty() {
            assert!(risky.contract.crash_routes.is_empty());
        } else {
            let [route] = risky.contract.crash_routes.as_slice() else {
                panic!("expected one route on the selected implementation")
            };
            assert_eq!(route.cause, CrashCause::Trap);
            assert_eq!(guard(route), [Formal(0), Integer(0.into()), Apply(Equal)]);
        }
    }
}

fn sole_direct_call(machine: &LoweredMachine) -> (omega_lowering::MachineId, &[CallArgument]) {
    let [op] = machine.body.operations.as_slice() else { panic!("expected one operation") };
    let OperationKind::Call { callee: CallTarget::Machine(id), arguments, .. } = &op.kind else { panic!("expected direct invocation") };
    (*id, arguments)
}

#[test]
fn a_closed_callback_keeps_the_selected_route_at_its_invocation() {
    let routed = "machine guarded(x:u8)->u8 crashes Trap x == 0 {x}
                  data ByteStep=machine(x:u8)->u8;
                  machine apply<const F:ByteStep>(v:u8)->u8{F(v)}
                  machine outer(a:u8)->u8{apply::<guarded>(a)}";
    let unrouted = routed.replace(" crashes Trap x == 0", "");
    for (source, has_route) in [(routed, true), (unrouted.as_str(), false)] {
        let package = lower(source);
        let (outer, apply, guarded) = (machine(&package, "outer"), machine(&package, "apply"), machine(&package, "guarded"));
        let (instance, outer_args) = sole_direct_call(outer);
        let (target, inner_args) = sole_direct_call(apply);
        let [omega_lowering::StaticArgument::Callable(binding)] = apply.arguments.as_slice() else { panic!("missing binder") };
        assert!(!binding.empty_crash_ceiling);
        psi_handoff_consumer::product_witness::call_reads_formals(&outer.body, outer_args, &[0]).unwrap();
        psi_handoff_consumer::product_witness::call_reads_formals(&apply.body, inner_args, &[0]).unwrap();
        spec_corpus::observe_handoff!(psi "psi-output/callback-route-selection",
            assert_eq!((instance, binding.machine, target), (apply.id, guarded.id, guarded.id)));
        assert!(apply.contract.crash_routes.is_empty() && !apply.contract.has_empty_crash_ceiling);
        if has_route {
            let [dependency] = apply.contract.callback_crash_routes.as_slice() else { panic!("missing selected dependency") };
            let [route] = guarded.contract.crash_routes.as_slice() else { panic!("missing callback route") };
            spec_corpus::observe_handoff!(psi "psi-output/callback-route-guard",
                assert_eq!((dependency.binder, dependency.routes.clone(), dependency.operand_formals.as_slice(),
                            spelling(source, &dependency.invocation), route.cause, guard(route)),
                           (0, 0..1, &[0][..], "F(v)", CrashCause::Trap, &[Formal(0), Integer(0.into()), Apply(Equal)][..])));
        } else {
            assert!(guarded.contract.crash_routes.is_empty() && apply.contract.callback_crash_routes.is_empty());
        }
    }
}

#[test]
fn callback_routes_distinguish_swapped_actuals_and_sibling_selections() {
    let source = "machine first(x:u8,y:u8)->u8 crashes Trap x == 7 {x}
                  machine second(x:u8,y:u8)->u8 crashes Abort y != 3 crashes Trap x == 0 {y}
                  data Step=machine(x:u8,y:u8)->u8;
                  machine flip<const G:Step,const F:Step>(a:u8,b:u8)->u8{F(b,a)}
                  machine one(i:u8,j:u8)->u8{flip::<second,first>(i,j)}
                  machine two(i:u8,j:u8)->u8{flip::<first,second>(j,i)}";
    let package = lower(source);
    for (caller, selected, order, cause, predicate) in [
        ("one", "first", [0,1], CrashCause::Trap, [Formal(0), Integer(7.into()), Apply(Equal)]),
        ("two", "second", [1,0], CrashCause::Abort, [Formal(1), Integer(3.into()), Apply(NotEqual)]),
    ] {
        let outer = machine(&package, caller);
        let (instance, actuals) = sole_direct_call(outer);
        psi_handoff_consumer::product_witness::call_reads_formals(&outer.body, actuals, &order).unwrap();
        let flip = package.machines.iter().find(|m| m.id == instance).unwrap();
        assert_eq!(flip.declaration.path, "flip");
        let (target, inner) = sole_direct_call(flip);
        psi_handoff_consumer::product_witness::call_reads_formals(&flip.body, inner, &[1,0]).unwrap();
        let callback = machine(&package, selected);
        let [dependency] = flip.contract.callback_crash_routes.as_slice() else { panic!("missing dependency") };
        let omega_lowering::StaticArgument::Callable(binding) = &flip.arguments[dependency.binder] else { panic!("not a binder") };
        assert_eq!(dependency.binder, 1, "the unused, equally typed G contributes no route");
        assert_eq!(dependency.routes, 0..if selected == "second" { 2 } else { 1 });
        let routes = &callback.contract.crash_routes[dependency.routes.clone()];
        if selected == "second" {
            assert_eq!((routes[1].cause, guard(&routes[1])), (CrashCause::Trap, &[Formal(0), Integer(0.into()), Apply(Equal)][..]));
        }
        let route = &routes[0];
        spec_corpus::observe_handoff!(psi "psi-output/callback-route-substitution",
            assert_eq!((binding.machine, target, dependency.operand_formals.as_slice(), route.cause, guard(route)),
                       (callback.id, callback.id, &[1,0][..], cause, predicate.as_slice())));
    }
}

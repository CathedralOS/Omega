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

#[test]
fn a_routed_callback_selection_is_refused_before_its_guard_is_lost() {
    let routed = "machine guarded(x:u8)->u8 crashes Trap x == 0 {x}
                  data ByteStep=machine(x:u8)->u8;
                  machine apply<const F:ByteStep>(v:u8)->u8{F(v)}
                  machine outer(a:u8)->u8{apply::<guarded>(a)}";
    // The selected callback's crash ceiling reaches outer through apply. The header
    // check refuses outer's effects, before its body or the specialized apply lowers.
    let error = sources::refuse(
        &[("route.omg", routed)],
        "route.omg",
        sources::unique_span(routed, "machine outer(a:u8)->u8{apply::<guarded>(a)}"),
        "machine requires contract or effects lowering",
    );
    assert!(
        [
            "a callback crash route requires indirect invocation coverage",
            "machine requires contract or effects lowering",
        ]
        .contains(&error.message.as_str()),
        "{error:?}"
    );
    // Without the route the same selection lowers to a direct call.
    let unrouted = routed.replace(" crashes Trap x == 0", "");
    sources::lower(&[("route.omg", &unrouted)]).unwrap_or_else(|error| panic!("{unrouted}: {error:?}"));
}

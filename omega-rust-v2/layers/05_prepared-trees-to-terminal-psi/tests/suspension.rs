#[path = "support/source.rs"]
mod source;
use omega_lowering::{CallSite, CallTarget, CompletionContinuation, OperationKind, OperationResult, Parameter};

#[test]
fn a_parked_call_keeps_its_argument_and_later_operand_without_committing_its_result() {
    for (effect, marker, parks) in [
        (" suspends", "suspend ", true),
        ("", "", false),
        ("", "suspend ", false),
    ] {
        let text = format!(
            "machine step(n:u8)->u8{effect} {{n}} machine use_it(n:u8,keep:u8,dead:u8)->u8{effect} {{ let answer:u8 = {marker}step(n); answer +% keep }}"
        );
        let pkg = source::lower(&[("park.omg", &text)]).unwrap();
        let machine = |name| {
            pkg.machines
                .iter()
                .find(|m| m.declaration.path == name)
                .unwrap()
        };
        let owner = machine("use_it");
        let target = machine("step");
        let body = &owner.body;
        assert_eq!(target.contract.pause_bounds.declared().suspend, parks);
        assert_eq!(owner.contract.realized_pause.suspend, parks);
        let [call, successor] = body.operations.as_slice() else {
            panic!("expected an invocation and addition")
        };
        let OperationKind::Call { callee, .. } = call.kind else {
            panic!("missing ordinary invocation")
        };
        assert_eq!(callee, CallTarget::Machine(target.id));
        let site = CallSite::Operation(call.id);
        assert_eq!(
            body.call_acknowledgements
                .iter()
                .map(|a| (a.call, a.effects.suspend))
                .collect::<Vec<_>>(),
            if marker.is_empty() { vec![] } else { vec![(site, true)] }
        );
        assert_eq!(
            psi_handoff_consumer::suspending_local_calls(&pkg, body),
            body.suspension_sites.iter().map(|s| s.call).collect::<Vec<_>>()
        );
        if !parks {
            assert!(body.suspension_sites.is_empty());
            continue;
        }
        let [crossing] = body.suspension_sites.as_slice() else {
            panic!("expected one crossing")
        };
        assert_eq!(
            (crossing.call, crossing.callee, crossing.continuation),
            (site, callee, CompletionContinuation::Operation(successor.id))
        );
        let [
            Parameter::Scalar(n),
            Parameter::Scalar(keep),
            Parameter::Scalar(dead),
        ] = body.parameters[..]
        else {
            panic!("scalar formals")
        };
        let OperationResult::Scalar(result) = call.result else {
            panic!("normal completion needs a scalar")
        };
        assert_eq!(
            crossing
                .activation
                .iter()
                .map(|row| (row.value, row.needed_by_callee, row.needed_after_completion))
                .collect::<Vec<_>>(),
            [(n, true, false), (keep, false, true)]
        );
        for row in &crossing.activation {
            assert_eq!(row.ty, body.values[row.value.index()].ty);
            assert!(row.carry.allows_suspension);
            assert!(!(row.carry.requires_same_thread || row.carry.requires_same_cpu || row.carry.requires_stable_address));
            assert!(row.value != result && row.value != dead);
        }
    }
}

#[test]
fn a_terminal_call_forwards_only_on_normal_completion() {
    let text = "machine step(n:u8)->u8 suspends {n} machine forward(n:u8)->u8 suspends{transition{_ -> suspend step(n),}}";
    let pkg = source::lower(&[("tail.omg", text)]).unwrap();
    let owner = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap();
    let [site] = owner.body.suspension_sites.as_slice() else {
        panic!("one terminal crossing")
    };
    assert!(matches!(site.call, CallSite::Edge(_)));
    assert_eq!(site.continuation, CompletionContinuation::Return);
    let [Parameter::Scalar(arg)] = owner.body.parameters[..] else {
        panic!("one scalar")
    };
    let [row] = site.activation.as_slice() else { panic!("one held argument") };
    assert_eq!((row.value, row.needed_by_callee, row.needed_after_completion), (arg, true, false));
}

#[test]
fn successive_crossings_own_distinct_frontiers_and_continuations() {
    let text = "machine step(n:u8)->u8 suspends {n} machine twice(n:u8,keep:u8,flag:bool)->u8 suspends{let a:u8=suspend step(n); let b:u8=suspend step(a); match flag{true -> b, false -> keep,}}";
    let pkg = source::lower(&[("twice.omg", text)]).unwrap();
    let body = &pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "twice")
        .unwrap()
        .body;
    let [first, second] = body.suspension_sites.as_slice() else {
        panic!("two crossings")
    };
    let (CallSite::Operation(a), CallSite::Operation(b)) = (first.call, second.call) else {
        panic!("ordinary calls")
    };
    let (OperationResult::Scalar(av), OperationResult::Scalar(bv)) = (body.operations[a.index()].result, body.operations[b.index()].result) else {
        panic!("scalar results")
    };
    let [
        Parameter::Scalar(n),
        Parameter::Scalar(keep),
        Parameter::Scalar(flag),
    ] = body.parameters[..]
    else {
        panic!("scalar formals")
    };
    assert_eq!(first.continuation, CompletionContinuation::Operation(b));
    assert!(!first.activation.iter().any(|v| v.value == av || v.value == bv));
    assert!(
        first
            .activation
            .iter()
            .any(|v| v.value == n && v.needed_by_callee)
    );
    assert!(
        second
            .activation
            .iter()
            .any(|v| v.value == av && v.needed_by_callee)
    );
    assert!(!second.activation.iter().any(|v| v.value == n || v.value == bv));
    for site in [first, second] {
        for after in [flag, keep] {
            assert!(
                site.activation
                    .iter()
                    .any(|v| v.value == after && v.needed_after_completion)
            );
        }
    }
}

#[test]
fn a_live_join_parameter_holds_its_predecessors_actual_values() {
    let text = "machine step(n:u8)->u8 suspends{n} machine pick(n:u8,keep:u8,flag:bool)->u8 suspends{let a:u8=suspend step(n); let picked:u8=match flag {true -> keep,false -> a,}; picked +% 1u8}";
    let pkg = source::lower(&[("join.omg", text)]).unwrap();
    let body = &pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "pick")
        .unwrap()
        .body;
    assert!(body.blocks.iter().any(|b| !b.scalar_parameters.is_empty()));
    let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
    let [
        Parameter::Scalar(n),
        Parameter::Scalar(keep),
        Parameter::Scalar(flag),
    ] = body.parameters[..]
    else {
        panic!("scalar formals")
    };
    assert_eq!(
        site.activation
            .iter()
            .map(|r| (r.value, r.needed_by_callee, r.needed_after_completion))
            .collect::<Vec<_>>(),
        [(n, true, false), (keep, false, true), (flag, false, true)]
    );
}

#[test]
fn ordinary_and_terminal_calls_retain_the_same_input_loan_until_completion() {
    for (tail, invocation) in [
        (false, "let count:u64 = suspend inspect(right); count"),
        (true, "transition{_ -> suspend inspect(right),}"),
    ] {
        let text = format!(
            "machine inspect(bytes:@[u8])->u64 suspends {{bytes.len}} machine forward(left:@[u8],right:@[u8],dead:@[u8])->u64 suspends {{{invocation}}}"
        );
        let pkg = source::lower(&[("loan.omg", &text)]).unwrap();
        let body = &pkg
            .machines
            .iter()
            .find(|m| m.declaration.path == "forward")
            .unwrap()
            .body;
        let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
        let [loan] = site.retained_loans.as_slice() else {
            panic!("only the selected view stays live")
        };
        assert_eq!(Parameter::Structural(loan.descriptor), body.parameters[1]);
        assert_eq!(loan.ty, body.places[loan.descriptor.index()].ty);
        assert_eq!(loan.access, omega_lowering::ReferenceAccess::Shared);
        assert!(loan.needed_by_callee && !loan.needed_after_completion);
        assert!(site.activation.is_empty());
        assert!(matches!(
            (tail, site.call, site.continuation),
            (true, CallSite::Edge(_), CompletionContinuation::Return) | (false, CallSite::Operation(_), CompletionContinuation::Terminator(_))
        ));
    }
}

#[test]
fn suspension_before_a_choice_retains_its_inputs_and_afterward_retains_only_its_result() {
    let text = "machine wait() suspends {} machine inspect(view:@[u8])->u64 suspends{view.len} machine forward(flag:bool,left:@[u8],right:@[u8],dead:@[u8])->u64 suspends {suspend wait(); let selected = match flag {true -> left, false -> right,}; suspend inspect(selected)}";
    let package = source::lower(&[("two_crossings.omg", text)]).unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap()
        .body;
    let [before, after] = body.suspension_sites.as_slice() else {
        panic!("one crossing on each side of the join")
    };
    let [
        Parameter::Scalar(flag),
        Parameter::Structural(left),
        Parameter::Structural(right),
        Parameter::Structural(_),
    ] = body.parameters[..]
    else {
        panic!("formals")
    };
    assert_eq!(
        before
            .activation
            .iter()
            .map(|row| (row.value, row.needed_by_callee, row.needed_after_completion))
            .collect::<Vec<_>>(),
        [(flag, false, true)]
    );
    assert_eq!(
        before
            .retained_loans
            .iter()
            .map(|loan| (loan.descriptor, loan.needed_by_callee, loan.needed_after_completion))
            .collect::<Vec<_>>(),
        [(left, false, true), (right, false, true)]
    );
    let [joined] = after.retained_loans.as_slice() else {
        panic!("one selected descriptor")
    };
    let CallSite::Operation(call_id) = after.call else { panic!("ordinary call") };
    let OperationKind::Call { arguments, .. } = &body.operations[call_id.index()].kind else {
        panic!("selected call")
    };
    assert_eq!(arguments.as_slice(), [omega_lowering::CallArgument::CopyWhole(joined.descriptor)]);
    assert!(matches!(
        body.places[joined.descriptor.index()].origin,
        omega_lowering::PlaceOrigin::BlockParameter(_)
    ));
    assert!(joined.needed_by_callee && !joined.needed_after_completion);
    assert!(after.activation.is_empty());
}

#[test]
fn a_join_can_forward_the_result_of_a_completed_local_call() {
    let text = "machine relay(p:@[u8])->@[u8]{p} machine inspect(bits:@[u8]) suspends{} machine forward(flag:bool,left:@[u8],other:@[u8]) suspends{let right=relay(other); let chosen=match flag{true->left,false->right,}; suspend inspect(chosen)}";
    let pkg = source::lower(&[("joined.omg", text)]).unwrap();
    let body = &pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap()
        .body;
    let [site] = body.suspension_sites.as_slice() else {
        panic!("one parked call")
    };
    let [loan] = site.retained_loans.as_slice() else {
        panic!("one selected view")
    };
    let omega_lowering::PlaceOrigin::BlockParameter(join) = body.places[loan.descriptor.index()].origin else {
        panic!("parked view must be the join parameter")
    };
    let [
        Parameter::Scalar(_),
        Parameter::Structural(left),
        Parameter::Structural(other),
    ] = body.parameters[..]
    else {
        panic!("caller formals")
    };
    let completed = body
        .operations
        .iter()
        .find(|op| matches!(&op.kind,OperationKind::Call{arguments,..} if arguments.as_slice()==[omega_lowering::CallArgument::CopyWhole(other)]))
        .unwrap();
    let OperationResult::Structural(right) = completed.result else {
        panic!("completed view")
    };
    let arrivals: Vec<_> = body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            omega_lowering::BlockTerminator::Jump(edge) if edge.target == join => Some(edge.structural_arguments.as_slice()),
            _ => None,
        })
        .collect();
    assert_eq!(arrivals.len(), 2);
    for input in [left, right] {
        assert!(arrivals.contains(&[omega_lowering::CallArgument::CopyWhole(input)].as_slice()));
    }
}

#[test]
fn a_boundary_view_has_no_local_return_to_identify_its_backing() {
    let text = "boundary trait Device {machine view(p:@[u8])->@[u8] reaches Device crashes none;} machine relay(p:@[u8])->@[u8] reaches Device {Device::view(p)} machine inspect(bits:@[u8]) suspends{} machine forward(flag:bool,left:@[u8],other:@[u8]) suspends reaches Device {let right=relay(other); let chosen=match flag{true->left,false->right,}; suspend inspect(chosen)}";
    let err = source::refuse(
        &[("joined.omg", text)],
        "joined.omg",
        source::unique_span(text, "inspect(chosen)"),
        "suspension needs the exact backing and carry evidence of the live structural place",
    );
    assert!(
        err.site
            .span
            .slice(text.as_bytes())
            .unwrap()
            .ends_with(b"inspect(chosen)")
    );
}

#[test]
fn a_closed_callback_keeps_its_declared_ceiling_while_only_its_selected_effect_parks() {
    let text = "public data SuspendingStep=machine(x:u8)->u8 suspends; public machine apply<const F:SuspendingStep>(n:u8)->u8 suspends{suspend F(n)} machine quiet(n:u8)->u8{n} machine park(n:u8)->u8 suspends{n} machine direct(n:u8)->u8{apply::<quiet>(n)} machine delayed(n:u8)->u8 suspends{suspend apply::<park>(n)}";
    let pkg = source::lower(&[("closed.omg", text)]).unwrap();
    for (name, parks) in [("direct", false), ("delayed", true)] {
        let caller = pkg
            .machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap();
        let [call] = caller.body.operations.as_slice() else {
            panic!("one wrapper call")
        };
        let OperationKind::Call {
            callee: CallTarget::Machine(id),
            ..
        } = call.kind
        else {
            panic!("missing wrapper instance")
        };
        let wrapper = &pkg.machines[(id.get() - 1) as usize];
        assert_eq!(wrapper.contract.pause_bounds.suspend, Some(true));
        assert_eq!(wrapper.contract.realized_pause.suspend, parks);
        assert_eq!(wrapper.body.suspension_sites.len(), usize::from(parks));
        assert_eq!(caller.body.suspension_sites.len(), usize::from(parks));
        assert_eq!(wrapper.body.call_acknowledgements.len(), 1);
    }
}

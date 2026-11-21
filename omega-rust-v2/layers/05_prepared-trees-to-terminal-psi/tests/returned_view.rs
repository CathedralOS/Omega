#[path = "support/boundary_result.rs"]
mod boundary_result;
#[path = "support/source.rs"]
mod source;

use omega_lowering::{BlockTerminator, Parameter, ReturnEdge};
use psi_handoff_consumer::{recursive_view_backings, returned_view_backing};

#[test]
fn a_returned_view_keeps_the_actual_at_the_selected_formal_position() {
    for (element, returned, formal, backing) in [
        ("u8", "first", 0, 1),
        ("u8", "second", 1, 0),
        ("u16", "first", 0, 1),
        ("u16", "second", 1, 0),
    ] {
        let text = format!(
            "machine select(first:@[{element}],second:@[{element}])->@[{element}] {{ {returned} }}
             machine inspect(bytes:@[{element}])->u64 suspends {{ bytes.length }}
             machine forward(left:@[{element}],right:@[{element}])->u64 suspends {{
                 let chosen = select(right,left);
                 let count:u64 = suspend inspect(chosen);
                 count +% 1u64
             }}"
        );
        let mut package = source::lower(&[("returned_view.omg", &text)]).unwrap();
        let select_ty = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "select")
            .unwrap()
            .result_type
            .unwrap();
        let expected = if element == "u8" {
            omega_lowering::TypeShape::SharedByteView
        } else {
            omega_lowering::TypeShape::SharedElementView {
                element: omega_lowering::ViewElement::Scalar(omega_lowering::ScalarType::Integer { signed: false, width: 16 }),
            }
        };
        assert_eq!(package.types.get(select_ty), Some(&expected));
        let observe = |package: &_| returned_view_backing(package, "forward", "inspect", "select");
        assert_eq!(observe(&package), Ok((formal, backing)));

        // Keep the two actuals and the retained result unchanged. Only replace
        // the selected formal on the returning callee's normal edge.
        let select = package
            .machines
            .iter_mut()
            .find(|machine| machine.declaration.path == "select")
            .unwrap();
        let Parameter::Structural(other) = select.body.parameters[1 - formal] else {
            panic!("expected the other view formal");
        };
        let BlockTerminator::Return {
            result: ReturnEdge::Structural { source, .. },
            ..
        } = &mut select.body.blocks[0].terminator
        else {
            panic!("expected the normal structural return");
        };
        *source = other;
        assert_eq!(observe(&package), Ok((1 - formal, 1 - backing)));
    }
}

#[test]
fn only_an_actual_that_can_supply_the_return_needs_backing_at_the_later_crossing() {
    for selected in ["known", "external"] {
        let text = format!(
            "boundary trait Device{{machine view(p:@[u8])->@[u8] reaches Device crashes none;}} machine pick(known:@[u8],external:@[u8])->@[u8] {{{selected}}} machine inspect(bytes:@[u8]) suspends {{}} machine forward(input:@[u8]) suspends reaches Device {{let external=Device::view(input); let selected=pick(input,external); suspend inspect(selected)}}"
        );
        use boundary_result::*;
        use omega_lowering::{CallTarget, ValueOperand::CopyWhole};
        let pkg = source::lower(&[("selected_origin.omg", &text)]).unwrap();
        let body = &machine(&pkg, "forward").body;
        let pick = machine(&pkg, "pick");
        let (target, actuals) = producing_call(body, parked_view(&pkg, body));
        assert_eq!(target, CallTarget::Machine(pick.id));
        let [CopyWhole(input), CopyWhole(external)] = actuals else { panic!("ordered pick operands") };
        assert_eq!(body.parameters, [Parameter::Structural(*input)]);
        let anchor = lifetime_actual(&pkg, body, *external);
        let BlockTerminator::Return { result: ReturnEdge::Structural { source, .. }, .. } = pick.body.blocks[0].terminator else { panic!("normal return") };
        let formal = pick.body.parameters.iter().position(|p| *p == Parameter::Structural(source)).unwrap();
        if selected == "external" {
            spec_corpus::observe_handoff!(psi "psi-output/boundary-view-result", assert_eq!(
                (formal, &actuals[formal], anchor), (1, &CopyWhole(*external), (0, *input))
            ));
        } else {
            assert_eq!((formal, &actuals[formal]), (0, &CopyWhole(*input)));
        }
    }
}

#[test]
fn a_caller_chosen_boundary_result_is_not_an_input_relative_loan() {
    // `Device::view()` takes no reference, so each caller chooses its result
    // lifetime (wiki/spec/language/lifetimes.md#caller-chosen-result-lifetimes)
    // and only the admitted provider backs its bytes
    // (wiki/spec/build/foreign_storage.md#provider-owned-views-and-completion).
    // The two programs differ in one input: pick returns the caller's `known`
    // view or the provider's `external` one. Psi publishes the caller-chosen
    // lifetime and keeps the completed pick at the crossing. The Psi-only
    // reader names input 0 for `known`, and for `external` the boundary
    // invocation with no input.
    use boundary_result::{machine, parked_view, producing_call};
    use omega_lowering::{BoundaryResultLifetime::CallerChosen, CallSite, CallTarget, MemberRealization, PlaceOrigin, ValueOperand::CopyWhole};
    use psi_handoff_consumer::{ProviderInvocation, ViewSources, parked_view_sources};
    for selected in ["known", "external"] {
        let text = format!(
            "boundary trait Device{{machine view()->@[u8] reaches Device crashes none;}} machine pick(known:@[u8],external:@[u8])->@[u8] {{{selected}}} machine inspect(bytes:@[u8]) suspends {{}} machine forward(input:@[u8]) suspends reaches Device {{let external=Device::view(); let selected=pick(input,external); suspend inspect(selected)}}"
        );
        let files = [("no_anchor.omg", text.as_str())];
        let pkg = source::lower(&files).unwrap();
        let (forward, pick) = (machine(&pkg, "forward"), machine(&pkg, "pick"));
        let [selection] = pkg.member_calls.as_slice() else {
            panic!("one boundary member call")
        };
        let retained = parked_view(&pkg, &forward.body);
        let (target, actuals) = producing_call(&forward.body, retained);
        let [CopyWhole(input), CopyWhole(external)] = actuals else {
            panic!("ordered pick operands")
        };
        let (boundary, boundary_actuals) = producing_call(&forward.body, *external);
        let PlaceOrigin::Operation(view_call) = forward.body.places[external.index()].origin else {
            panic!("the provider's result is a completed call")
        };
        let sources = parked_view_sources(&pkg, "forward");
        if selected == "external" {
            spec_corpus::observe_handoff!(psi "psi-output/caller-chosen-boundary-view", assert_eq!(
                (
                    (selection.requirement.declaration.path.as_str(), &selection.target, selection.boundary_result_lifetime),
                    (target, boundary, boundary_actuals),
                    (forward.body.parameters.as_slice(), forward.body.lifetime_bindings.is_empty()),
                    sources,
                ),
                (
                    ("Device::view", &MemberRealization::Boundary { direction: None }, Some(CallerChosen)),
                    (CallTarget::Machine(pick.id), CallTarget::Member(0), [].as_slice()),
                    ([Parameter::Structural(*input)].as_slice(), true),
                    Ok(ViewSources {
                        inputs: vec![],
                        providers: vec![ProviderInvocation {
                            machine: forward.id,
                            call: CallSite::Operation(view_call),
                            member: 0,
                            lifetime: CallerChosen,
                        }],
                    }),
                )
            ));
        } else {
            assert_eq!(selection.boundary_result_lifetime, Some(CallerChosen));
            assert_eq!(sources, Ok(ViewSources { inputs: vec![0], providers: vec![] }));
            assert_eq!(returned_view_backing(&pkg, "forward", "inspect", "pick"), Ok((0, 0)));
        }
    }
}

#[test]
fn the_reader_names_the_selected_provider_invocation() {
    // Two zero-input boundary members. Choosing which local pick receives
    // changes the named member and invocation; rewriting only pick's
    // published operand moves it again, so the reader follows Psi rather than
    // the source. Clearing the published lifetime makes it refuse.
    use boundary_result::machine;
    use omega_lowering::{BoundaryResultLifetime::CallerChosen, CallArgument, CallSite, CallTarget, OperationId, OperationKind, OperationResult, PlaceId};
    use psi_handoff_consumer::{ProviderInvocation, ViewSources, parked_view_sources};
    let local = |pkg: &omega_lowering::LoweredPackage, path: &str| -> (OperationId, PlaceId, usize) {
        let forward = machine(pkg, "forward");
        forward
            .body
            .operations
            .iter()
            .find_map(|op| match (&op.kind, op.result) {
                (OperationKind::Call { callee: CallTarget::Member(slot), .. }, OperationResult::Structural(place))
                    if pkg.member_calls[*slot].requirement.declaration.path == path =>
                {
                    Some((op.id, place, *slot))
                }
                _ => None,
            })
            .unwrap()
    };
    for (chosen, path, other) in [("a", "Device::view", "Device::read"), ("b", "Device::read", "Device::view")] {
        let text = format!(
            "boundary trait Device{{machine view()->@[u8] reaches Device crashes none; machine read()->@[u8] reaches Device crashes none;}} machine pick(known:@[u8],external:@[u8])->@[u8] {{external}} machine inspect(bytes:@[u8]) suspends {{}} machine forward(input:@[u8]) suspends reaches Device {{let a=Device::view(); let b=Device::read(); let selected=pick(input,{chosen}); suspend inspect(selected)}}"
        );
        let mut pkg = source::lower(&[("providers.omg", &text)]).unwrap();
        let forward = machine(&pkg, "forward").id;
        let named = |pkg: &omega_lowering::LoweredPackage, path: &str| {
            let (call, _, member) = local(pkg, path);
            Ok(ViewSources {
                inputs: vec![],
                providers: vec![ProviderInvocation {
                    machine: forward,
                    call: CallSite::Operation(call),
                    member,
                    lifetime: CallerChosen,
                }],
            })
        };
        assert_eq!(parked_view_sources(&pkg, "forward"), named(&pkg, path), "{chosen}");

        let (_, replacement, _) = local(&pkg, other);
        let pick = machine(&pkg, "pick").id;
        let forward_body = &mut pkg.machines.iter_mut().find(|m| m.declaration.path == "forward").unwrap().body;
        let arguments = forward_body
            .operations
            .iter_mut()
            .find_map(|op| match &mut op.kind {
                OperationKind::Call { callee: CallTarget::Machine(id), arguments, .. } if *id == pick => Some(arguments),
                _ => None,
            })
            .unwrap();
        arguments[1] = CallArgument::CopyWhole(replacement);
        assert_eq!(parked_view_sources(&pkg, "forward"), named(&pkg, other), "{chosen} rewritten");

        let (_, _, member) = local(&pkg, other);
        pkg.member_calls[member].boundary_result_lifetime = None;
        assert_eq!(parked_view_sources(&pkg, "forward"), Err("boundary view result has no published lifetime relation"));
    }
}

#[test]
fn a_boundary_lifetime_anchor_is_reported_apart_from_the_provider() {
    // The input-anchored twin of the caller-chosen result: `Device::view(input)`
    // lives as long as `input`, but its provider still supplies the bytes. The
    // reader names the anchor as an input and the invocation as the provider.
    use boundary_result::{machine, parked_view};
    use omega_lowering::{BoundaryResultLifetime, CallSite, OperationKind};
    use psi_handoff_consumer::{ProviderInvocation, ViewSources, parked_view_sources};
    let text = "boundary trait Device{machine view(p:@[u8])->@[u8] reaches Device crashes none;} machine pick(known:@[u8],external:@[u8])->@[u8] {external} machine inspect(bytes:@[u8]) suspends {} machine forward(input:@[u8]) suspends reaches Device {let external=Device::view(input); let selected=pick(input,external); suspend inspect(selected)}";
    let pkg = source::lower(&[("selected_origin.omg", text)]).unwrap();
    let forward = machine(&pkg, "forward");
    parked_view(&pkg, &forward.body);
    let view_call = forward
        .body
        .operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::Call { callee: omega_lowering::CallTarget::Member(0), .. }))
        .unwrap()
        .id;
    assert_eq!(
        parked_view_sources(&pkg, "forward"),
        Ok(ViewSources {
            inputs: vec![0],
            providers: vec![ProviderInvocation {
                machine: forward.id,
                call: CallSite::Operation(view_call),
                member: 0,
                lifetime: BoundaryResultLifetime::Input(0),
            }],
        })
    );
}

#[test]
fn a_boundary_view_with_two_reference_inputs_publishes_no_relation() {
    // Two reference inputs leave the elided relation ambiguous
    // (wiki/spec/language/lifetimes.md#returned-views). Neither input is the
    // anchor and the result is not caller-chosen, so L5 publishes no relation.
    // A crossing that retains such a result keeps the same refusal as before.
    let text = "boundary trait Device{machine view(p:@[u8],q:@[u8])->@[u8] reaches Device crashes none;} machine forward(input:@[u8])->@[u8] reaches Device {Device::view(input,input)}";
    let pkg = source::lower(&[("two_views.omg", text)]).unwrap();
    let [selection] = pkg.member_calls.as_slice() else {
        panic!("one boundary member call")
    };
    assert_eq!(selection.boundary_result_lifetime, None);
}

#[test]
fn a_boundary_lifetime_anchor_counts_scalar_positions_and_preserves_the_call_occurrence() {
    use boundary_result::*;
    for (selected, index) in [("a", 1), ("b", 2)] {
        let text = format!("boundary trait Device{{machine view(n:u8,p:@[u16])->@[u8] reaches Device crashes none;}} machine inspect(bytes:@[u8]) suspends {{}} machine forward(n:u8,left:@[u16],right:@[u16]) suspends reaches Device {{let a=Device::view(n,left); let b=Device::view(n,right); suspend inspect({selected})}}");
        let pkg = source::lower(&[("occurrence.omg", &text)]).unwrap();
        let body = &machine(&pkg, "forward").body;
        let result = parked_view(&pkg, body);
        let (position, actual) = lifetime_actual(&pkg, body, result);
        assert_ne!(body.places[actual.index()].ty, body.places[result.index()].ty);
        assert_eq!((position, Parameter::Structural(actual)), (1, body.parameters[index]));
    }
}

#[test]
fn a_returned_view_carries_the_producing_calls_unproved_precondition() {
    let text = "machine pick(p:@[u8])->@[u8] requires false {p} machine inspect(bytes:@[u8]) suspends {} machine forward(input:@[u8]) suspends {let selected=pick(input); suspend inspect(selected)}";
    let pkg = source::lower(&[("guarded_view.omg", text)]).unwrap();
    let forward = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap();
    let pick = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "pick")
        .unwrap();
    let [site] = forward.body.suspension_sites.as_slice() else {
        panic!("one crossing")
    };
    let [loan] = site.retained_loans.as_slice() else {
        panic!("one returned view")
    };
    let omega_lowering::PlaceOrigin::Operation(id) = forward.body.places[loan.descriptor.index()].origin else {
        panic!("completed pick result")
    };
    let omega_lowering::OperationKind::Call { callee, requirements, .. } = &forward.body.operations[id.index()].kind else {
        panic!("producing call")
    };
    assert_eq!(*callee, omega_lowering::CallTarget::Machine(pick.id));
    let [required] = pick.contract.requires.as_slice() else {
        panic!("one entry condition")
    };
    assert_eq!(required.predicate, [omega_lowering::PredicateStep::Boolean(false)]);
    assert_eq!(
        requirements.as_slice(),
        [omega_lowering::CallRequirement {
            site: omega_lowering::CallSite::Operation(id),
            clause: 0
        }]
    );
}

#[test]
fn a_local_conformance_return_uses_the_selected_implementations_body() {
    let text = "trait Views {machine view(x:@[u8],y:@[u8])->@[u8];} machine second(x:@[u8],y:@[u8])->@[u8]{y} Local satisfies Views{Views::view=second;} machine relay<const E:Views>(x:@[u8],y:@[u8])->@[u8]{E::view(x,y)} machine inspect(v:@[u8]) suspends{} machine forward(a:@[u8],b:@[u8]) suspends{let selected=relay::<Local>(a,b); suspend inspect(selected)}";
    let pkg = source::lower(&[("member_view.omg", text)]).unwrap();
    let second = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "second")
        .unwrap();
    let relay = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "relay")
        .unwrap();
    let [member] = relay.body.operations.as_slice() else {
        panic!("one member call")
    };
    let omega_lowering::OperationKind::Call {
        callee: omega_lowering::CallTarget::Member(slot),
        ..
    } = member.kind
    else {
        panic!("selected member")
    };
    assert!(matches!(pkg.member_calls[slot].target,omega_lowering::MemberRealization::Conformance{machine,..} if machine==second.id));
    let Parameter::Structural(y) = second.body.parameters[1] else {
        panic!("second formal")
    };
    assert!(matches!(second.body.blocks[0].terminator,BlockTerminator::Return{result:ReturnEdge::Structural{source,..},..} if source==y));
    let forward = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap();
    let [site] = forward.body.suspension_sites.as_slice() else {
        panic!("one parked call")
    };
    let [loan] = site.retained_loans.as_slice() else { panic!("one loan") };
    let omega_lowering::PlaceOrigin::Operation(id) = forward.body.places[loan.descriptor.index()].origin else {
        panic!("completed call")
    };
    assert!(
        matches!(forward.body.operations[id.index()].kind,omega_lowering::OperationKind::Call{callee:omega_lowering::CallTarget::Machine(target),..} if target==relay.id)
    );
}

/// A recursive callee's view result is backed by the actuals at the formals its
/// returns reach. Each case lists the caller's incoming formals that back it.
const RECURSIVE_VIEWS: [(&str, &str, &[usize]); 5] = [
    (
        "cycle.omg",
        "machine cycle(v:@[u8])->@[u8]{cycle(v)} machine inspect(v:@[u8]) suspends{} machine forward(input:@[u8]) suspends {let selected=cycle(input); suspend inspect(selected)}",
        &[0],
    ),
    (
        "swap.omg",
        "machine cycle(a:@[u8],b:@[u8])->@[u8]{cycle(b,a)} machine inspect(v:@[u8]) suspends{} machine forward(x:@[u8],y:@[u8]) suspends {let selected=cycle(x,y); suspend inspect(selected)}",
        &[0, 1],
    ),
    (
        "narrow.omg",
        "machine cycle(a:@[u8],b:@[u8])->@[u8]{cycle(b,b)} machine inspect(v:@[u8]) suspends{} machine forward(x:@[u8],y:@[u8]) suspends {let selected=cycle(x,y); suspend inspect(selected)}",
        &[1],
    ),
    (
        "terminal.omg",
        "machine cycle(a:@[u8],b:@[u8])->@[u8]{transition { _ -> cycle(b,b), }} machine inspect(v:@[u8]) suspends{} machine forward(x:@[u8],y:@[u8]) suspends {let selected=cycle(x,y); suspend inspect(selected)}",
        &[1],
    ),
    (
        "base.omg",
        "machine cycle(v:@[u8],flag:bool)->@[u8] {transition flag {true -> v, false -> cycle(v,flag),}} machine inspect(v:@[u8]) suspends{} machine forward(input:@[u8],flag:bool) suspends {let selected=cycle(input,flag); suspend inspect(selected)}",
        &[0],
    ),
];

#[test]
fn a_recursive_view_result_retains_the_completed_call_at_the_suspension() {
    for (file, text, backings) in RECURSIVE_VIEWS {
        let package = source::lower(&[(file, text)]).unwrap();
        let machine = |path| {
            package
                .machines
                .iter()
                .find(|m| m.declaration.path == path)
                .unwrap()
        };
        let (cycle, forward, inspect) = (machine("cycle"), machine("forward"), machine("inspect"));
        let [completed, parked] = forward.body.operations.as_slice() else {
            panic!("{file}: the recursive call completes before inspect parks")
        };
        let [site] = forward.body.suspension_sites.as_slice() else {
            panic!("{file}: only inspect parks")
        };
        let [loan] = site.retained_loans.as_slice() else {
            panic!("{file}: one retained view")
        };
        let omega_lowering::OperationKind::Call { callee, .. } = &completed.kind else {
            panic!("{file}: completed call")
        };
        spec_corpus::observe_handoff!(
            psi "psi-output/recursive-view-backing",
            assert_eq!(
                (
                    forward.body.places[loan.descriptor.index()].origin,
                    *callee,
                    completed.result,
                    site.call,
                    site.callee,
                    recursive_view_backings(&package, "forward", "cycle"),
                ),
                (
                    omega_lowering::PlaceOrigin::Operation(completed.id),
                    omega_lowering::CallTarget::Machine(cycle.id),
                    omega_lowering::OperationResult::Structural(loan.descriptor),
                    omega_lowering::CallSite::Operation(parked.id),
                    omega_lowering::CallTarget::Machine(inspect.id),
                    Ok(backings.to_vec()),
                ),
                "{file}",
            ),
        );
        assert_eq!(
            (loan.access, loan.needed_by_callee, loan.needed_after_completion),
            (omega_lowering::ReferenceAccess::Shared, true, false)
        );
    }
}

#[test]
fn the_consumer_reads_the_recursive_actuals_from_the_published_call() {
    // Only replace the recursive call's actuals in Psi. The traced backing
    // follows them, so the consumer reads the call rather than the source.
    let (file, text, backings) = RECURSIVE_VIEWS[2];
    let mut package = source::lower(&[(file, text)]).unwrap();
    assert_eq!(recursive_view_backings(&package, "forward", "cycle"), Ok(backings.to_vec()));
    let cycle = package
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == "cycle")
        .unwrap();
    let Parameter::Structural(a) = cycle.body.parameters[0] else {
        panic!("first formal")
    };
    let [call] = cycle.body.operations.as_mut_slice() else {
        panic!("one recursive call")
    };
    let omega_lowering::OperationKind::Call { arguments, .. } = &mut call.kind else {
        panic!("recursive call")
    };
    *arguments = vec![omega_lowering::CallArgument::CopyWhole(a); 2];
    assert_eq!(recursive_view_backings(&package, "forward", "cycle"), Ok(vec![0]));
}

/// One arm of the recursion returns the boundary completion `Device::view(v)`;
/// the other calls `cycle` again on the same input. This is the exact program
/// that stopped before the boundary selection recorded its lifetime input.
const RELAYED_BOUNDARY: &str = "boundary trait Device{machine view(p:@[u8])->@[u8] reaches Device crashes none;} machine cycle(v:@[u8],flag:bool)->@[u8] reaches Device {transition flag {true -> Device::view(v), false -> cycle(v,flag),}} machine inspect(v:@[u8]) suspends{} machine forward(input:@[u8],flag:bool) suspends reaches Device {let selected=cycle(input,flag); suspend inspect(selected)}";

#[test]
fn a_recursive_relay_does_not_supply_a_boundary_views_backing() {
    // Psi publishes the boundary invocation, the recursive back edge and the
    // parked loan of the completed relay. The applied member names operand 0 as
    // the result's lifetime anchor; that is a validity relation, not a claim
    // that `v` holds the provider's bytes
    // (wiki/spec/language/lifetimes.md#returned-views). So no caller input
    // stands in for the provider backing: the retained loan names the relay's
    // result, forward binds no lifetime, and the Psi-only reader of local
    // returns declines the boundary arm instead of naming `input`.
    use boundary_result::{machine, parked_view, producing_call};
    use omega_lowering::{
        CallSite, CallTarget, MemberRealization, ParameterType, PlaceOrigin, SuccessorEdge,
        ValueOperand::{CopyWhole, Scalar},
    };
    let text = RELAYED_BOUNDARY;
    let pkg = source::lower(&[("relayed_boundary.omg", text)]).unwrap();
    let (cycle, forward, inspect) = (machine(&pkg, "cycle"), machine(&pkg, "forward"), machine(&pkg, "inspect"));

    // cycle's two arms, read from the transition's successor blocks.
    let [Parameter::Structural(v), Parameter::Scalar(flag)] = cycle.body.parameters[..] else {
        panic!("cycle(v, flag)")
    };
    assert!(cycle.body.operations.is_empty(), "both arms are terminal calls");
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &cycle.body.blocks[0].terminator
    else {
        panic!("transition on flag")
    };
    let arm = |edge: &SuccessorEdge| {
        assert!(edge.structural_arguments.is_empty() && edge.arguments.is_empty());
        match &cycle.body.blocks[edge.target.get() - 1].terminator {
            BlockTerminator::TerminalCall {
                callee, arguments, cleanup, ..
            } if cleanup.is_empty() => (*callee, arguments.clone()),
            other => panic!("cleanup-free terminal call expected: {other:?}"),
        }
    };
    let [selection] = pkg.member_calls.as_slice() else {
        panic!("one boundary member call")
    };
    assert_eq!(selection.requirement.declaration.path, "Device::view");
    assert_eq!(
        (selection.result, selection.parameters.as_slice()),
        (cycle.result_type, [ParameterType::Structural(cycle.parameter_types[0])].as_slice())
    );

    // forward parks the completed relay, not its incoming view.
    let [Parameter::Structural(input), Parameter::Scalar(selector)] = forward.body.parameters[..] else {
        panic!("forward(input, flag)")
    };
    let retained = parked_view(&pkg, &forward.body);
    let [relay, parked] = forward.body.operations.as_slice() else {
        panic!("the relay completes before inspect parks")
    };
    let [site] = forward.body.suspension_sites.as_slice() else {
        panic!("only inspect parks")
    };
    let (target, actuals) = producing_call(&forward.body, retained);
    spec_corpus::observe_handoff!(psi "psi-output/recursive-boundary-view-relay", assert_eq!(
        (
            (*condition, arm(when_true), arm(when_false)),
            (&selection.target, selection.boundary_result_lifetime),
            (forward.body.places[retained.index()].origin, target, actuals),
            (site.call, site.callee, site.activation.is_empty(), site.retained_owners.is_empty()),
            (retained == input, forward.body.lifetime_bindings.is_empty()),
            recursive_view_backings(&pkg, "forward", "cycle"),
        ),
        (
            (
                flag,
                (CallTarget::Member(0), vec![CopyWhole(v)]),
                (CallTarget::Machine(cycle.id), vec![CopyWhole(v), Scalar(flag)]),
            ),
            (&MemberRealization::Boundary { direction: None }, Some(omega_lowering::BoundaryResultLifetime::Input(0))),
            (
                PlaceOrigin::Operation(relay.id),
                CallTarget::Machine(cycle.id),
                [CopyWhole(input), Scalar(selector)].as_slice(),
            ),
            (CallSite::Operation(parked.id), CallTarget::Machine(inspect.id), true, true),
            (false, true),
            Err("unsupported view-returning terminator"),
        )
    ));
}

#[test]
fn a_recursive_relay_of_a_caller_chosen_boundary_view_retains_the_provider_result() {
    // The relay above with a zero-input `Device::view()`. No input carries its
    // result lifetime, so each caller chooses it
    // (wiki/spec/language/lifetimes.md#caller-chosen-result-lifetimes). The
    // selection publishes that, so lowering follows the boundary arm to the
    // provider and the recursive arm back through cycle. The parked loan
    // retains the completed relay. Its lifetime still follows `input` through
    // cycle's own elided signature, but the reader names the boundary
    // invocation inside cycle, not `input`, as what supplies the bytes.
    use boundary_result::{machine, parked_view, producing_call};
    use omega_lowering::{
        BoundaryResultLifetime, CallSite, CallTarget, PlaceOrigin, SuccessorEdge,
        ValueOperand::{CopyWhole, Scalar},
    };
    use psi_handoff_consumer::{ProviderInvocation, ViewSources, parked_view_sources};
    let text = "boundary trait Device{machine view()->@[u8] reaches Device crashes none;} machine cycle(v:@[u8],flag:bool)->@[u8] reaches Device {transition flag {true -> Device::view(), false -> cycle(v,flag),}} machine inspect(v:@[u8]) suspends{} machine forward(input:@[u8],flag:bool) suspends reaches Device {let selected=cycle(input,flag); suspend inspect(selected)}";
    assert_eq!(
        text,
        RELAYED_BOUNDARY
            .replace("view(p:@[u8])", "view()")
            .replace("Device::view(v)", "Device::view()"),
        "the twin differs only in the boundary result's lifetime input"
    );
    let pkg = source::lower(&[("relayed_no_anchor.omg", text)]).unwrap();
    let (cycle, forward, inspect) = (machine(&pkg, "cycle"), machine(&pkg, "forward"), machine(&pkg, "inspect"));
    let [Parameter::Structural(v), Parameter::Scalar(flag)] = cycle.body.parameters[..] else {
        panic!("cycle(v, flag)")
    };
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &cycle.body.blocks[0].terminator
    else {
        panic!("transition on flag")
    };
    let arm = |edge: &SuccessorEdge| match &cycle.body.blocks[edge.target.get() - 1].terminator {
        BlockTerminator::TerminalCall {
            id, callee, arguments, cleanup, ..
        } if cleanup.is_empty() => (*id, *callee, arguments.clone()),
        other => panic!("cleanup-free terminal call expected: {other:?}"),
    };
    let (boundary_edge, boundary, boundary_actuals) = arm(when_true);
    let [selection] = pkg.member_calls.as_slice() else {
        panic!("one boundary member call")
    };
    let [Parameter::Structural(input), Parameter::Scalar(selector)] = forward.body.parameters[..] else {
        panic!("forward(input, flag)")
    };
    let retained = parked_view(&pkg, &forward.body);
    let [relay, parked] = forward.body.operations.as_slice() else {
        panic!("the relay completes before inspect parks")
    };
    let [site] = forward.body.suspension_sites.as_slice() else {
        panic!("only inspect parks")
    };
    let (target, actuals) = producing_call(&forward.body, retained);
    spec_corpus::observe_handoff!(psi "psi-output/recursive-caller-chosen-boundary-view", assert_eq!(
        (
            (*condition, (boundary, boundary_actuals), (arm(when_false).1, arm(when_false).2)),
            selection.boundary_result_lifetime,
            (forward.body.places[retained.index()].origin, target, actuals),
            (site.call, site.callee, site.activation.is_empty(), site.retained_owners.is_empty()),
            (retained == input, forward.body.lifetime_bindings.is_empty()),
            parked_view_sources(&pkg, "forward"),
        ),
        (
            (
                flag,
                (CallTarget::Member(0), vec![]),
                (CallTarget::Machine(cycle.id), vec![CopyWhole(v), Scalar(flag)]),
            ),
            Some(BoundaryResultLifetime::CallerChosen),
            (
                PlaceOrigin::Operation(relay.id),
                CallTarget::Machine(cycle.id),
                [CopyWhole(input), Scalar(selector)].as_slice(),
            ),
            (CallSite::Operation(parked.id), CallTarget::Machine(inspect.id), true, true),
            (false, true),
            Ok(ViewSources {
                inputs: vec![0],
                providers: vec![ProviderInvocation {
                    machine: cycle.id,
                    call: CallSite::Edge(boundary_edge),
                    member: 0,
                    lifetime: BoundaryResultLifetime::CallerChosen,
                }],
            }),
        )
    ));

    // The anchored relay reaches the same kind of invocation, whose lifetime follows `v`.
    let anchored = source::lower(&[("relayed_boundary.omg", RELAYED_BOUNDARY)]).unwrap();
    let anchored_cycle = machine(&anchored, "cycle");
    let BlockTerminator::Conditional { when_true, .. } = &anchored_cycle.body.blocks[0].terminator else {
        panic!("transition on flag")
    };
    let BlockTerminator::TerminalCall { id, .. } = anchored_cycle.body.blocks[when_true.target.get() - 1].terminator else {
        panic!("boundary arm")
    };
    assert_eq!(
        parked_view_sources(&anchored, "forward"),
        Ok(ViewSources {
            inputs: vec![0],
            providers: vec![ProviderInvocation {
                machine: anchored_cycle.id,
                call: CallSite::Edge(id),
                member: 0,
                lifetime: BoundaryResultLifetime::Input(0),
            }],
        })
    );
}

#[test]
fn a_recursion_without_a_view_input_relays_only_the_provider() {
    // cycle has no reference input, so its own result lifetime is caller-chosen
    // too. Its summary starts with no formal, the boundary arm adds the
    // provider, and the parked view depends on no input of forward.
    use boundary_result::machine;
    use omega_lowering::{BoundaryResultLifetime::CallerChosen, CallSite};
    use psi_handoff_consumer::{ProviderInvocation, ViewSources, parked_view_sources};
    let text = "boundary trait Device{machine view()->@[u8] reaches Device crashes none;} machine cycle(flag:bool)->@[u8] reaches Device {transition flag {true -> Device::view(), false -> cycle(flag),}} machine inspect(v:@[u8]) suspends{} machine forward(flag:bool) suspends reaches Device {let selected=cycle(flag); suspend inspect(selected)}";
    let pkg = source::lower(&[("providers_only.omg", text)]).unwrap();
    let cycle = machine(&pkg, "cycle");
    let BlockTerminator::Conditional { when_true, .. } = &cycle.body.blocks[0].terminator else {
        panic!("transition on flag")
    };
    let BlockTerminator::TerminalCall { id, .. } = cycle.body.blocks[when_true.target.get() - 1].terminator else {
        panic!("boundary arm")
    };
    assert_eq!(
        parked_view_sources(&pkg, "forward"),
        Ok(ViewSources {
            inputs: vec![],
            providers: vec![ProviderInvocation {
                machine: cycle.id,
                call: CallSite::Edge(id),
                member: 0,
                lifetime: CallerChosen,
            }],
        })
    );
}

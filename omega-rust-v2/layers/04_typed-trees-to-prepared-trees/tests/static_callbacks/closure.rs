use super::resolve;
use omega_preparation::{
    CheckedPreparationEvidence, PreparationAnswers, PreparationFailure, PreparationReason, PreparationRound, PreparedValue, RefusalCause,
    prepare_source_package_diagnosed, prepare_source_package_diagnosed_with_inputs, prepare_typing_pass_with_cause,
};
use omega_typed::{BoundarySlot, CallUse, InvocationTarget, RequirementSlot, SlotDirection, StaticValueDemand, TypedTrees, ValueType};
use omega_typer::TypingProgress;

#[path = "../support/typed_packages.rs"]
mod typed_packages;

fn range(
    source: &str,
    occurrence: &str,
) -> (usize, usize) {
    let start = source.find(occurrence).unwrap();
    assert_eq!(source.rfind(occurrence), Some(start));
    (start, start + occurrence.len())
}

fn refused(
    failure: PreparationFailure,
    reason: PreparationReason,
    source: &str,
    occurrence: &str,
    text: &str,
) {
    assert!(matches!(failure, PreparationFailure::Preparation(_)), "{failure:?}");
    spec_corpus::assert_preparation_refusal!(
        failure,
        RefusalCause::Preparation(reason),
        "app",
        "app.omg",
        range(source, occurrence),
        format!("static evaluation cannot admit this invocation: {text}")
    );
}

#[test]
fn a_cycle_through_the_selected_static_formal_is_not_a_direct_recursive_component() {
    let source = "data Step=machine(n:u8)->u8;
        machine apply<const F:Step>(n:u8)->u8 {F(n)}
        machine step(n:u8)->u8 {n}
        machine loopback(n:u8)->u8 {match n == 0u8 {true -> n, false -> apply::<step>(n),}}
        const RESULT:u8=1u8 +% apply::<loopback>(0u8);";
    let control = prepare_source_package_diagnosed(&resolve(source)).unwrap();
    assert_eq!(
        control.constant(&control.typed().constants[0].declaration),
        Some(&PreparedValue::Integer(1.into()))
    );
    // The authored direct edge is still loopback -> apply. Only the selection
    // for F changes. The composed invokes row names targets, not back-edge sites.
    let cyclic = source.replace("apply::<step>", "apply::<loopback>");
    let package = resolve(&cyclic);
    let round = PreparationRound::begin(&package, &[], None).unwrap();
    let TypingProgress::Complete(trees) = round.typing() else {
        panic!("expected a closed call")
    };
    let row = trees
        .machine_effects
        .iter()
        .find(|row| row.machine.path == "loopback")
        .unwrap();
    assert!(row.invokes.iter().any(|target| match target {
        InvocationTarget::Declaration(slot) => slot.declaration == row.machine,
        InvocationTarget::AppliedMachine(selected) => *selected.machine == row.machine,
        _ => false,
    }));
    refused(
        prepare_source_package_diagnosed(&package).unwrap_err(),
        PreparationReason::CyclicCallbackWithoutTermination,
        &cyclic,
        "apply::<loopback>(0u8)",
        "a cyclic static callback dependency has no checked termination proof",
    );
}

#[test]
fn a_callback_reaching_an_unpromised_public_helper_reports_that_call() {
    let source = "data Step=machine(n:u8)->u8;
        machine apply<const F:Step>(n:u8)->u8 {F(n)}
        public machine helper(n:u8)->u8 {n}
        machine pick(n:u8)->u8 {helper(n)}
        const RESULT:u8=1u8 +% apply::<pick>(0u8);";
    let control = source.replace("->u8 {n}", "->u8 terminates {n}");
    let prepared = prepare_source_package_diagnosed(&resolve(&control)).unwrap();
    assert_eq!(
        prepared.constant(&prepared.typed().constants[0].declaration),
        Some(&PreparedValue::Integer(1.into()))
    );
    refused(
        prepare_source_package_diagnosed(&resolve(source)).unwrap_err(),
        PreparationReason::PublicMachineWithoutTermination,
        source,
        "helper(n)",
        "the selected public machine does not declare termination",
    );
}

#[test]
fn a_provider_edge_is_not_a_direct_local_termination_edge() {
    let source = "boundary trait Reading {machine read()->u8 reaches none crashes none terminates;}
        data Step=machine()->u8;
        machine apply<const F:Step>()->u8 {F()}
        machine pick()->u8 {Reading::read()}
        const RESULT:u8=1u8 +% apply::<pick>();";
    let input = resolve(source);
    let read = input
        .declarations
        .iter()
        .find(|decl| decl.path == "Reading::read")
        .unwrap();
    let slot = BoundarySlot {
        requirement: RequirementSlot {
            declaration: read.clone(),
            trait_arguments: vec![],
        },
        arguments: vec![],
        direction: SlotDirection::ProgramToProvider,
    };
    let failure = prepare_source_package_diagnosed_with_inputs(&input, &[slot], None).unwrap_err();
    refused(
        failure,
        PreparationReason::UnresolvedCallbackClosure,
        source,
        "Reading::read()",
        "the exact static callback has an unresolved call in its termination closure",
    );
}

#[test]
fn an_unresolved_callback_helper_reports_its_own_packages_call() {
    let library = "machine helper(n:u8)->u8 {n} public machine pick(n:u8)->u8 reaches none crashes none terminates {helper(n)}";
    let client = "data Step=machine(n:u8)->u8; machine apply<const F:Step>(n:u8)->u8 {F(n)} const RESULT:u8=apply::<geo\\pick>(42u8);";
    let progress = TypingProgress::Complete(typed_packages::typed(library, client));
    let prepare = |progress: &TypingProgress<'_>| prepare_typing_pass_with_cause(progress, &[], None, CheckedPreparationEvidence::default());
    let PreparationAnswers::Frozen { constants, .. } = prepare(&progress).unwrap().answers else {
        panic!("expected a frozen callback result")
    };
    let TypingProgress::Complete(trees) = progress else { unreachable!() };
    assert_eq!(constants[&trees.constants[0].declaration], PreparedValue::Integer(42.into()));
    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    assert!(facts.library.is_none());
    let pos = facts
        .signatures
        .iter()
        .position(|signature| signature.machine.path == "helper")
        .unwrap();
    let helper = facts.signatures.remove(pos);
    // A missing signature and a present signature naming a nonexistent body
    // both leave the edge unresolved. The call itself is a reliable site.
    for bad_node in [false, true] {
        if bad_node {
            let mut invalid = (*helper).clone();
            invalid.node.node = usize::MAX;
            facts.signatures.push(invalid.into());
        }
        let changed = TypingProgress::Complete(TypedTrees::new(resolved.clone(), facts));
        let refusal = prepare(&changed).unwrap_err();
        spec_corpus::assert_preparation_cause!(
            refusal,
            RefusalCause::Preparation(PreparationReason::UnresolvedCallbackClosure),
            "geometry",
            "geometry.omg",
            range(library, "helper(n)"),
            "static evaluation cannot admit this invocation: the exact static callback has an unresolved call in its termination closure"
        );
        let TypingProgress::Complete(trees) = changed else { unreachable!() };
        facts = trees.into_facts();
    }
}

#[test]
fn a_specialized_request_waits_for_its_selected_overload() {
    let source = "data Flag<const B:bool>[copy]{}
        machine enabled()->bool{true}
        machine pick(x:Flag<true>)->u64{2u64}
        machine pick(x:Flag<false>)->u64{3u64}
        data Size=machine()->u64 reaches none crashes none;
        machine two()->u64{pick(Flag::<enabled()>{})}
        data Word<const F:Size>{x:u<F()>;}
        machine use_it(x:Word<two>){}";
    let input = resolve(source);
    let round = PreparationRound::begin(&input, &[], None).unwrap();
    let TypingProgress::Pending(pending) = round.typing() else {
        panic!("the overload in two depends on enabled()")
    };
    let demands = pending.demands();
    assert!(
        demands
            .iter()
            .any(|d| matches!(d, StaticValueDemand::Closed(r) if r.expression.span.slice(source.as_bytes()) == Some(b"enabled()")))
    );
    assert!(
        !demands
            .iter()
            .any(|d| matches!(d, StaticValueDemand::Specialized { .. }))
    );
    assert!(
        !pending
            .trees()
            .calls
            .iter()
            .any(|call| call.machine.path == "pick")
    );
    assert!(
        !pending
            .trees()
            .machine_effects
            .iter()
            .any(|row| row.machine.path == "two")
    );
    let answers = round.evaluate(Default::default()).unwrap();
    let omega_preparation::PreparationStep::Evaluate(round) = round.advance(answers).unwrap() else {
        panic!("expected the specialized request")
    };
    let TypingProgress::Pending(pending) = round.typing() else {
        panic!("expected the specialized request")
    };
    let (request, arguments) = pending
        .demands()
        .iter()
        .find_map(|d| match d {
            StaticValueDemand::Specialized { request, arguments } if request.expression.span.slice(source.as_bytes()) == Some(b"F()") => {
                Some((request, arguments))
            }
            _ => None,
        })
        .unwrap();
    let trees = pending.trees();
    let [(word, 1)] = trees.static_request_scopes[&request.node].as_slice() else {
        panic!("one specialization binder")
    };
    assert_eq!(word.path, "Word");
    let [ValueType::MachineSelection(selected)] = arguments.as_slice() else {
        panic!("closed selection")
    };
    assert_eq!(selected.machine.path, "two");
    assert!(trees.indirect_calls.iter().any(|call| call.node == request.node
        && call.use_site == CallUse::StaticArgument
        && matches!(&call.entry, InvocationTarget::StaticParameter { owner, index: 0 } if owner == word)));
    assert!(
        trees
            .calls
            .iter()
            .any(|call| call.caller.as_ref() == Some(selected.machine.as_ref()) && call.machine.path == "pick")
    );
    assert!(
        trees
            .machine_effects
            .iter()
            .any(|row| row.machine == *selected.machine && row.crash_ceiling.as_deref() == Some(&[]))
    );
    let prepared = prepare_source_package_diagnosed(&input).unwrap();
    let [answer] = prepared.specialized_indices() else {
        panic!("expected one field width")
    };
    assert_eq!(answer.arguments, *arguments);
    assert_eq!(answer.value, omega_typed::StaticValue::Integer(2.into()));

    let direct = source.replace("x:u<F()>", "x:u<two()>");
    let prepared = prepare_source_package_diagnosed(&resolve(&direct)).unwrap();
    assert!(
        prepared
            .typed()
            .calls
            .iter()
            .any(|call| call.machine.path == "pick" && call.caller.as_ref() == Some(selected.machine.as_ref()))
    );

    // The overload now selected for two advertises a possible trap. Its
    // literal result cannot narrow that ceiling to satisfy Size.
    let risky = source.replace("Flag<true>)->u64{", "Flag<true>)->u64 crashes Trap {");
    spec_corpus::observe_handoff!(
        "closed-callback-crash-bound/consumer",
        refused(
            prepare_source_package_diagnosed(&resolve(&risky)).unwrap_err(),
            PreparationReason::ForwardedCallbackNeedsRefinement,
            &risky,
            "F()",
            "the exact static callback needs checked refinement of its callable contract",
        )
    );
}

#[test]
fn a_literal_callback_computes_the_field_type_but_cannot_supply_its_own_refinement() {
    let source = "data Size=machine()->u64 reaches none crashes none;
        machine two()->u64{2u64}
        data Word<const F:Size>{x:u<F()>;}
        machine read(w:Word<two>)->u<2>{w.x}";
    let prepared = prepare_source_package_diagnosed(&resolve(source)).unwrap();
    let [answer] = prepared.specialized_indices() else {
        panic!("expected one field width")
    };
    assert_eq!(answer.value, omega_typed::StaticValue::Integer(2.into()));
    let projection = prepared
        .typed()
        .expressions
        .iter()
        .find(|expr| expr.site.span.slice(source.as_bytes()) == Some(b"w.x"))
        .unwrap();
    spec_corpus::observe_handoff!(
        "specialized-callback-field/producer",
        assert_eq!(projection.ty, ValueType::Integer { signed: false, width: Some(2) }),
    );
    for restricted in [
        source.replace("two()->u64{", "two()->u64 requires false {"),
        source.replace("->u64 reaches", "->u64 requires true reaches"),
    ] {
        refused(
            prepare_source_package_diagnosed(&resolve(&restricted)).unwrap_err(),
            PreparationReason::CallbackNeedsRefinement,
            &restricted,
            "F()",
            "the exact static callback needs a checked refinement of its callable contract",
        );
    }
}

#[test]
fn a_bare_initializer_cannot_execute_another_owners_callback_recipe() {
    let client = "data Size=machine()->u64 reaches none crashes none;
        data Word<const F:Size>{x:u<F()>;}
        machine two()->u64{2u64} const WIDTH:u64=two( );";
    let progress = TypingProgress::Complete(typed_packages::typed("", client));
    let prepare = |progress: &TypingProgress<'_>| prepare_typing_pass_with_cause(progress, &[], None, Default::default());
    let PreparationAnswers::Frozen { constants, .. } = prepare(&progress).unwrap().answers else {
        panic!("expected a closed width")
    };
    let TypingProgress::Complete(trees) = progress else { unreachable!() };
    assert_eq!(constants[&trees.constants[0].declaration], PreparedValue::Integer(2.into()));
    let resolved = trees.resolved().clone();
    let mut facts = trees.into_facts();
    let [invalid] = facts.indirect_calls.as_mut_slice() else {
        panic!("expected one field recipe")
    };
    let direct = facts.calls.pop().unwrap();
    assert_eq!(direct.node, facts.constants[0].initializer.node);
    // Misdirect the initializer's callee to the field's formal. Its call
    // occurrence and type still agree, but WIDTH supplies no such application.
    invalid.node = direct.node;
    invalid.site = direct.site;
    invalid.callee = invalid.site.clone();
    let (start, _) = range(client, "two( )");
    invalid.callee.span = omega_lexer::SourceSpan::new(start, start + "two".len());
    let changed = TypingProgress::Complete(TypedTrees::new(resolved, facts));
    spec_corpus::assert_preparation_cause!(
        prepare(&changed).unwrap_err(),
        RefusalCause::Preparation(PreparationReason::CallbackWithoutCallerApplication),
        "client",
        "client.omg",
        range(client, "two( )"),
        "static evaluation cannot admit this invocation: the static callback has no closed caller application"
    );
}

#[test]
fn the_same_field_recipe_can_run_for_one_nested_selection_while_another_waits() {
    use omega_preparation::{PreparationProgress, PreparationStep};

    let source = "data Flag<const B:bool>[copy]{}
        machine enabled()->bool{true}
        machine pick(x:Flag<true>)->u64{2u64}
        machine pick(x:Flag<false>)->u64{3u64}
        data Size=machine()->u64 reaches none crashes none;
        machine two()->u64{pick(Flag::<enabled()>{})}
        machine five()->u64{5u64}
        machine relay<const H:Size>()->u64{H()}
        data Word<const F:Size,const G:Size>{x:u<F()>;}
        machine first(x:Word<relay::<two>,five>){}
        machine second(x:Word<relay::<five>,two>){}
        machine third(x:Word<relay::<five>,five>){}";
    for (slot, widths) in [("F()", [2, 5, 5]), ("G()", [5, 2, 5])] {
        let source = source.replace("x:u<F()>", &format!("x:u<{slot}>"));
        let package = resolve(&source);
        let mut round = PreparationRound::begin(&package, &[], None).unwrap();
        let TypingProgress::Pending(pending) = round.typing() else {
            panic!("expected separate width demands")
        };
        let ready: Vec<_> = pending
            .demands()
            .iter()
            .filter_map(|d| match d {
                StaticValueDemand::Specialized { request, arguments } => Some((request.clone(), arguments.clone())),
                StaticValueDemand::Closed(_) => None,
            })
            .collect();
        let [(request, a), (other_request, b)] = ready.as_slice() else { panic!("expected the two unblocked tuples") };
        assert_eq!(request, other_request);
        assert_ne!(a, b);
        assert_eq!(request.expression.span.slice(source.as_bytes()), Some(slot.as_bytes()));
        let prepared = loop {
            let answers = round.evaluate(Default::default()).unwrap();
            match round.advance(answers).unwrap() {
                PreparationStep::Evaluate(next) => round = *next,
                PreparationStep::Finished(PreparationProgress::Complete(prepared)) => break prepared,
                PreparationStep::Finished(PreparationProgress::OpenTarget(_)) => panic!("callback widths are target-independent"),
            }
        };
        let trees = prepared.typed();
        let answers = prepared.specialized_indices();
        assert_eq!(answers.len(), 3);
        for (name, width) in ["first", "second", "third"].into_iter().zip(widths) {
            let signature = trees.signatures.iter().find(|s| s.machine.path == name).unwrap();
            let (_, arguments) = signature.parameters[0].1.nominal_parts().unwrap();
            // Both tuples yielding five were ready on the first pass. Even the
            // delayed answer must retain that request, not just its source text.
            assert_eq!([a.as_slice(), b.as_slice()].contains(&arguments), width == 5);
            let answer = answers.iter().find(|a| &a.request == request && a.arguments == arguments);
            spec_corpus::observe_handoff!(
                "closed-callback-readiness/consumer",
                assert_eq!(answer.map(|a| &a.value), Some(&omega_typed::StaticValue::Integer(width.into())), "{name}: {slot}")
            );
        }
    }

    // Admitting relay cannot launder the precondition of its captured H.
    let restricted = source.replace("five()->u64{", "five()->u64 requires false {");
    spec_corpus::observe_handoff!(
        "closed-callback-readiness/captured-refusal",
        refused(
            prepare_source_package_diagnosed(&resolve(&restricted)).unwrap_err(),
            PreparationReason::CallbackNeedsRefinement,
            &restricted,
            "F()",
            "the exact static callback needs a checked refinement of its callable contract",
        )
    );
}

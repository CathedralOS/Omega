mod support;

use omega_typer::{InvocationTarget, ObligationKind, ValueType};
use support::check;

#[test]
fn public_contract_checks_the_visibility_of_the_selected_overload() {
    let declarations = "public machine pred(x:u8)->bool{true} machine pred(x:u16)->bool{true}";
    let accepted = format!("{declarations} public machine f(x:u8) requires pred(x) {{}}");
    let stage = check(&[("main.omg", &accepted)]).unwrap();
    assert!(
        stage
            .calls
            .iter()
            .any(|call| call.machine.path == "pred" && call.machine.operands == ["u8"])
    );

    let rejected = accepted.replace("f(x:u8)", "f(x:u16)");
    let error = check(&[("main.omg", &rejected)]).unwrap_err();
    assert!(
        error
            .message
            .contains("public signature exposes private declaration pred"),
        "{error:?}"
    );
}

#[test]
fn a_static_callback_carries_the_machine_application_into_refinement_and_invocation() {
    let declarations = "machine id<T>(x:T)->T{x} machine apply<const F:machine(x:u8)->u8>(x:u8)->u8{F(x)}";
    for selected in ["id::<u8>", "(id::<u8>)"] {
        let source = format!("{declarations} machine f(x:u8)->u8{{apply::<{selected}>(x)}}");
        let stage = check(&[("main.omg", &source)]).unwrap();
        let call = stage
            .calls
            .iter()
            .find(|call| call.machine.path == "apply")
            .unwrap();
        let arguments = [ValueType::Integer { signed: false, width: Some(8) }];
        assert!(matches!(&call.type_arguments[..], [ValueType::MachineSelection(id, args)] if id.path == "id" && args == &arguments));
        assert!(stage.obligations.iter().any(|obligation| matches!(
            &obligation.kind,
            ObligationKind::RefineCallable { machine, arguments: args, .. } if machine.path == "id" && args == &arguments
        )));
        let effects = stage
            .machine_effects
            .iter()
            .find(|effect| effect.machine.path == "f")
            .unwrap();
        assert!(matches!(
            &effects.invokes[..],
            [InvocationTarget::AppliedMachine { declaration, arguments: args }] if declaration.path == "id" && args == &arguments
        ));
    }

    let rejected = format!("{declarations} machine f(x:u8)->u8{{apply::<id::<u16>>(x)}}");
    let error = check(&[("main.omg", &rejected)]).unwrap_err();
    assert!(
        error
            .message
            .contains("selected machine has incompatible callable signature"),
        "{error:?}"
    );
}

#[test]
fn a_static_callback_requires_admitted_arguments_for_its_selected_machine() {
    let declarations = "trait Step<T>{machine run(x:T)->T;} ByteStep satisfies Step<u8>{machine run(x:u8)->u8{x}} BoolStep satisfies Step<bool>{machine run(x:bool)->bool{x}} machine selected<const E:Step<u8>>(x:u8)->u8{E::run(x)} machine apply<const F:machine(x:u8)->u8>(x:u8)->u8{F(x)}";
    let accepted = format!("{declarations} machine f(x:u8)->u8{{apply::<selected::<ByteStep>>(x)}}");
    let stage = check(&[("main.omg", &accepted)]).unwrap();
    assert!(stage.obligations.iter().any(|obligation| matches!(
        &obligation.kind,
        ObligationKind::RefineCallable { machine, arguments, .. }
            if machine.path == "selected"
                && matches!(arguments.as_slice(), [ValueType::Implementation(id)] if id.path == "ByteStep")
    )));

    for application in ["selected::<BoolStep>(x)", "apply::<selected::<BoolStep>>(x)"] {
        let rejected = format!("{declarations} machine f(x:u8)->u8{{{application}}}");
        let error = check(&[("main.omg", &rejected)]).unwrap_err();
        assert_eq!(error.message, "selected evidence has the wrong trait application", "{application}");
    }
}

#[test]
fn a_static_callback_matches_the_applied_nominal_requirement() {
    let declarations =
        "trait Step<T>{machine run<U>(x:u8)->u8;} machine selected<T>(x:u8)->u8 satisfies Step<T>::run {x} machine accept<const F:Step<u8>::run>(){}";
    let accepted = format!("{declarations} machine f(){{accept::<selected::<u8>>()}}");
    let stage = check(&[("main.omg", &accepted)]).unwrap();
    let byte = ValueType::Integer { signed: false, width: Some(8) };
    assert!(stage.obligations.iter().any(|obligation| matches!(
        &obligation.kind,
        ObligationKind::RefineCallable { machine, arguments, contract }
            if machine.path == "selected"
                && arguments.as_slice() == std::slice::from_ref(&byte)
                && contract.requirement.as_ref().is_some_and(|slot| {
                    slot.declaration.path == "Step::run" && slot.trait_arguments == arguments.as_slice()
                })
    )));

    let rejected = accepted.replace("selected::<u8>", "selected::<bool>");
    let error = check(&[("main.omg", &rejected)]).unwrap_err();
    assert_eq!(error.message, "selected machine lacks the exact satisfaction edge");
}

#[test]
fn a_static_callback_publishes_nested_evidence_at_the_selected_machine() {
    let source = "machine theorem<machine Step>(x:u8) requires machine Step(y:u8) requires y != 0 ensures y > 0; ensures Step::ensures(x) {} machine lemma(y:u8) requires y != 0 ensures y > 0 {} machine accept<const F:machine(x:u8)>(){} machine f(){accept::<theorem::<lemma>>()}";
    let resolved = support::resolve_packages(&[("game", "main.omg", source)], &[]);
    let typed = omega_typer::type_stage(&resolved).unwrap();
    let [hypothesis] = typed.logical_hypotheses.as_slice() else {
        panic!("expected the selected machine's hypothesis");
    };
    let tree = typed.trees().next().unwrap();
    let file = tree.resolved().file;
    let selection = file
        .nodes
        .iter()
        .enumerate()
        .find(|(node, syntax)| {
            matches!(syntax.kind, omega_parser::NodeKind::PathExpression | omega_parser::NodeKind::PathType)
                && file.span(*node).slice(source.as_bytes()) == Some(b"theorem::<lemma>".as_slice())
        })
        .map(|(node, _)| node)
        .expect("expected the applied machine path");
    assert!(tree.semantics_at(selection).any(|entry| matches!(
        entry,
        omega_typer::NodeSemantics::Obligation(obligation)
            if matches!(&obligation.kind,
                ObligationKind::SupplyHypothesis { requirement, evidence: ValueType::ProofCitation(citation), substitutions }
                if requirement == &hypothesis.site
                    && citation.path == "lemma"
                    && matches!(substitutions.as_slice(), [ValueType::ProofCitation(id)] if id == citation)
            )
    )));
}

#[test]
fn a_dependent_call_publishes_operand_order_and_formal_subjects() {
    let declaration = "data Frame<A:u8,B:u8>{} machine keep(first:u8,second:u8,frame:Frame<second,first>)->Frame<second,first>{frame}";
    let caller = "machine f(x:u8,y:u8,payload:Frame<x,y>)->Frame<x,y>{keep(y,(x),payload)}";
    let resolved = support::resolve_packages(
        &[
            ("game", "declaration.omg", declaration),
            ("game", "caller.omg", caller),
        ],
        &[],
    );
    let typed = omega_typer::type_stage(&resolved).unwrap();
    let signature = |path: &str| {
        typed
            .signatures
            .iter()
            .find(|s| s.machine.path == path)
            .unwrap()
    };
    let keep = signature("keep");
    let f = signature("f");
    let tree = typed
        .trees()
        .find(|tree| tree.resolved().file.path == "caller.omg")
        .unwrap();
    let file = tree.resolved().file;
    let node = file
        .nodes
        .iter()
        .enumerate()
        .find(|(node, syntax)| {
            syntax.kind == omega_parser::NodeKind::Call && file.span(*node).slice(caller.as_bytes()) == Some(b"keep(y,(x),payload)".as_slice())
        })
        .map(|(node, _)| node)
        .unwrap();
    let call = tree
        .semantics_at(node)
        .find_map(|entry| match entry {
            omega_typer::NodeSemantics::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    assert_eq!(call.machine, keep.machine);
    let [_, argument_list] = file.nodes[node].children.as_slice() else {
        panic!("expected a callee and argument list");
    };
    let [first, second, frame] = call.operands.as_slice() else {
        panic!("expected three selected operands");
    };
    for (position, (operand, caller_parameter)) in [(first, 1), (second, 0), (frame, 2)].into_iter().enumerate() {
        assert_eq!(operand.node, file.node_id(file.nodes[*argument_list].children[position]));
        assert_eq!(operand.parameter.file, "declaration.omg");
        assert_eq!(operand.parameter.span, keep.parameters[position].0);
        let ValueType::Subject { declaration, version: None } = &operand.subject else {
            panic!("the operand must preserve the caller's immutable binding");
        };
        assert_eq!(declaration.file, "caller.omg");
        assert_eq!(declaration.span, f.parameters[caller_parameter].0);
    }
    assert_eq!(second.site.span.slice(caller.as_bytes()), Some(b"(x)".as_slice()));
    assert_eq!(call.result, f.result);
    assert_eq!(call.instantiate(&keep.result), f.result);
    assert_eq!(call.instantiate(&keep.parameters[2].1), f.parameters[2].1);
}

#[test]
fn public_termination_keeps_a_private_ranking_witness() {
    use omega_typer::{RankingView, TypingEnvironment, type_stage_in_environment};
    let source = "data Down{} const limit:u8=7; public machine f(x:u8) terminates by x -> Down in limit {}";
    let resolved = support::resolve_packages(&[("game", "main.omg", source)], &[]);
    let byte = ValueType::Integer { signed: false, width: Some(8) };
    let views = [RankingView {
        declaration: resolved
            .declarations
            .iter()
            .find(|id| id.path == "Down")
            .unwrap()
            .clone(),
        subject: byte.clone(),
        parameters: Vec::new(),
        rank: byte,
        canonical_default: false,
    }];
    let stage = type_stage_in_environment(
        &resolved,
        &TypingEnvironment {
            ranking_views: &views,
            ..Default::default()
        },
    )
    .unwrap()
    .into_facts();
    assert!(matches!(&stage.ranking_witnesses[..], [witness] if witness.machine.path == "f" && witness.view.path == "Down"));
}

#[test]
fn competing_canonical_views_refuse_a_default_and_preserve_an_explicit_selection() {
    use omega_typer::{RankingView, TypingEnvironment, type_stage_in_environment};
    let run = |source: &str, rival_is_default| {
        let resolved = support::resolve_packages(&[("game", "rank.omg", source)], &[]);
        let byte = ValueType::Integer { signed: false, width: Some(8) };
        let views = [("Rival", rival_is_default), ("Preferred", true)].map(|(path, canonical_default)| RankingView {
            declaration: resolved
                .declarations
                .iter()
                .find(|id| id.path == path)
                .unwrap()
                .clone(),
            subject: byte.clone(),
            rank: byte.clone(),
            parameters: vec![],
            canonical_default,
        });
        let stage = type_stage_in_environment(
            &resolved,
            &TypingEnvironment {
                ranking_views: &views,
                ..Default::default()
            },
        )?;
        let [witness] = stage.ranking_witnesses.as_slice() else {
            panic!("expected one ranking witness")
        };
        assert_eq!(witness.view, views[1].declaration);
        assert_eq!(witness.rank, byte);
        Ok::<_, omega_typer::TypeError>(())
    };
    let source = "data Preferred{} data Rival{} machine f(x:u8) terminates by x -> Preferred {}";
    let implicit = source.replace(" -> Preferred", "");
    run(source, true).unwrap();
    run(&implicit, false).unwrap();
    let error = run(&implicit, true).unwrap_err();
    assert_eq!(error.message, "ambiguous canonical ranking view");
    assert_eq!(error.span.slice(implicit.as_bytes()), Some(b"x".as_slice()));
}

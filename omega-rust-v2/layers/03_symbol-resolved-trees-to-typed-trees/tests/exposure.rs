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

#[test]
fn preparation_receives_typed_static_demands_before_dependent_identity_is_known() {
    use omega_resolver::{ParsedSource, StageSource};
    use omega_typer::{
        NodeSemantics, SpecializedIndexValue, StaticIntegerDemand, TypingEnvironment, TypingProgress, type_source_package, type_source_package_for_preparation,
    };
    let declarations = "data Buffer<const N:u8>{} machine width(n:u8)->u8{n+%1}";
    for (body, specialized) in [
        ("machine f(x:Buffer<width(7)>)->Buffer<8>{x}", false),
        (
            "machine make<const N:u8>()->Buffer<width(N)>{Buffer::<width(N)>{}} machine f()->Buffer<8>{make::<7>()}",
            true,
        ),
    ] {
        let source = format!("{declarations} {body}");
        let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
        let syntax = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
        let resolved = omega_toolchain::resolve_source_package(&[StageSource {
            package: "game",
            source: ParsedSource {
                logical_path: "prepare.omg",
                text: &source,
                tokens: &tokens,
                tree: &syntax,
            },
        }])
        .unwrap();

        assert!(
            type_source_package(&resolved)
                .unwrap_err()
                .message
                .contains("admitted evaluation result")
        );
        let TypingProgress::Pending(pending) = type_source_package_for_preparation(&resolved, &TypingEnvironment::default()).unwrap() else {
            panic!("typing needs the selected static expression");
        };
        assert!(!pending.demands().is_empty());
        let byte = ValueType::Integer { signed: false, width: Some(8) };
        for demand in pending.demands() {
            let request = demand.request();
            assert_eq!(request.carrier, byte);
            let tree = pending.trees().trees().nth(request.node.file).unwrap();
            let file = tree.resolved().file;
            let expected = if specialized { b"width(N)".as_slice() } else { b"width(7)".as_slice() };
            assert_eq!(file.span(request.node.node).slice(source.as_bytes()), Some(expected));
            let selected = tree
                .semantics_at(request.node.node)
                .find_map(|semantics| match semantics {
                    NodeSemantics::Call(call) => Some(call),
                    _ => None,
                })
                .expect("expected a typed call at the evaluation request");
            assert_eq!(selected.machine.path, "width");
            assert_eq!(selected.operands.len(), 1);
            match demand {
                StaticIntegerDemand::Specialized { arguments, .. } if specialized => {
                    assert_eq!(
                        arguments,
                        &[ValueType::StaticInteger {
                            carrier: Box::new(byte.clone()),
                            value: 7.into()
                        }]
                    );
                    let make = resolved
                        .declarations
                        .iter()
                        .find(|id| id.path == "make")
                        .unwrap();
                    assert_eq!(
                        selected.operands[0].subject,
                        ValueType::Parameter {
                            owner: Box::new(make.clone()),
                            index: 0
                        }
                    );
                }
                StaticIntegerDemand::Closed(_) if !specialized => {
                    assert_eq!(
                        selected.operands[0].subject,
                        ValueType::StaticInteger {
                            carrier: Box::new(byte.clone()),
                            value: 7.into()
                        }
                    );
                }
                other => panic!("unexpected demand {other:?}"),
            }
        }

        let resume = |value: u8| {
            let mut evaluated_indices = Vec::new();
            let mut specialized_indices = Vec::new();
            for demand in pending.demands() {
                match demand {
                    StaticIntegerDemand::Closed(request) => evaluated_indices.push((request.clone(), value.into())),
                    StaticIntegerDemand::Specialized { request, arguments } => specialized_indices.push(SpecializedIndexValue {
                        request: request.clone(),
                        arguments: arguments.clone(),
                        value: value.into(),
                    }),
                }
            }
            type_source_package_for_preparation(
                &resolved,
                &TypingEnvironment {
                    evaluated_indices: &evaluated_indices,
                    specialized_indices: &specialized_indices,
                    ..Default::default()
                },
            )
        };
        let TypingProgress::Complete(typed) = resume(8).unwrap() else {
            panic!("the evaluated index closes typing");
        };
        let f = typed
            .signatures
            .iter()
            .find(|signature| signature.machine.path == "f")
            .unwrap();
        assert_eq!(f.result.to_string(), "game::Buffer<8>");
        assert!(
            resume(9)
                .unwrap_err()
                .message
                .contains("expected game::Buffer<8>")
        );
    }
}

#[test]
fn array_return_waits_for_its_extent_and_rechecks_the_closed_type() {
    use omega_typer::{StaticIntegerDemand, TypingEnvironment, TypingProgress, type_source_package_for_preparation};
    let source = "machine width(n:u64)->u64{n} machine f(x:[[u8; width(4)]; 2])->[[u8; 4]; 2]{x}";
    fn resolve(text: &str) -> omega_typer::SourcePackage<'_> {
        let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
        omega_typer::resolve_source_package(&[omega_resolver::StageSource {
            package: "game",
            source: omega_resolver::ParsedSource {
                logical_path: "arrays.omg",
                text,
                tokens: &tokens,
                tree: &tree,
            },
        }])
        .unwrap()
    }
    let resolved = resolve(source);

    let TypingProgress::Pending(pending) = type_source_package_for_preparation(&resolved, &Default::default()).unwrap() else {
        panic!("the inner extent requires evaluation");
    };
    let [StaticIntegerDemand::Closed(request)] = pending.demands() else {
        panic!("expected just the call that computes the inner extent");
    };
    let resume = |width: u64| {
        type_source_package_for_preparation(
            &resolved,
            &TypingEnvironment {
                evaluated_indices: &[(request.clone(), width.into())],
                ..Default::default()
            },
        )
    };
    assert!(matches!(resume(4).unwrap(), TypingProgress::Complete(_)));
    assert_eq!(resume(5).unwrap_err().message, "expected [[u8; 4]; 2], found [[u8; 5]; 2]");
    let wrong_element = source.replace("->[[u8", "->[[u16");
    let resolved = resolve(&wrong_element);

    let error = type_source_package_for_preparation(&resolved, &Default::default()).unwrap_err();
    assert!(
        error
            .message
            .starts_with("expected [[u16; 4]; 2], found [[u8; pending")
    );
}

#[test]

fn constant_initializers_publish_references_calls_and_suffixed_values() {
    use omega_typer::{CallUse, NodeSemantics};
    let source = "const FORWARD:u8=SIZE; const SIZE=width(7u8); machine width(n:u8)->u8{n+%1}";
    let resolved = support::resolve_packages(&[("game", "constants.omg", source)], &[]);
    let typed = omega_typer::type_stage(&resolved).unwrap();
    let constant = |path| {
        typed
            .constants
            .iter()
            .find(|value| value.declaration.path == path)
            .unwrap()
    };
    let alias = constant("FORWARD");
    let size = constant("SIZE");
    let tree = typed.trees().next().unwrap();
    let byte = ValueType::Integer { signed: false, width: Some(8) };
    assert_eq!(alias.initializer.carrier, byte);
    assert_eq!(size.initializer.carrier, byte);
    assert!(
        tree.semantics_at(alias.initializer.node.node)
            .any(|record| matches!(
                record, NodeSemantics::Constant(declaration) if declaration == &size.declaration
            ))
    );
    let width = tree
        .semantics_at(size.initializer.node.node)
        .find_map(|record| match record {
            NodeSemantics::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    assert_eq!(width.machine.path, "width");
    assert_eq!(width.use_site, CallUse::StaticArgument);
    assert!(
        tree.semantics_at(width.operands[0].node.node)
            .any(|record| matches!(
                record, NodeSemantics::Landing(landing) if landing.value == num_rational::BigRational::from_integer(7.into()) && landing.ty == byte
            ))
    );
}

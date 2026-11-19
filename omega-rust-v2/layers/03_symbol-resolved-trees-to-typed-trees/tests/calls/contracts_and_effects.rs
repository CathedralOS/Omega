#[test]
fn specialized_callback_crash_retains_the_selected_guard_and_its_argument() {
    let code = "machine guarded(x:u8)->u8 crashes Trap x == 0 {x} machine relay<const F:machine(x:u8)->u8>(v:u8)->u8{F(v)} machine forward<const H:machine(x:u8)->u8>(q:u8)->u8{relay::<H>(q)} machine outer(a:u8)->u8{forward::<guarded>(a)}";
    let stage = check(&[("guarded.omg", code)]).unwrap();
    let (call, route, enclosing) = stage
        .guarded_calls
        .iter()
        .find_map(|guarded| match *guarded {
            omega_typer::GuardedCall::Indirect {
                call,
                route,
                specialization: Some(enclosing),
            } => Some((
                stage.guarded_invocation(call).unwrap(),
                stage.crash_route(route).unwrap(),
                &stage.calls[enclosing],
            )),
            _ => None,
        })
        .unwrap();
    let spelling = |s: &omega_typer::Site| s.span.slice(code.as_bytes()).unwrap();
    assert_eq!(spelling(route.guard.as_ref().unwrap()), b"x == 0");
    assert_eq!(spelling(&call.operands[0]), b"v");
    assert_eq!(spelling(&enclosing.site), b"forward::<guarded>(a)");
    assert_eq!(enclosing.caller.as_ref().unwrap().path, "outer");
    assert!(matches!(call.entry, omega_typer::InvocationTarget::StaticParameter { .. }));
    assert!(matches!(enclosing.type_arguments.as_slice(), [ValueType::MachineSelection(id, _)] if id.path == "guarded"));
}

#[test]
fn result_domains_and_contracts() {
    let code = "domain u8::Small requires self < 8; machine make()->u8 {9} machine make()->u8 in Small {1 as +Small} machine bare()->u8{make()} machine selected()->u8 in Small {make()}";
    let stage = check(&[("a.omg", code)]).unwrap();
    assert!(stage.calls[0].machine.result_domains.is_empty());
    assert!(!stage.calls[1].machine.result_domains.is_empty());
    for (source, msg) in [
        ("domain u8::A; domain u8::B; machine f(x:u8 in A & B)->u8 in A { x as =A }", "cast omits domain"),
        ("domain u8::A; machine f(x:u8 in A)->u8 in A {x as +A}", "invalid +"),
        ("domain u8::A; machine f()->u8 in A {3}", "expected u8 in"),
        ("machine f(n:u8) requires n { }", "expected bool"),
        ("machine f(n:u8)->u8 ensures result {n}", "expected bool"),
    ] {
        let e = check(&[("a.omg", source)]).unwrap_err();
        assert!(e.message.contains(msg), "{e:?}");
    }
}

#[test]
fn pause_defaults_belong_to_the_declaration_being_checked() {
    let source = "machine private_fn() {}
        public machine exported() {}
        machine bare() blocks suspends {}
        machine explicit() blocks false suspends true {}
        trait Base {}
        trait Local = Base { machine * blocks suspends false; }";
    let stage = check(&[("pause.omg", source)]).unwrap();
    for (name, block, suspend) in [
        ("private_fn", None, None),
        ("exported", Some(false), Some(false)),
        ("bare", Some(true), Some(true)),
        ("explicit", Some(false), Some(true)),
    ] {
        let signature = stage
            .signatures
            .iter()
            .find(|s| s.machine.path == name)
            .unwrap();
        assert_eq!((signature.pause.block, signature.pause.suspend), (block, suspend), "{name}");
    }
    let restriction = stage
        .refinements
        .iter()
        .find(|r| r.schema.path == "Local")
        .unwrap();
    assert!(!restriction.prohibits_block);
    assert!(restriction.prohibits_suspend);
}

#[test]
fn inferred_pause_and_argument_domains() {
    let code = "machine wait() blocks {} machine wrap(){block wait()} machine entry(){block wrap()}";
    let stage = check(&[("a.omg", code)]).unwrap();
    assert!(stage.machine_effects.iter().all(|m| m.pause.block));
    for (code, message) in [
        (
            "machine wait() blocks {} machine wrap(){block wait()} machine entry(){wrap()}",
            "block acknowledgement",
        ),
        ("machine f(x:u8)->u8{x} machine g()->u8{f(suspend f(0))}", "suspension position"),
        ("machine wait() blocks {} public machine f(){block wait()}", "blocking ceiling"),
    ] {
        let e = check(&[("a.omg", code)]).unwrap_err();
        assert!(e.message.contains(message), "{e:?}");
    }
    let stage = check(&[(
        "a.omg",
        "domain u8::D; machine make()->u8{1} machine make()->u8 in D{1 as +D} machine take(x:u8 in D){} machine f(){take(make())}",
    )])
    .unwrap();
    assert!(!stage.calls[0].machine.result_domains.is_empty());
}

#[test]
fn forwarded_nominal_callback_publishes_its_originating_slot() {
    let source = "trait Step<T>{machine apply(v:T);} machine via<T,const F:Step<T>::apply>(v:T){F(v)} machine wrap<T,const G:machine>(v:T){via::<T,G>(v)}";
    let stage = check(&[("callbacks.omg", source)]).unwrap();
    assert_eq!(stage.callable_bounds.len(), 2);
    assert!(stage.callable_bounds.iter().all(|(_, _, c)| {
        c.requirement
            .as_ref()
            .is_some_and(|r| r.declaration.path == "Step::apply")
    }));
    let code = format!("{source} machine fake(v:bool){{}} machine bad(){{wrap::<bool,fake>(true)}}");
    let e = check(&[("callbacks.omg", &code)]).unwrap_err();
    assert!(e.message.contains("satisfaction edge"), "{e:?}");
}

#[test]
fn forwarded_callback_uses_the_value_binding_when_a_type_shares_its_name() {
    let source =
        "data G {} trait Step<T>{machine apply(v:T);} machine via<T,const F:Step<T>::apply>(v:T){F(v)} machine wrap<const G:machine>(v:G){via::<G,G>(v)}";
    let stage = check(&[("callbacks.omg", source)]).unwrap();
    let (_, _, bound) = stage
        .callable_bounds
        .iter()
        .find(|(owner, _, _)| owner.path == "wrap")
        .unwrap();
    let (data, _) = stage
        .record_fields
        .iter()
        .find(|(id, _)| id.path == "G")
        .unwrap();
    assert_eq!(bound.parameters, [ValueType::Nominal(Box::new(data.clone()))]);
    let origin = bound.requirement.as_ref().unwrap();
    assert_eq!(origin.declaration.path, "Step::apply");
    assert_eq!(origin.trait_arguments, bound.parameters);
}

#[test]
fn qualified_static_callback_uses_the_members_arguments_and_applied_receiver() {
    let defs = "trait Step<T>{machine apply(v:T);} boundary trait Relay<T>{machine via<const F:Step<T>::apply>(v:T);}";
    let caller = "machine wrap<const G:machine>(v:bool){Relay::<bool>::via::<G>(v)}";
    let stage = check(&[("schema.omg", defs), ("caller.omg", caller)]).unwrap();
    let (_, _, bound) = stage
        .callable_bounds
        .iter()
        .find(|(owner, _, _)| owner.path == "wrap")
        .unwrap();
    assert_eq!(bound.parameters, [ValueType::Bool]);
    assert_eq!(bound.requirement.as_ref().unwrap().trait_arguments, [ValueType::Bool]);

    let forwarded = "machine first<const H:machine>(v:bool){wrap::<H>(v)}";
    let stage = check(&[
        ("first.omg", forwarded),
        ("schema.omg", defs),
        ("caller.omg", caller),
    ])
    .unwrap();
    assert!(
        stage
            .callable_bounds
            .iter()
            .any(|(owner, _, c)| owner.path == "first" && c == bound)
    );

    let conflict = "machine wrap<const G:machine>(v:bool){Relay::<bool>::via::<G>(v); Relay::<u8>::via::<G>(0);}";
    let error = check(&[("schema.omg", defs), ("caller.omg", conflict)]).unwrap_err();
    assert!(
        error
            .message
            .contains("incompatible originating callback requirements"),
        "{error:?}"
    );
    assert_eq!(error.span.slice(conflict.as_bytes()), Some(b"G".as_slice()));

    let cycle = "machine first<const F:machine>(){second::<F>()} machine second<const G:machine>(){first::<G>()}";
    let error = check(&[("cycle.omg", cycle)]).unwrap_err();
    assert!(error.message.contains("lacks an originating requirement"), "{error:?}");
}

#[test]
fn cleanup_is_an_exact_owned_call_and_conflicting_domain_plans_reject() {
    let source = "data R dropped by end {v:u8;} machine end(r:R){} domain R::Live dropped by end; machine read(r:R in Live){}";
    let stage = check(&[("resource.omg", source)]).unwrap();
    assert_eq!(stage.cleanups.len(), 2);
    for hook in &stage.cleanups {
        assert_eq!(hook.site.span.slice(source.as_bytes()), Some(b"dropped by end".as_slice()));
        assert!(
            stage
                .obligations
                .iter()
                .any(|o| o.site == hook.site && matches!(&o.kind, omega_typer::ObligationKind::CleanupContract(machine) if machine == &hook.machine))
        );
    }
    let conflicting = source.replace("::Live dropped by end", "::Live dropped by other") + " machine other(r:R){}";
    assert!(
        check(&[("resource.omg", &conflicting)])
            .unwrap_err()
            .message
            .contains("conflicting cleanup")
    );
    let wrong = source.replace("end(r:R)", "end(r:@R)");
    assert!(
        check(&[("resource.omg", &wrong)])
            .unwrap_err()
            .message
            .contains("owned carrier")
    );
}

#[test]
fn crash_causes_and_checked_arithmetic_keep_their_proof_sites() {
    let source = "machine risky(a:u8,b:u8)->u8 crashes Trap{a+!b} public machine use_it(a:u8)->u8{risky(a,1)}";
    let stage = check(&[("effects.omg", source)]).unwrap();
    assert!(
        stage
            .machine_effects
            .iter()
            .any(|e| e.machine.path == "risky" && e.crash_ceiling == Some(vec![omega_typer::CrashCause::Trap]))
    );
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(&o.kind,omega_typer::ObligationKind::CrashCoverage{machine,cause:omega_typer::CrashCause::Trap} if machine.path=="use_it"))
    );
    let conflicting = "machine wrong() crashes none crashes Abort{}";
    assert!(
        check(&[("effects.omg", conflicting)])
            .unwrap_err()
            .message
            .contains("conflicts")
    );
}

#[test]
fn a_runtime_callback_carries_its_reach_into_the_calling_machine() {
    let prefix = "boundary trait Sensor{} ";
    let accepted = format!("{prefix} machine run(f:machine() reaches Sensor crashes none terminates) reaches Sensor {{f()}}");
    let stage = check(&[("effects.omg", &accepted)]).unwrap();
    assert!(stage.indirect_calls[0].contract.terminates);
    let refused = format!("{prefix} machine run(f:machine() reaches Sensor crashes none) reaches none {{f()}}");
    assert!(
        check(&[("effects.omg", &refused)])
            .unwrap_err()
            .message
            .contains("callback exceeds")
    );
}

#[test]
fn admitted_ranking_views_check_the_subject_arguments_and_rank_bounds() {
    use omega_parser::{NodeKind, TerminationClause};
    use omega_typer::{NodeSemantics, ObligationKind, RankingView, TypeError, TypedTrees, TypingEnvironment, type_stage_in_environment};

    const BYTE: ValueType = ValueType::Integer { signed: false, width: Some(8) };
    const WORD: ValueType = ValueType::Integer {
        signed: false,
        width: Some(16),
    };

    fn run(code: &str) -> Result<TypedTrees<'_>, TypeError> {
        let resolved = support::resolve_packages(&[("game", "rank.omg", code)], &[]);
        let view = |name: &str| {
            resolved
                .declarations
                .iter()
                .find(|d| d.path == name)
                .unwrap()
                .clone()
        };
        let views = [
            RankingView {
                declaration: view("Down"),
                subject: BYTE,
                parameters: vec![ValueType::Bool],
                rank: BYTE,
                canonical_default: false,
            },
            RankingView {
                declaration: view("DefaultDown"),
                subject: BYTE,
                parameters: vec![],
                rank: WORD,
                canonical_default: true,
            },
        ];
        type_stage_in_environment(
            &resolved,
            &TypingEnvironment {
                ranking_views: &views,
                ..Default::default()
            },
        )
    }

    let source = "data Down{} data DefaultDown{} machine f(x:u8,flag:bool) terminates by x -> Down(flag) in 0..=255 {} machine g(x:u8) terminates by x {} machine h() terminates {}";
    let typed = run(source).unwrap();
    assert_eq!(typed.ranking_witnesses.len(), 2);
    let tree = typed.trees().next().unwrap();
    let file = tree.resolved().file;
    let text = |node| file.span(node).slice(source.as_bytes()).unwrap();
    let clauses = file
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(node, syntax)| syntax.termination_clause().map(|clause| (node, clause)))
        .collect::<Vec<_>>();
    let [
        (explicit, application),
        (default, short_form),
        (bare, bare_form),
    ] = clauses.as_slice()
    else {
        panic!("expected one explicit, one canonical and one bare termination clause");
    };

    for (node, machine, selection) in [
        (*explicit, "f", Some(("Down", BYTE))),
        (*default, "g", Some(("DefaultDown", WORD))),
        (*bare, "h", None),
    ] {
        let mut witnesses = tree.semantics_at(node).filter_map(|entry| match entry {
            NodeSemantics::RankingWitness(witness) => Some(witness),
            _ => None,
        });
        if let Some((view, rank)) = selection {
            let witness = witnesses.next().unwrap();
            assert_eq!(witness.node, file.node_id(node));
            assert_eq!(witness.machine.path, machine);
            assert_eq!(witness.view.path, view);
            assert_eq!(witness.rank, rank);
        }
        assert!(witnesses.next().is_none());
        assert!(tree.semantics_at(node).any(|entry| matches!(
            entry,
            NodeSemantics::Obligation(obligation)
                if matches!(&obligation.kind, ObligationKind::Termination(owner) if owner.path == machine)
        )));
    }

    let TerminationClause::By {
        measure,
        order: Some(order),
        bound: Some(bound),
    } = *application
    else {
        panic!("expected the authored subject, view application and rank range");
    };
    assert_eq!(text(measure), b"x");
    assert!(
        tree.semantics_at(measure)
            .any(|entry| matches!(entry, NodeSemantics::Expression(typed) if typed.ty == BYTE))
    );
    assert_eq!(file.nodes[order].kind, NodeKind::Call);
    let [view, arguments] = file.nodes[order].children.as_slice() else {
        panic!("expected the view and its argument list");
    };
    assert_eq!(text(*view), b"Down");
    let [argument] = file.nodes[*arguments].children.as_slice() else {
        panic!("expected the authored view argument");
    };
    assert_eq!(text(*argument), b"flag");
    assert!(
        tree.semantics_at(*argument)
            .any(|entry| matches!(entry, NodeSemantics::Expression(typed) if typed.ty == ValueType::Bool))
    );

    let range = file.range_bounds(bound).unwrap();
    assert!(range.inclusive);
    for (endpoint, spelling) in [(range.start, b"0".as_slice()), (range.end, b"255".as_slice())] {
        let endpoint = endpoint.unwrap();
        assert_eq!(text(endpoint), spelling);
        assert!(
            tree.semantics_at(endpoint)
                .any(|entry| matches!(entry, NodeSemantics::Expression(typed) if matches!(typed.ty, ValueType::Exact { .. })))
        );
        assert!(
            tree.semantics_at(endpoint)
                .any(|entry| matches!(entry, NodeSemantics::Landing(typed) if typed.ty == BYTE))
        );
    }
    assert!(matches!(short_form, TerminationClause::By { order: None, bound: None, .. }));
    assert!(matches!(bare_form, TerminationClause::Bare));

    for (original, replacement, message) in [
        ("..=255", "..=256", "expected u8"),
        ("f(x:u8", "f(x:bool", "expected u8"),
        ("Down(flag)", "Down(x)", "expected bool"),
    ] {
        let error = run(&source.replace(original, replacement)).unwrap_err();
        assert!(error.message.contains(message), "{error:?}");
    }
}

#[test]
fn v1_crash_route_guards_type_as_total_facts_about_the_entry_value() {
    let source = "machine forward(value:i64)->i64 crashes Trap value == 0 {value}";
    let stage = check(&[("route.omg", source)]).unwrap();
    assert_eq!(stage.crash_routes[0].cause, omega_typer::CrashCause::Trap);
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| Some(&o.site) == stage.crash_routes[0].guard.as_ref() && matches!(o.kind, omega_typer::ObligationKind::TotalCrashGuard))
    );
    assert_eq!(
        stage.crash_routes[0]
            .guard
            .as_ref()
            .unwrap()
            .span
            .slice(source.as_bytes())
            .unwrap(),
        b"value == 0"
    );
    for (guard, diagnostic) in [
        ("value", "expected bool"),
        ("value +! 1 == 0", "trapping arithmetic"),
    ] {
        assert!(
            check(&[("route.omg", &source.replace("value == 0", guard))])
                .unwrap_err()
                .message
                .contains(diagnostic)
        );
    }
}

#[test]
fn a_parenthesized_crash_guard_stays_the_guard_of_its_cause() {
    // The parser once read `Trap (...)` as a family application, so the cause failed to resolve and the guard vanished.
    for guard in ["(value == 0)", "(value == 0) || done"] {
        let source = format!("machine forward(value:i64, done:bool)->i64 crashes Trap {guard} {{value}}");
        let stage = check(&[("route.omg", &source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        let route = &stage.crash_routes[0];
        assert_eq!(route.cause, omega_typer::CrashCause::Trap, "{source}");
        let site = route.guard.as_ref().unwrap();
        assert_eq!(site.span.slice(source.as_bytes()).unwrap(), guard.as_bytes(), "{source}");
        assert!(
            stage
                .obligations
                .iter()
                .any(|o| &o.site == site && matches!(o.kind, omega_typer::ObligationKind::TotalCrashGuard)),
            "{source}"
        );
    }
}

#[test]
fn synchronous_invocation_distinguishes_two_callbacks_with_the_same_type() {
    let ty = "machine() reaches none crashes none";
    let head = format!("machine run(f:{ty}) invokes f {{f()}}");
    let source = format!("{head} machine wrap(left:{ty},right:{ty}) invokes right {{run(right)}}");
    let stage = check(&[("invokes.omg", &source)]).unwrap();
    let effect = stage
        .machine_effects
        .iter()
        .find(|e| e.machine.path == "wrap")
        .unwrap();
    assert!(matches!(
        effect.invokes.as_slice(),
        [omega_typer::InvocationTarget::RuntimeParameter { index: 1, .. }]
    ));
    let err = check(&[("invokes.omg", &source.replace("run(right)", "run(left)"))]).unwrap_err();
    assert!(err.message.contains("synchronous invocation ceiling"));
}

#[test]
fn forwarded_callback_targets_survive_grouping_in_the_typed_call() {
    use omega_typer::InvocationTarget;
    let source = "machine action() reaches none crashes none {}
        machine run(f:machine() reaches none crashes none) invokes f {f()}
        machine wrap(g:machine() reaches none crashes none) invokes g {run((g))}
        machine start() invokes action {wrap((action))}";
    let stage = check(&[("invokes.omg", source)]).unwrap();
    let wrap = stage.calls.iter().find(|c| c.machine.path == "run").unwrap();
    let start = stage.calls.iter().find(|c| c.machine.path == "wrap").unwrap();
    assert!(matches!(&wrap.operands[0].callable, Some(InvocationTarget::RuntimeParameter { owner, index:0 }) if owner.path == "wrap"));
    assert!(matches!(&start.operands[0].callable, Some(InvocationTarget::Declaration(slot)) if slot.declaration.path == "action"));
    let composed = stage
        .machine_effects
        .iter()
        .find(|e| e.machine.path == "start")
        .unwrap();
    assert_eq!(composed.invokes.as_slice(), std::slice::from_ref(start.operands[0].callable.as_ref().unwrap()));
    assert_eq!(start.operands[0].site.span.slice(source.as_bytes()), Some(b"(action)".as_slice()));
}

#[test]
fn recursive_forwarding_propagates_each_reach_and_callback_once() {
    let defs = "boundary trait IO<T>{} machine action() reaches IO<u8> crashes none {}
        machine start(){first(action)}";
    let cycle = "machine first(cb:machine() reaches IO<u8> crashes none){second(cb);second(cb)}
        machine second(cb:machine() reaches IO<u8> crashes none){first(cb);cb()}";
    for files in [
        [("defs.omg", defs), ("cycle.omg", cycle)],
        [("cycle.omg", cycle), ("defs.omg", defs)],
    ] {
        let stage = check(&files).unwrap();
        for name in ["first", "second", "start"] {
            let effects = stage
                .machine_effects
                .iter()
                .find(|e| e.machine.path == name)
                .unwrap();
            let [service] = effects.reach.as_slice() else {
                panic!("{name} reach: {:?}", effects.reach)
            };
            assert_eq!(service.service.path, "IO");
            assert_eq!(service.arguments[0].to_string(), "u8");
            let [entry] = effects.invokes.as_slice() else {
                panic!("{name} invocations: {:?}", effects.invokes)
            };
            if name == "start" {
                assert!(matches!(entry, omega_typer::InvocationTarget::Declaration(slot) if slot.declaration.path == "action"));
            } else {
                assert!(matches!(entry, omega_typer::InvocationTarget::RuntimeParameter{owner,index:0} if owner.path == name));
            }
        }
    }
}

#[test]
fn governed_domains_require_the_admitted_policy_requirement() {
    use omega_resolver::{ParsedSource, StageSource, resolve_stage};
    use omega_typer::{TypingEnvironment, type_stage_in_environment};
    let code = "data Schema{} data Plan{} trait AccessPolicy{machine plan(s:Schema)->Plan;} trait OtherPolicy{machine plan(s:Schema)->Plan;} Rule satisfies AccessPolicy{machine plan(s:Schema)->Plan terminates{Plan{}}} data C{} domain C::Locked access Rule;";
    let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "game",
        source: ParsedSource {
            logical_path: "policy.omg",
            text: code,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let resolved = resolve_stage(&sources, &[]).unwrap();
    let run = |name| {
        type_stage_in_environment(
            &resolved,
            &TypingEnvironment {
                access_plan_requirement: resolved.declarations.iter().find(|d| d.path == name),
                ..Default::default()
            },
        )
    };
    let stage = run("AccessPolicy::plan").unwrap();
    assert_eq!(stage.access_policies[0].plan_machine.path, "Rule::plan");
    assert!(stage.obligations.iter().any(|o| o.site.file == "policy.omg"
        && o.site.span.slice(code.as_bytes()) == Some(b"access Rule")
        && matches!(&o.kind, omega_typer::ObligationKind::EvaluateAccessPlan { domain,machine }
            if domain.path=="C::Locked" && machine.path=="Rule::plan")));
    assert!(
        run("OtherPolicy::plan")
            .unwrap_err()
            .message
            .contains("does not supply")
    );
    assert!(run("").unwrap_err().message.contains("requires the admitted"));
}

#[test]
fn a_result_domain_can_be_declared_before_its_parent() {
    let parent = "domain u8::Parent;";
    let child = "domain u8::Parent::Child;";
    let caller = "machine pick(x:u8)->u8 in u8::Parent::Child{x as +Parent +u8::Parent::Child} machine f(x:u8)->u8 in u8::Parent::Child{pick(x)}";
    for (first, second) in [(parent, child), (child, parent)] {
        let stage = check(&[("a.omg", first), ("b.omg", second), ("caller.omg", caller)]).unwrap();
        let [call] = stage.calls.as_slice() else {
            panic!("expected one selected call")
        };
        assert_eq!(call.machine.result_domains, ["u8 :: Parent :: Child"]);
        let ValueType::Qualified(_, domains) = &call.result else {
            panic!("expected a qualified result")
        };
        assert_eq!(
            domains
                .iter()
                .map(|domain| domain.declaration.path.as_str())
                .collect::<Vec<_>>(),
            ["u8::Parent", "u8::Parent::Child"]
        );
    }
}

#[test]
fn service_reach_uses_the_declared_boundary_trait_in_all_files() {
    let run = |header, body| check(&[("a.omg", body), ("z.omg", header)]);
    run("public boundary trait Sensor{}", "machine f() reaches Sensor {}").unwrap();
    run("trait Base{} boundary trait Sensor:Base{}", "machine f() reaches Sensor {}").unwrap();
    for header in ["trait Sensor{}", "trait boundary{} trait Sensor:boundary{}"] {
        let error = run(header, "machine f() reaches Sensor {}").unwrap_err();
        assert_eq!(error.file, "a.omg");
        assert_eq!(error.message, "reach requires a boundary trait");
    }
    let error = run("boundary trait Sensor{}", "trait Local:Sensor{}").unwrap_err();
    assert_eq!(error.message, "ordinary trait inherits a boundary trait");
    let error = run("trait Sensor{}", "machine f(cb:machine() reaches Sensor crashes none) {}").unwrap_err();
    assert_eq!(error.message, "reach requires a boundary trait");
}

#[test]
fn passing_a_stored_callable_preserves_its_crash_ceiling() {
    let bodies = ["none", "Trap", "Abort"];
    for actual in bodies {
        for allowed in bodies {
            let code =
                format!("machine take(f:machine() reaches none crashes {allowed}){{}} machine pass(cb:machine() reaches none crashes {actual}){{take(cb)}}");
            assert_eq!(check(&[("row.omg", &code)]).is_ok(), actual == "none" || actual == allowed, "{code}");
        }
    }
}

#[test]
fn guards_reference_direct_and_indirect_executions_and_their_declared_routes() {
    let code = "machine named(x:u8) crashes Trap x == 1 {} machine test(cb:machine(x:u8) reaches none crashes Abort x == 2) {named(3); cb(4);}";
    let stage = check(&[("routes.omg", code)]).unwrap();
    let at = |site: &omega_typer::Site| site.span.slice(code.as_bytes()).unwrap();
    let routes: Vec<_> = stage
        .guarded_calls
        .iter()
        .map(|guard| match *guard {
            omega_typer::GuardedCall::Direct { call, route } => {
                let call = &stage.calls[call];
                let route = stage.crash_route(route).unwrap();
                (at(&call.site), at(&call.operands[0].site), at(route.guard.as_ref().unwrap()), route.cause)
            }
            omega_typer::GuardedCall::Indirect {
                call,
                route,
                specialization: None,
            } => {
                let call = stage.guarded_invocation(call).unwrap();
                let route = stage.crash_route(route).unwrap();
                (at(&call.site), at(&call.operands[0]), at(route.guard.as_ref().unwrap()), route.cause)
            }
            _ => panic!("unexpected specialization"),
        })
        .collect();
    assert_eq!(routes.len(), 2);
    assert!(routes.contains(&(b"named(3)", b"3", b"x == 1", omega_typer::CrashCause::Trap)));
    assert!(routes.contains(&(b"cb(4)", b"4", b"x == 2", omega_typer::CrashCause::Abort)));
}

#[test]
fn calls_in_facts_require_effect_free_targets_even_without_a_crash_guard() {
    let schema = "boundary trait Sensor{}";
    let direct = "machine ready()->bool reaches Sensor {true} machine require_ready() requires ready() {}";
    let indirect = "machine require_ready(cb:machine()->bool reaches Sensor crashes none) requires cb() {}";
    for (body, site, reason) in [
        (direct, "ready()", "effect-free computation"),
        (indirect, "cb()", "effect-free callable"),
    ] {
        let source = format!("{schema} {body}");
        let error = check(&[("facts.omg", &source)]).unwrap_err();
        assert!(error.message.contains(reason), "{error:?}");
        assert_eq!(error.span.get_start_offset(), source.rfind(site).unwrap());
        check(&[("facts.omg", &source.replace("reaches Sensor", "reaches none"))]).unwrap();
    }
}

#[test]
fn a_fact_uses_the_pause_effect_of_its_selected_callback() {
    let declarations = "machine apply<const F:machine()->bool suspends>()->bool suspends {suspend F()}
                        machine local()->bool {true} machine parked()->bool suspends {true}";
    let check_fact = |selected: &str| {
        let source = format!("{declarations} machine verified() requires apply::<{selected}>() {{}}");
        check(&[("pause.omg", &source)])
    };
    let stage = check_fact("local").unwrap();
    assert!(
        stage
            .machine_effects
            .iter()
            .map(|effect| &effect.machine)
            .eq(stage.signatures.iter().map(|sig| &sig.machine))
    );
    let template = stage
        .machine_effects
        .iter()
        .find(|effect| effect.machine.path == "apply")
        .unwrap();
    assert!(template.callback_pause[&0].suspend);
    assert!(!template.fixed_pause.suspend);
    let fact = stage
        .calls
        .iter()
        .find(|call| call.use_site == omega_typer::CallUse::Fact)
        .unwrap();
    assert!(matches!(fact.type_arguments.as_slice(), [ValueType::MachineSelection(id, _)] if id.path == "local"));
    assert!(
        check_fact("parked")
            .unwrap_err()
            .message
            .contains("effect-free computation")
    );
}

#[test]
fn structural_callable_clauses_use_the_normalized_parameter_and_result_types() {
    let source = "data Byte=u8; data Handler { f:machine(x:Byte)->Byte requires x > 0 ensures result == x + 1 reaches none crashes none; }";
    let stage = check(&[("callback.omg", source)]).unwrap();
    for token in [">", "+", "=="] {
        assert!(stage.operations.iter().any(|op| op.token == token), "{token}");
    }
    let invalid = source.replace("x + 1", "x + true");
    assert!(
        check(&[("callback.omg", &invalid)])
            .unwrap_err()
            .message
            .contains("no matching operator")
    );
}

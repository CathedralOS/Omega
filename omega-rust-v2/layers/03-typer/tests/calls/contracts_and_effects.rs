#[test]
fn specialized_callback_crash_retains_the_selected_guard_and_its_argument() {
    let code = "machine guarded(x:u8)->u8 crashes Trap x == 0 {x} machine relay<const F:machine(x:u8)->u8>(v:u8)->u8{F(v)} machine forward<const H:machine(x:u8)->u8>(q:u8)->u8{relay::<H>(q)} machine outer(a:u8)->u8{forward::<guarded>(a)}";
    let stage = check(&[("guarded.omg", code)]).unwrap();
    let guard = stage
        .guarded_calls
        .iter()
        .find(|c| c.specialization.is_some())
        .unwrap();
    let spelling = |s: &omega_typer::Site| s.span.slice(code.as_bytes()).unwrap();
    assert_eq!(spelling(guard.guard.as_ref().unwrap()), b"x == 0");
    assert_eq!(spelling(&guard.operands[0]), b"v");
    assert_eq!(spelling(guard.specialization.as_ref().unwrap()), b"forward::<guarded>(a)");
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
fn cleanup_is_an_exact_owned_call_and_conflicting_domain_plans_reject() {
    let source = "data R dropped by end {v:u8;} machine end(r:R){} domain R::Live dropped by end; machine read(r:R in Live){}";
    let stage = check(&[("resource.omg", source)]).unwrap();
    assert_eq!(stage.cleanups.len(), 2);
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
    use omega_resolver::{ParsedSource, StageSource, resolve_stage};
    use omega_typer::{RankingView, TypingEnvironment, type_stage_in_environment};
    let run = |code: &str| {
        let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
        let inputs = [StageSource {
            package: "game",
            source: ParsedSource {
                logical_path: "rank.omg",
                text: code,
                tokens: &tokens,
                tree: &tree,
            },
        }];
        let resolved = resolve_stage(&inputs, &[]).unwrap();
        let byte = ValueType::Integer { signed: false, width: Some(8) };
        let views = [RankingView {
            declaration: resolved
                .declarations
                .iter()
                .find(|d| d.path == "Down")
                .unwrap()
                .clone(),
            subject: byte.clone(),
            parameters: vec![],
            rank: byte,
            canonical_default: true,
        }];
        type_stage_in_environment(
            &inputs,
            &resolved,
            &TypingEnvironment {
                ranking_views: &views,
                ..Default::default()
            },
        )
    };
    let source = "data Down{} machine f(x:u8) terminates by x -> Down in 0..=255 {} machine g(x:u8) terminates by x {}";
    let stage = run(source).unwrap();
    assert_eq!(stage.ranking_witnesses.len(), 2);
    assert_eq!(stage.ranking_witnesses[0].bounds.len(), 2);
    assert!(stage.ranking_witnesses[0].inclusive_upper);
    assert!(
        run(&source.replace("..=255", "..=256"))
            .unwrap_err()
            .message
            .contains("expected u8")
    );
    assert!(
        run(&source.replace("f(x:u8)", "f(x:bool)"))
            .unwrap_err()
            .message
            .contains("expected u8")
    );
}

#[test]
fn v1_crash_route_guards_type_as_total_facts_about_the_entry_value() {
    let source = "machine forward(value:i64)->i64 crashes Trap value == 0 {value}";
    let stage = check(&[("route.omg", source)]).unwrap();
    assert_eq!(stage.crash_routes[0].cause, omega_typer::CrashCause::Trap);
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
            &sources,
            &resolved,
            &TypingEnvironment {
                access_plan_requirement: resolved.declarations.iter().find(|d| d.path == name),
                ..Default::default()
            },
        )
    };
    let stage = run("AccessPolicy::plan").unwrap();
    assert_eq!(stage.access_policies[0].plan_machine.path, "Rule::plan");
    assert!(
        run("OtherPolicy::plan")
            .unwrap_err()
            .message
            .contains("does not supply")
    );
    assert!(run("").unwrap_err().message.contains("requires the admitted"));
}

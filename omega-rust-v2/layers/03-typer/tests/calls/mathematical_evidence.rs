#[test]
fn boundary_mathematics_retains_assumption_closure_through_private_helpers() {
    let definitions = "boundary let trusted(x:u8):u8; let wrap(x:u8):u8=trusted(x);";
    let stage = check(&[("math.omg", &format!("{definitions} machine proof(x:u8) ensures wrap(x)==x {{}}"))]).unwrap();
    let row = stage
        .mathematical_interfaces
        .iter()
        .find(|(id, ..)| id.path == "wrap")
        .unwrap();
    assert_eq!(row.2[0].path, "trusted");
    assert!(
        check(&[("math.omg", &format!("{definitions} machine execute(x:u8)->u8 {{wrap(x)}}"))])
            .unwrap_err()
            .message
            .contains("cannot supply executable")
    );
}

#[test]
fn witness_theorem_preserves_its_strict_goal_without_claiming_a_proof() {
    let witness = "data Witness<u:core::Level,v:core::Level,A:core::Type<u>,P:A->core::Strict<v>> requires P(value){value:A;}";
    let theorem = "machine follows<u:core::Level,v:core::Level,w:core::Level,A:core::Type<u>,machine Step>(P:A->core::Strict<v>,Q:A->core::Strict<w>,present:core::Squash<Witness<u,v,A,P>>) requires machine Step(value:A) requires P(value) ensures Q(value); ensures core::Squash::<Witness<u,w,A,Q>> {}";
    let code = format!("{witness} {theorem}");
    let stage = check(&[("theorem.omg", &code)]).unwrap();
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(&o.kind,omega_typer::ObligationKind::ProveGuarantee{machine,..} if machine.path=="follows"))
    );
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(&o.kind, omega_typer::ObligationKind::FormSquash { .. }))
    );
    assert!(stage.indirect_calls.is_empty());
}

#[test]
fn projected_evidence_keeps_the_conclusion_to_check() {
    let code = "data Bundle<P:u8->core::Strict<0>> { evidence:(x:u8)->P(x); } machine theorem<machine Step>(x:u8) requires machine Step(y:u8) ensures y==y; {} machine forward<P:u8->core::Strict<0>>(p:Bundle<P>,v:u8){theorem::<p.evidence>(v)}";
    let stage = check(&[("evidence.omg", code)]).unwrap();
    assert!(stage.obligations.iter().any(|o|matches!(&o.kind,omega_typer::ObligationKind::SupplyHypothesis{evidence:ValueType::ProofTerm(site),..} if site.span.slice(code.as_bytes())==Some(&b"p.evidence"[..]))));
    let initializer = format!(
        "{code} machine lemma(y:u8) requires y>0 ensures y==y {{}} machine bundle<P:u8->core::Strict<0>>()->Bundle<P>{{Bundle::<P>{{evidence:lemma}}}}"
    );
    let supplied = check(&[("citation.omg", &initializer)]).unwrap();
    assert!(
        supplied
            .obligations
            .iter()
            .any(|o| matches!(&o.kind,omega_typer::ObligationKind::RefineMathematicalCitation{machine,..} if machine.path=="lemma"))
    );
    assert!(!supplied.calls.iter().any(|c| c.machine.path == "lemma"));
    let wrong = code.replace("(x:u8)->P(x)", "(x:u8)->bool");
    assert!(
        check(&[("evidence.omg", &wrong)])
            .unwrap_err()
            .message
            .contains("expected evidence")
    );
}

#[test]
fn a_named_family_checks_nested_mathematical_subjects_in_declaration_order() {
    let code = "let keep(x:u8,p:predicate(inc(x))):predicate(inc(x))=p; let inc(x:u8):u8=x+1; let predicate(x:u8):core::Strict<0> = x>0;";
    check(&[("family.omg", code)]).unwrap();
    let wrong = code.replace("inc(x))=p", "inc(1u16))=p");
    assert!(
        check(&[("family.omg", &wrong)])
            .unwrap_err()
            .message
            .contains("wrong type")
    );
}

#[test]
fn dependent_arrows_apply_to_exact_subjects_and_ignore_binder_spelling() {
    let source = "let apply<A,F:A->core::Type<0>>(f:(x:A)->F(x),y:A):F(y)=f(y); let rename<A,F:A->core::Type<0>>(f:(x:A)->F(x)):(z:A)->F(z)=f;";
    check(&[("dependent.omg", source)]).unwrap();
    let bad = "let wrong<A,F:A->core::Type<0>>(f:(x:A)->F(x),a:A,b:A):F(b)=f(a);";
    assert!(
        check(&[("dependent.omg", bad)])
            .unwrap_err()
            .message
            .contains("expected")
    );
    let runtime = "data Buf<N:u8>{} let named(n:u8,b:Buf<n>):Buf<n>=b;";
    check(&[("dependent.omg", &runtime.replace("<n>=", "<n> ="))]).unwrap();
}

#[test]
fn mathematical_telescope_preserves_distinct_levels_and_strict_propositions() {
    let code = "let compose<u:core::Level,v:core::Level,w:core::Level,A:core::Type<u>,B:core::Type<v>,C:core::Type<w>>(f:B->C,g:A->B,value:A):C=f(g(value)); let positive(value:i32):core::Strict<0>=value>0; machine theorem(x:i32) requires positive(x) {}";
    let code = code.replace("<0>=", "<0> =");
    let stage = check(&[("math.omg", &code)]).unwrap();
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(&o.kind,omega_typer::ObligationKind::MathematicalFact{sort} if sort.to_string()=="core::Strict<0>"))
    );
    assert!(
        check(&[("math.omg", "machine f(x:core::Level){}")])
            .unwrap_err()
            .message
            .contains("binder kind")
    );
    assert!(
        check(&[("math.omg", "let bad<const N:u64>(x:u8):core::Strict<N> = x == 0;")])
            .unwrap_err()
            .message
            .contains("universe level")
    );
}

#[test]
fn a_machine_shaped_hypothesis_keeps_its_premises_and_cannot_be_executed() {
    let source = "machine theorem<machine Step>(x:u8) requires machine Step(y:u8) requires y != 0 ensures y > 0; ensures Step::ensures(x) {} machine lemma(y:u8) requires y != 0 ensures y > 0 {} machine use_theorem(x:u8){theorem::<lemma>(x)}";
    let stage = check(&[("proof.omg", source)]).unwrap();
    assert_eq!(stage.logical_hypotheses.len(), 1);
    assert_eq!(stage.logical_hypotheses[0].premises.len(), 1);
    assert_eq!(stage.logical_hypotheses[0].conclusions.len(), 1);
    assert!(stage.indirect_calls.is_empty());
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(o.kind, omega_typer::ObligationKind::SupplyHypothesis { .. }))
    );
    for (from, to, reason) in [
        ("ensures Step::ensures(x) {}", "{Step(x)}", "logical evidence cannot be executed"),
        ("machine lemma(y:u8)", "machine lemma(y:u16)", "incompatible logical evidence signature"),
        ("ensures y > 0;", "ensures 5u8;", "expected bool"),
    ] {
        let bad = source.replace(from, to);
        let error = check(&[("proof.omg", &bad)]).unwrap_err();
        assert!(error.message.contains(reason), "{error:?}");
    }
}

#[test]
fn tuples_mathematical_prefixes_and_write_only_places() {
    check(&[(
        "a.omg",
        "machine f(x:bool,y:bool)->bool{match (x,y) {(true,_)->true,(false,true)->true,(false,false)->false,}}",
    )])
    .unwrap();
    check(&[("a.omg", "let first(x:u8,y:bool):u8=x; let partial(x:u8):bool->u8=first(x);")]).unwrap();
    check(&[("a.omg", "data P { v:u8; } machine set(p:@write P){p.v=7;}")]).unwrap();
    let e = check(&[("a.omg", "data P { v:u8; } machine get(p:@write P)->u8{p.v}")]).unwrap_err();
    assert!(e.message.contains("write-only"));
}

#[test]
fn mathematical_generic_arguments_and_chained_prefixes_keep_the_checked_term() {
    let declarations = "let first<T>(x:T, y:bool):T=x; let identity<T>(x:T):T=x;";
    for expression in [
        "first::<u8>(5)(true)",
        "first(5u8,true)",
        "identity::<u8>(5)",
        "identity(5)",
    ] {
        let source = format!("{declarations} let term():u8={expression};");
        let stage = check(&[("math.omg", &source)]).unwrap();
        assert!(stage.obligations.iter().any(|o| matches!(&o.kind,
            omega_typer::ObligationKind::MathematicalApplication {definition:Some(_), generic_arguments, ..}
            if generic_arguments.len()==1 && generic_arguments[0].to_string()=="u8")));
        assert!(stage.indirect_calls.is_empty());
    }
    let source = format!("{declarations} let term():u8->bool->u8=first::<u8>;");
    check(&[("math.omg", &source)]).unwrap();
    for (expression, message) in [
        ("first::<u8>(true,true)", "expected u8"),
        ("first::<u8,bool>(5,true)", "excess generic arguments"),
        ("first::<u8>(5,true,false)", "excess mathematical arguments"),
        ("first(5u8)(7u8)", "expected bool"),
    ] {
        let source = format!("{declarations} let term():u8={expression};");
        let err = check(&[("math.omg", &source)]).unwrap_err();
        assert!(err.message.contains(message), "{expression}: {err:?}");
    }
}

#[test]
fn named_function_types_and_explicit_universe_arguments_work_through_calls() {
    for source in [
        "data Box<T>{} machine f(x:Box<(n:u8)->bool>)->Box<u8->bool>{x}",
        "machine id<T>(x:T)->T{x} machine f(x:(n:u8)->bool)->u8->bool{id::<(n:u8)->bool>(x)}",
        "machine id<A:core::Type<0>>(x:A)->A{x} machine f(x:u8)->u8{id::<u8>(x)}",
        "data X{} machine id<A:core::Type<0>>(x:A)->A{x} machine f(x:X)->X{id::<X>(x)}",
        "machine id<A:core::Type<0>>(x:A)->A{x} machine f<B:core::Type<0>>(x:B)->B{id::<B>(x)}",
    ] {
        check(&[("types.omg", source)]).unwrap();
    }
    check(&[(
        "domain.omg",
        "domain<A:core::Type<1>> u8::Tag<A>; machine member(x:u8)->bool{x in Tag::<core::Type<0>>}",
    )])
    .unwrap();
    for source in [
        "machine one<A:core::Type<0>>() requires A == u8 {}",
        "machine two<A:core::Type<0>,B:core::Type<0>>() requires A == B {}",
        "machine numeric<N:u8>() requires N == 0u8 {}",
    ] {
        check(&[("equality.omg", source)]).unwrap();
    }
    let wrong = "data Box<T>{} machine f<A:core::Type<0>,F:A->core::Type<0>>(x:Box<(a:A)->(b:A)->F(a)>)->Box<(a:A)->(b:A)->F(b)>{x}";
    assert!(
        check(&[("types.omg", wrong)])
            .unwrap_err()
            .message
            .contains("expected")
    );
}

#[test]
fn type_applications_require_the_same_universe_membership_as_machine_calls() {
    let prefix = "data Box<u:core::Level,A:core::Type<u>>{} data U=core::Type<0>;";
    let valid = format!("{prefix} machine f(x:Box<1,U>){{}}");
    let stage = check(&[("universe.omg", &valid)]).unwrap();
    let rows = stage
        .obligations
        .iter()
        .filter(|o| {
            matches!(&o.kind,omega_typer::ObligationKind::UniverseMembership {subject,sort}
        if subject.to_string()=="core::Type<0>" && sort.to_string()=="core::Type<1>")
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].site.span.slice(valid.as_bytes()).unwrap(), b"Box<1,U>");
    // Only the ordinary successor contains a universe. Ask the same question
    // through each construct that accepts an explicit type argument.
    for subject_level in 0..=3 {
        for bound_level in 0..=3 {
            let subject = format!("core::Type<{subject_level}>");
            let sort = format!("core::Type<{bound_level}>");
            let domain = format!("domain<A:{sort}> u8::Tag<A>;");
            for source in [
                format!("machine id<A:{sort}>(x:A)->A{{x}} machine f(x:{subject})->{subject}{{id::<{subject}>(x)}}"),
                format!("let id<A:{sort}>(x:A):A=x; let f(x:{subject}):{subject} = id::<{subject}>(x);"),
                format!("{prefix} machine f(x:Box<{bound_level},{subject}>){{}}"),
                format!("data U={subject}; data Box<A:{sort}>{{}} data Wrap<T> = Box<T>; machine f(x:Wrap<U>){{}}"),
                format!("{domain} machine f(x:u8 in Tag<{subject}>){{}}"),
                format!("{domain} machine f(x:u8::Tag<{subject}>){{}}"),
                format!("{domain} machine f(x:u8)->bool{{x in u8::Tag::<{subject}>}}"),
                format!("{domain} machine f(x:u8){{let tagged=x as +Tag<{subject}>;}}"),
            ] {
                let result = check(&[("universe.omg", &source)]);
                if bound_level == subject_level + 1 {
                    result.unwrap_or_else(|error| panic!("{source}: {error:?}"));
                } else {
                    assert_eq!(result.unwrap_err().message, "universe argument requires its exact successor universe", "{source}");
                }
            }
        }
    }
    let strict = "data StrictBox<A:core::Strict<1>>{} machine f(x:StrictBox<core::Type<0>>){}";
    assert_eq!(check(&[("universe.omg", strict)]).unwrap_err().message, "universe argument requires its exact successor universe");
}

#[test]
fn transparent_binder_kinds_preserve_argument_roles_and_universe_levels() {
    let aliases = "data Identity<T> = T; data Sort = Identity<core::Type<0>>; data High<L:core::Level> = core::Type<L>; data Byte = u8;";
    for bound in ["core::Type<0>", "Sort", "High<0>", "Identity<Sort>"] {
        for call in ["id(x)", "id::<u8>(x)"] {
            let machines = format!("machine id<A:{bound}>(x:A)->A{{x}} machine f(x:u8)->u8{{{call}}}");
            let ordered = [("a.omg", aliases), ("z.omg", machines.as_str())];
            let stage = check(&ordered).unwrap();
            let selected = stage
                .calls
                .iter()
                .find(|call| call.machine.path == "id")
                .unwrap();
            assert_eq!(
                selected
                    .type_arguments
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                ["u8"]
            );
            assert_eq!(stage, check(&[ordered[1], ordered[0]]).unwrap());
            check(&[("a.omg", &machines), ("z.omg", aliases)]).unwrap();
        }
    }
    let family = "data Family = u8::D::Child->core::Type<0>; domain u8::D; machine ready(x:u8 in D)->bool{true} domain u8::D::Child requires ready(self); machine f<F:Family>(){}";
    check(&[("family.omg", family)]).unwrap();
    let wrong_carrier = format!("{family} machine wrong(x:u16)->bool{{x in u8::D::Child}}");
    assert_eq!(check(&[("family.omg", &wrong_carrier)]).unwrap_err().message, "membership subject has the wrong carrier");
    let cyclic = "data A=B; data B=A; machine f<T:A>(){}";
    assert!(
        check(&[("cycle.omg", cyclic)])
            .unwrap_err()
            .message
            .contains("cyclic type alias")
    );
    let nominal = "data Sort{} machine f<A:Sort>(){}";
    assert!(check(&[("nominal.omg", nominal)]).is_err());
    let strict = "data S=core::Strict<1>; data Box<A:S>{} machine f(x:Box<core::Type<0>>){}";
    assert_eq!(
        check(&[("strict.omg", strict)]).unwrap_err().message,
        "universe argument requires its exact successor universe"
    );
    let indices = "data Buffer<N:Byte>{} machine f(x:Buffer<7>)->Buffer<7>{x}";
    check(&[("index.omg", indices), ("sort.omg", aliases)]).unwrap();
    for (level, accepted) in [(0, false), (1, true), (2, false)] {
        let code = format!("data Box<A:High<{level}>>{{}} machine f(x:Box<core::Type<0>>){{}}");
        let result = check(&[("box.omg", &code), ("sort.omg", aliases)]);
        if accepted {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err().message, "universe argument requires its exact successor universe");
        }
    }
}

#[test]
fn generated_alias_compositions_select_the_expected_binding() {
    use std::fmt::Write;

    // Both meanings of X are in scope. First and Last can nest, so two wrong
    // selections can cancel each other. Compare each tree to its selected leaf.
    let leaves = [("core::Type<0>".to_owned(), true), ("u8".to_owned(), false)];
    let mut kinds = leaves.to_vec();
    for _ in 0..2 {
        let children = kinds;
        kinds = leaves.to_vec();
        for (left, left_is_type) in &children {
            for (right, right_is_type) in &children {
                kinds.push((format!("First<{left},{right}>"), *left_is_type));
                kinds.push((format!("Last<{left},{right}>"), *right_is_type));
            }
        }
    }
    kinds.sort_by_key(|(bound, _)| bound.len());
    // Each call must resolve X independently, even when neighboring calls use the other meaning.
    let definitions = "data X{} data First<A,B> = A; data Last<A,B> = B;";
    for batch in kinds.chunks(32) {
        let mut source = definitions.to_owned();
        let mut body = String::from("machine caller<X:u8>(){");
        for (i, (bound, _)) in batch.iter().enumerate() {
            write!(&mut source, "\nmachine pick_{i}<T:{bound}>(){{}}").unwrap();
            write!(&mut body, "\npick_{i}::<X>();").unwrap();
        }
        source.push('\n');
        source.push_str(&body);
        source.push('}');
        let stage = check(&[("rule.omg", &source)]).unwrap_or_else(|e| panic!("{e:?}\n{source}"));
        assert_eq!(stage.calls.len(), batch.len());
        for (i, (call, (bound, takes_type))) in stage.calls.iter().zip(batch).enumerate() {
            assert_eq!(call.machine.path, format!("pick_{i}"), "{bound}");
            let [argument] = call.type_arguments.as_slice() else {
                panic!("{bound}\nexpected one argument")
            };
            let correct = if *takes_type {
                matches!(argument, ValueType::Nominal(id) if id.path == "X")
            } else {
                matches!(argument, ValueType::Parameter {owner, index:0} if owner.path == "caller")
            };
            assert!(correct, "{bound}\nexpected type role = {takes_type}, selected {argument:?}");
        }
    }
}

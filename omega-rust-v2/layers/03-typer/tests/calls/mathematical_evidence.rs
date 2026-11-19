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

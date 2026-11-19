//! Inspect the facts L3 passes to the checker at the source occurrence that requires them.
mod support;

use omega_typer::{Access, ObligationKind, Site, TypedStage, ValueType};

fn spelling<'a>(
    site: &Site,
    source: &'a str,
) -> &'a str {
    assert_eq!(site.package, "game");
    assert_eq!(site.file, "handoff.omg");
    std::str::from_utf8(site.span.slice(source.as_bytes()).unwrap()).unwrap()
}

fn expect_at(
    stage: &TypedStage,
    source: &str,
    operation: &str,
    expected: impl Fn(&ObligationKind) -> bool,
) {
    let obligations: Vec<_> = stage
        .obligations
        .iter()
        .filter(|o| spelling(&o.site, source) == operation)
        .map(|o| &o.kind)
        .collect();
    assert!(obligations.iter().any(|kind| expected(kind)), "{operation}: {obligations:?}");
}

#[test]
fn construction_distinguishes_omitted_fields_zero_bindings_and_moves() {
    let source = "data Options requires self.count > 0 { count:u8; retries:u16; } machine build(count:u8) -> Options { let spare:u32; let config = Options { count, .. }; move config }";
    let stage = support::check(&[("handoff.omg", source)]).unwrap();
    expect_at(
        &stage,
        source,
        "Options { count, .. }",
        |k| matches!(k, ObligationKind::EstablishData(id) if id.path=="Options"),
    );
    expect_at(
        &stage,
        source,
        "Options { count, .. }",
        |k| matches!(k, ObligationKind::ZeroField { field, ty } if field.path=="Options::retries" && field.kind==omega_parser::NodeKind::Field && ty.to_string()=="u16"),
    );
    expect_at(
        &stage,
        source,
        "let spare:u32;",
        |k| matches!(k, ObligationKind::ZeroValue(ty) if ty.to_string()=="u32"),
    );
    expect_at(
        &stage,
        source,
        "move config",
        |k| matches!(k, ObligationKind::Move {place,ty} if spelling(place,source)=="config" && ty.to_string()=="game::Options"),
    );
    let explicit = source
        .replace("count, ..", "count, retries: 0")
        .replace("let spare:u32;", "let spare:u32=1;");
    let explicit_stage = support::check(&[("handoff.omg", &explicit)]).unwrap();
    assert!(
        !explicit_stage
            .obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::ZeroField { .. } | ObligationKind::ZeroValue(..)))
    );
}

#[test]
fn identical_calls_keep_the_precondition_on_the_overload_that_requires_it() {
    let source = "machine take(x:u8) requires x>0 {} machine take(x:bool) {}
                  machine byte(x:u8){take(x)} machine flag(x:bool){take(x)}";
    let resolved = support::resolve_packages(&[("game", "handoff.omg", source)], &[]);
    let typed = omega_typer::type_stage(&resolved).unwrap();
    let tree = typed.trees().next().unwrap();
    let file = tree.resolved().file;
    let calls: Vec<_> = file
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, syntax)| syntax.kind == omega_parser::NodeKind::Call)
        .map(|(node, _)| node)
        .collect();
    let [byte_call, flag_call] = *calls.as_slice() else {
        panic!("expected two calls to take")
    };
    for (node, operand, requires) in [(byte_call, "u8", true), (flag_call, "bool", false)] {
        assert_eq!(file.span(node).slice(source.as_bytes()), Some(b"take(x)".as_slice()));
        let selected = tree
            .semantics_at(node)
            .find_map(|fact| match fact {
                omega_typer::NodeSemantics::Call(call) => Some(&call.machine),
                _ => None,
            })
            .unwrap();
        assert_eq!(selected.path, "take");
        assert_eq!(selected.operands, [operand]);
        let preconditions: Vec<_> = tree
            .semantics_at(node)
            .filter_map(|fact| match fact {
                omega_typer::NodeSemantics::Obligation(obligation) => match &obligation.kind {
                    ObligationKind::CallPrecondition(machine) => Some(machine),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        if requires {
            assert_eq!(preconditions, [selected]);
        } else {
            assert!(preconditions.is_empty(), "the bool overload has no precondition");
        }
    }
}

#[test]
fn borrows_recasts_reads_and_lifetime_bindings_keep_their_places() {
    let source = "machine bits(value:u32)->f32 { let view:@f32 = @value as @f32; view } data ByteCell {value:u8;} machine forward<'a>(input:@'a ByteCell)->@'a u8 {@input.value}";
    let stage = support::check(&[("handoff.omg", source)]).unwrap();
    expect_at(
        &stage,
        source,
        "@value",
        |k| matches!(k,ObligationKind::Borrow{access:Access::Shared,place} if spelling(place,source)=="value"),
    );
    expect_at(
        &stage,
        source,
        "@value as @f32",
        |k| matches!(k,ObligationKind::Recast{source,destination} if source.to_string()=="@u32" && destination.to_string()=="@f32"),
    );
    expect_at(&stage, source, "view", |k| matches!(k, ObligationKind::CopyRead(ValueType::Float32)));
    expect_at(
        &stage,
        source,
        "@input.value",
        |k| matches!(k,ObligationKind::ReturnBorrow{machine} if machine.path=="forward"),
    );
    expect_at(&stage, source, "@input.value", |k| {
        matches!(k,ObligationKind::BindLifetime{reference,destination:ValueType::AtLifetime(inner,region)}
        if inner.as_ref()==reference && reference.to_string()=="@u8" && matches!(region.as_ref(),ValueType::Parameter{owner,index:0} if owner.path=="forward"))
    });
}

#[test]
fn numeric_casts_and_domain_establishment_keep_distinct_subjects() {
    let source = "domain u16::Nonzero requires self>0; machine widen(x:u8,y:u8)->u16 in Nonzero { let doubled = x +% y; (doubled as u16) as +Nonzero }";
    let stage = support::check(&[("handoff.omg", source)]).unwrap();
    expect_at(
        &stage,
        source,
        "x +% y",
        |k| matches!(k,ObligationKind::OperationContract{token,operands} if token=="+%" && operands.len()==2 && operands.iter().all(|ty|ty.to_string()=="u8")),
    );
    expect_at(
        &stage,
        source,
        "doubled as u16",
        |k| matches!(k,ObligationKind::ExactCast{source,destination} if source.to_string()=="u8" && destination.to_string()=="u16"),
    );
    expect_at(
        &stage,
        source,
        "(doubled as u16) as +Nonzero",
        |k| matches!(k,ObligationKind::EstablishDomain(domain) if domain.declaration.path=="u16::Nonzero" && domain.arguments.is_empty()),
    );
}

#[test]
fn structural_callbacks_retain_the_actual_and_expected_contracts() {
    let source = "machine local(n:u8)->u8 {n} machine invoke<const F:machine(v:u8)->u8>(n:u8)->u8 {F(n)} machine direct(n:u8)->u8 {invoke::<local>(n)} machine adapt(g:machine(actual:u8)->u8 reaches none crashes none) {let f:machine(formal:u8)->u8 reaches none crashes none=g;}";
    let stage = support::check(&[("handoff.omg", source)]).unwrap();
    expect_at(&stage, source, "invoke::<local>(n)", |k| {
        matches!(k,ObligationKind::RefineCallable{contract,machine,..}
        if machine.path=="local" && spelling(&contract.contract,source)=="machine(v:u8)->u8" && contract.result.to_string()=="u8")
    });
    expect_at(&stage, source, "g", |k| {
        matches!(k,ObligationKind::RefineCallableValue{source:from,destination:to}
        if spelling(from,source)=="machine(actual:u8)->u8 reaches none crashes none" && spelling(to,source)=="machine(formal:u8)->u8 reaches none crashes none")
    });
}

#[test]
fn hypothesis_application_keeps_requirement_and_operand_order() {
    let requirement = "requires machine Step(left:u8,right:u8) ensures left>right;";
    let source = format!("machine theorem<machine Step>(x:u8,y:u8) {requirement} ensures Step::ensures(y,x) {{}}");
    let stage = support::check(&[("handoff.omg", &source)]).unwrap();
    expect_at(&stage, &source, "Step::ensures(y,x)", |k| {
        matches!(k,ObligationKind::ApplyHypothesis{requirement:site,operands}
        if spelling(site,&source)==requirement && operands.iter().map(|s|spelling(s,&source)).eq(["y","x"]))
    });
}

#[test]
fn copy_returns_read_the_value_without_returning_a_loan() {
    for (source, read_site) in [
        ("machine f()->u8 {let x:u8=7; let view=@x; view}", "view"),
        ("machine f()->u8 {let x:u8=7; let view=@x; let n:u8=view; n}", "view"),
        ("machine f(view:@u8)->u8 {view}", "view"),
        ("machine f<'a>(view:@'a u8)->u8 {view}", "view"),
        ("machine f<'a>(view:@'a mut u8)->u8 {let n:u8=view; n}", "view"),
        ("machine f()->bool {let flag:bool=true; @flag}", "@flag"),
    ] {
        let stage = support::check(&[("handoff.omg", source)]).unwrap();
        expect_at(&stage, source, read_site, |k| matches!(k, ObligationKind::CopyRead(_)));
        assert!(
            !stage
                .obligations
                .iter()
                .any(|o| matches!(o.kind, ObligationKind::ReturnBorrow { .. })),
            "{source}"
        );
    }
    for source in [
        "machine f(view:@write u8)->u8 {view}",
        "machine f<'a>(view:@'a write u8)->u8 {view}",
        "machine f<'a>(view:@'a u8)->bool {view}",
    ] {
        let error = support::check(&[("handoff.omg", source)]).unwrap_err();
        assert_eq!(error.span.slice(source.as_bytes()), Some(b"view".as_slice()));
        assert!(error.message.starts_with("expected "), "{error:?}");
    }
}

#[test]
fn terminal_transitions_record_the_borrow_they_forward() {
    let source = "machine f(left:@u8,right:@u8,b:bool)->@u8 {transition b {true->left,false->right,}}";
    let stage = support::check(&[("handoff.omg", source)]).unwrap();
    for arm in ["left", "right"] {
        expect_at(
            &stage,
            source,
            arm,
            |k| matches!(k, ObligationKind::ReturnBorrow { machine } if machine.path=="f"),
        );
    }
    let copied = source.replace(")->@u8", ")->u8");
    let stage = support::check(&[("handoff.omg", &copied)]).unwrap();
    for arm in ["left", "right"] {
        expect_at(&stage, &copied, arm, |k| matches!(k, ObligationKind::CopyRead(_)));
    }
    assert!(
        !stage
            .obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::ReturnBorrow { .. }))
    );

    let local = "machine f(x:u8,right:@u8,b:bool)->@u8 {transition b {true->@x,false->right,}}";
    let error = support::check(&[("handoff.omg", local)]).unwrap_err();
    assert_eq!(error.message, "returned reference borrows invocation-local storage");
    assert_eq!(error.span.slice(local.as_bytes()), Some(b"@x".as_slice()));
}

#[test]
fn bare_arm_bindings_cover_the_subject_and_qualified_constants_test_one_value() {
    let sibling = ("sibling.omg", "const x:bool=true;");
    let limits = ("limits/values.omg", "const x:u32=7;");
    for dispatch in ["match", "transition"] {
        let bare = format!("machine f(v:u32)->u32 {{ {dispatch} v {{ x -> x }} }}");
        support::check(&[("handoff.omg", &bare)]).unwrap();
        support::check(&[("handoff.omg", &bare), sibling]).unwrap();
        let compare = format!("machine f(v:u32)->u32 {{ {dispatch} v {{ limits::x -> 1 }} }}");
        let error = support::check(&[("handoff.omg", &compare), limits]).unwrap_err();
        assert_eq!(error.message, "dispatch does not cover its subject");
        let covered = compare.replace("limits::x -> 1", "limits::x -> 1, x -> x");
        support::check(&[("handoff.omg", &covered), limits, sibling]).unwrap();

        let guard = format!("machine f(ready:bool)->u32 {{ {dispatch} {{ ready -> 1, _ -> 0 }} }}");
        support::check(&[("handoff.omg", &guard)]).unwrap();
        let wrong = guard.replace("ready:bool", "ready:u8");
        let error = support::check(&[("handoff.omg", &wrong)]).unwrap_err();
        assert_eq!(error.message, "expected bool, found u8");
        assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"ready".as_slice()));

        let literals = format!("machine f(true:u32, false:u32, v:bool)->u32 {{ {dispatch} v {{ true -> 1, false -> 0 }} }}");
        support::check(&[("handoff.omg", &literals)]).unwrap();
    }
}

#[test]
fn value_patterns_read_a_copy_through_a_reference_subject() {
    // domains.md: reading a copyable primitive through a readable reference
    // needs no cast. A value pattern compares the referent, as a case pattern
    // already does, and the subject that is read records the copy.
    for (source, read_site, copied) in [
        ("machine f(v:@bool)->u8 {match v {true->1,false->0,}}", "v", "bool"),
        ("machine f(v:@mut bool)->u8 {match v {true->1,false->0,}}", "v", "bool"),
        ("machine f(v:@u8)->u8 {match v {0->1,_->0,}}", "v", "u8"),
        ("machine f(v:@u8)->u8 {match v {limits::Z->1,_->0,}}", "v", "u8"),
        ("machine f<'a>(v:@'a u8)->u8 {match v {0->1,_->0,}}", "v", "u8"),
        ("machine f(a:@bool,b:u8)->u8 {match (a,b) {(true,_)->1,(false,_)->0,}}", "a", "bool"),
        ("data V {r:@u8;} machine f(h:V)->u8 {match h {V {r:0}->1,V {r as _}->0,}}", "r:0", "u8"),
    ] {
        let stage = support::check(&[("handoff.omg", source), ("limits/values.omg", "const Z:u8=0;")]).unwrap_or_else(|e| panic!("{source}: {e:?}"));
        expect_at(
            &stage,
            source,
            read_site,
            |k| matches!(k, ObligationKind::CopyRead(ty) if ty.to_string()==copied),
        );
    }
    // A binder keeps the reference itself, so returning it forwards the loan.
    let source = "machine f(v:@u8)->@u8 {match v {r->r,}}";
    let stage = support::check(&[("handoff.omg", source)]).unwrap();
    assert!(
        !stage
            .obligations
            .iter()
            .any(|o| matches!(o.kind, ObligationKind::CopyRead(_)))
    );
    for (source, site, message) in [
        (
            "machine f(v:@write bool)->u8 {match v {true->1,false->0,}}",
            "true",
            "cannot read through a write-only reference",
        ),
        ("machine f(v:@u8)->u8 {match v {true->1,_->0,}}", "true", "expected u8, found bool"),
    ] {
        let error = support::check(&[("handoff.omg", source)]).unwrap_err();
        assert_eq!(error.span.slice(source.as_bytes()), Some(site.as_bytes()), "{source}: {error:?}");
        assert_eq!(error.message, message, "{source}");
    }
    let record = "machine f(v:@limits::I)->u8 {match v {limits::K->1,_->0,}}";
    let error = support::check(&[
        ("handoff.omg", record),
        ("limits/values.omg", "data I {n:u8;} const K:I=I {n:0};"),
    ])
    .unwrap_err();
    assert_eq!(error.span.slice(record.as_bytes()), Some(b"v".as_slice()));
    assert_eq!(error.message, "expected game::limits::I, found @game::limits::I");
    // An exact reference overload still wins over the by-value one.
    let source = "machine take(n:u8)->u8 {n} machine take(n:@u8)->u8 {0u8} machine f(v:@u8)->u8 {take(v)}";
    let stage = support::check(&[("handoff.omg", source)]).unwrap();
    assert_eq!(stage.calls[0].machine.operands, ["@ u8"]);
}

#[test]
fn callback_guarantees_retain_the_static_binder_and_subjects_without_invoking_it() {
    let requirement = "trait Step{machine run(x:u8)->u8 ensures result==x;}";
    let alias = "data Callback=machine(x:u8)->u8 reaches none crashes none ensures result==x;";
    for bound in ["Step::run", "Callback", "machine(x:u8)->u8 ensures result==x"] {
        let code = format!("{requirement} {alias} machine wrap<const F:{bound}>(x:u8)->u8 ensures F::ensures(x,result) {{F(x)}}");
        let checked = support::check(&[("handoff.omg", &code)]).unwrap();
        let [call] = checked.indirect_calls.as_slice() else {
            panic!("expected only the callback in the body")
        };
        assert_eq!(call.site.span.slice(code.as_bytes()), Some(b"F(x)".as_slice()));
        expect_at(&checked, &code, "F::ensures(x,result)", |kind| {
            matches!(kind,
            ObligationKind::ApplyCallbackGuarantee { binder, contract, operands }
                if matches!((&call.entry, binder),
                    (omega_typer::InvocationTarget::StaticParameter { owner, index }, ValueType::Parameter { owner: formal, index: i })
                    if owner == formal.as_ref() && index == i) && contract == &call.contract.contract
                && operands.iter().map(|s| s.span.slice(code.as_bytes()).unwrap()).eq([b"x".as_slice(), b"result"]))
        });
    }
    for (bound, body, reason) in [
        ("u8", "ensures F::ensures(x,result) {x}", "static binder requires a callable contract"),
        ("machine(x:u8)->u8", "ensures F::ensures(x,result) {x}", "no declared guarantee"),
        ("Step::run", "{F::ensures(x,x)}", "logical evidence cannot be executed"),
        ("Step::run", "ensures F::ensures(x) {x}", "declared subjects"),
    ] {
        let code = format!("{requirement} machine wrap<const F:{bound}>(x:u8)->u8 {body}");
        let error = support::check(&[("handoff.omg", &code)]).unwrap_err();
        assert!(error.message.contains(reason), "{bound}: {error:?}");
    }
}

#[test]
fn omitted_case_fields_retain_their_exact_declaration_and_instantiated_type() {
    let source = "data Sum<T>{case A(value:T);case B(value:bool);} machine a()->Sum<u8>{Sum::<u8>::A{..}} machine b()->Sum<u8>{Sum::<u8>::B{..}}";
    let stage = support::check(&[("handoff.omg", source)]).unwrap();
    for (case, ty) in [("A", "u8"), ("B", "bool")] {
        expect_at(&stage, source, &format!("Sum::<u8>::{case}{{..}}"), |kind| {
            matches!(kind, ObligationKind::ZeroField{field, ty: actual}
            if field.path == format!("Sum::{case}::value") && actual.to_string() == ty)
        });
    }
}

#[test]
fn exclusive_move_obligations_identify_the_evaluated_subject_and_field() {
    let definitions = "data Resource{n:u8;} data Holder{left:Resource;right:Resource;}";
    for (body, operand, field, alias) in [
        ("let (slot,_) = ((h.left,h.right)); let old=slot; slot=replacement; old", "slot", "left", true),
        ("let old=h.right; h.right=replacement; old", "h.right", "right", false),
        ("let old=(move h.left); h.left=replacement; old", "h.left", "left", false),
        ("let Holder{left as slot,..}=h; let old=slot; slot=replacement; old", "slot", "left", true),
    ] {
        let source = format!("{definitions} machine replace(h:@mut Holder,replacement:Resource)->Resource{{{body}}}");
        let resolved = support::resolve_packages(&[("game", "handoff.omg", &source)], &[]);
        let typed = omega_typer::type_stage(&resolved).unwrap();
        let tree = typed.trees().next().unwrap();
        let file = tree.resolved().file;
        let mut restores = typed.obligations.iter().filter_map(|o| match &o.kind {
            ObligationKind::RestoreBorrowed { place, ty } => Some((*place, ty)),
            _ => None,
        });
        let (taken, ty) = restores.next().expect(body);
        assert!(restores.next().is_none(), "{body}");
        assert_eq!(ty.to_string(), "game::Resource");
        assert_eq!(file.span(taken.node).slice(source.as_bytes()), Some(operand.as_bytes()));
        let store = file
            .nodes
            .iter()
            .find(|node| node.kind == omega_parser::NodeKind::Assign)
            .unwrap()
            .children[0];
        let (capture, path) = typed.place_path(taken);
        let (written, destination) = typed.place_path(file.node_id(store));
        let [step] = path.as_slice() else { panic!("{path:?}") };
        assert_eq!(step.through, Some(Access::Mut));
        assert!(matches!(&step.projection, Some(omega_typer::Projection::Field(id)) if id.path == format!("Holder::{field}")));
        assert_eq!(destination.len(), 1);
        assert_eq!(destination[0].projection, step.projection);
        assert_eq!(file.span(capture.node).slice(source.as_bytes()), Some(b"h".as_slice()));
        assert_eq!(tree.resolved().symbol(written.node), tree.resolved().symbol(capture.node));
        assert_eq!(written == capture, alias);
    }
    for body in [
        "machine f(h:@mut Holder)->u8{h.left.n}",
        "machine f(h:@mut Holder)->@Resource{@h.left}",
        "machine f(h:Holder)->Resource{move h.left}",
    ] {
        let source = format!("{definitions} {body}");
        let typed = support::check(&[("handoff.omg", &source)]).unwrap();
        assert!(
            typed
                .obligations
                .iter()
                .all(|o| !matches!(o.kind, ObligationKind::RestoreBorrowed { .. })),
            "{body}"
        );
    }
}

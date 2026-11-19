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
        |k| matches!(k, ObligationKind::ZeroField(id,field) if id.path=="Options" && field=="retries"),
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
            .any(|o| matches!(o.kind, ObligationKind::ZeroField(..) | ObligationKind::ZeroValue(..)))
    );
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
        matches!(k,ObligationKind::RefineCallable{contract,machine}
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

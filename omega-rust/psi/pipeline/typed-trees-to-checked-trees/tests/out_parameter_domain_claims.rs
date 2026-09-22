//! Out-parameter domain establishment joins.
//!
//! `ensures <&mut parameter> in <domain>` is the out-parameter direction of
//! `ensures result in <domain>`: a provider requirement mints the membership
//! into caller storage it borrows mutably, and the call-ensures join rejoins
//! the claim on the exact argument place. These tests witness the join across
//! validation, boundary authorization, provisional claim publication, and the
//! call-results check, plus the rejection cases that keep immutable, shared,
//! and owned-mutable parameters as caller premises.

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    typed_trees_to_checked_trees::lower_typed_trees(
        typed(source)?,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
}

fn typed(source: &str) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(
            std::path::PathBuf::from("out_parameter_domain_claims.omg"),
            source.to_owned(),
        )
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let syntax = syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
        syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest::new(syntax),
    )?;
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
        },
    )?;
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])
}

fn assert_checks(source: &str) {
    if let Err(diagnostics) = check(source) {
        panic!("expected the program to check cleanly, got: {diagnostics:#?}");
    }
}

fn assert_rejects(source: &str) -> Vec<diagnostics::Diagnostic> {
    match check(source) {
        Ok(_) => panic!("expected the program to be rejected, but it checked cleanly"),
        Err(diagnostics) => diagnostics,
    }
}

#[test]
fn out_parameter_ensures_establishes_through_boundary_route() {
    // The join itself: `established by Filler::fill` authorizes the boundary
    // requirement's `ensures buf in Buf::Full`; `consume` then sees the claim
    // on the exact `&mut` argument place after the call.
    assert_checks(
        r#"
data Buf { length: u64; }
domain Buf::Full established by Filler::fill;
boundary trait Filler {
    machine fill(buf: &mut Buf) ensures buf in Buf::Full;
}
machine consume(x: Buf in Full) -> u64 { x.length }
machine use_fill(filler: &Filler, buf: &mut Buf) reaches Filler {
    filler.fill(buf);
    let _r: u64 = consume(buf);
}
"#,
    );
}

#[test]
fn out_parameter_ensures_checked_body_machine_passes() {
    // A checked-body callee that delegates to the route provider owes its
    // authored parameter claim at every exit; the caller-side provisional
    // claim joins through the checked-body pass.
    assert_checks(
        r#"
data Buf { length: u64; }
domain Buf::Full established by Filler::fill;
boundary trait Filler {
    machine fill(buf: &mut Buf) ensures buf in Buf::Full;
}
machine fill_wrapper(filler: &Filler, buf: &mut Buf) ensures buf in Buf::Full reaches Filler {
    filler.fill(buf);
}
machine consume(x: Buf in Full) -> u64 { x.length }
machine use_fill(filler: &Filler, buf: &mut Buf) reaches Filler {
    fill_wrapper(filler, buf);
    let _r: u64 = consume(buf);
}
"#,
    );
}

#[test]
fn immutable_parameter_ensures_subject_is_rejected() {
    // The requirement authorizes `Full` through its own result, so route
    // resolution passes — validation still refuses `ensures` on an immutable
    // parameter: it is a caller premise.
    let diagnostics = assert_rejects(
        r#"
data Buf { length: u64; }
domain Buf::Full established by Filler::fill;
boundary trait Filler {
    machine fill(buf: Buf) -> Buf ensures result in Buf::Full ensures buf in Buf::Full;
}
"#,
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            format!("{diagnostic:?}").contains("only for its exact `result`")
        }),
        "expected the boundary-subject diagnostic, got: {diagnostics:#?}",
    );
}

#[test]
fn owned_mutable_parameter_ensures_subject_is_rejected() {
    // `mut buf: Buf` is callee-owned storage — nothing it mints reaches the
    // caller, so only a `&mut` borrow may be an establishment subject.
    let diagnostics = assert_rejects(
        r#"
data Buf { length: u64; }
domain Buf::Full established by Filler::fill;
boundary trait Filler {
    machine fill(mut buf: Buf) -> Buf ensures result in Buf::Full ensures buf in Buf::Full;
}
"#,
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            format!("{diagnostic:?}").contains("only for its exact `result`")
        }),
        "expected the boundary-subject diagnostic, got: {diagnostics:#?}",
    );
}

#[test]
fn shared_parameter_ensures_subject_is_rejected() {
    // A `&` (shared) borrow cannot write caller storage either.
    let diagnostics = assert_rejects(
        r#"
data Buf { length: u64; }
domain Buf::Full established by Filler::fill;
boundary trait Filler {
    machine fill(buf: &Buf) -> Buf ensures result in Buf::Full ensures buf in Buf::Full;
}
"#,
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            format!("{diagnostic:?}").contains("only for its exact `result`")
        }),
        "expected the boundary-subject diagnostic, got: {diagnostics:#?}",
    );
}

#[test]
fn out_parameter_claim_rejects_without_a_naming_route() {
    // The domain is routed but the route names `Sealer::seal`, not
    // `Filler::fill`: nothing authorizes the fill requirement to mint `Full`,
    // so no caller-side claim may join.
    assert_rejects(
        r#"
data Buf { length: u64; }
domain Buf::Full established by Sealer::seal;
boundary trait Sealer {
    machine seal(buf: &mut Buf) ensures buf in Buf::Full;
}
boundary trait Filler {
    machine fill(buf: &mut Buf) ensures buf in Buf::Full;
}
machine consume(x: Buf in Full) -> u64 { x.length }
machine use_fill(filler: &Filler, buf: &mut Buf) reaches Filler {
    filler.fill(buf);
    let _r: u64 = consume(buf);
}
"#,
    );
}

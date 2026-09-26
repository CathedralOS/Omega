use crate::CheckingRequest;
use crate::flow::check_against_whole_pass as lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;
use crate::tests::front_end::typed_program_with_core_service;
use facts::{FactOrigin, FactPayload};
use language_semantics::{DomainEstablishmentRoute, QualificationEvidenceOrigin};

/// `Binding<R>` carrier fixtures check against the toolchain declaration with
/// the same fused-service erasure authorizations `settle_checked_providers`
/// binds in real builds; without them the carrier field stays unshaped and the
/// machine's unit plan fails closed.
fn check_service_source(
    source: &str,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let mut typed = typed_program_with_core_service(source);
    crate::tests::bind_fixture_fused_service_erasures(&mut typed);
    lower_typed_trees(typed, &CheckingRequest::settled())
}

#[test]
fn selected_payload_qualification_reaches_direct_return_construction() {
    let source = r#"
data Token { identity: u64; }
domain Token::Issued established by Issuer::issue;
boundary trait Issuer { machine issue() -> Token in Issued; }
data Outcome { case Qualified(value: Token in Issued); case Empty; }
data Receipt { value: Token in Issued; }
machine rebuild(outcome: Outcome, fallback: Token in Issued) -> Receipt {
    transition outcome {
        Outcome::Qualified { value } -> Receipt { value: value }
        Outcome::Empty -> Receipt { value: fallback }
    }
}
"#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("the selected payload's routed qualification reaches its constructor");
    let wrong_case = source.replace(
        "Outcome::Empty -> Receipt { value: fallback }",
        "Outcome::Empty -> Receipt { value: outcome.value }",
    );
    let diagnostics =
        lower_typed_trees(parse_typed_trees(&wrong_case), &CheckingRequest::settled())
            .expect_err("the sibling arm cannot reuse the qualified arm's selection");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("construction of `Receipt`")),
        "{diagnostics:#?}"
    );
}

#[test]
fn case_payloads_transport_owned_qualifications_through_return_and_dispatch() {
    let source = r#"
data Token [linear] { identity: u64; }
domain Token::Issued established by Issuer::issue;
domain Token::Vacant;
boundary trait Issuer { machine issue() -> Token in Issued; }
data Outcome {
    case Accepted(value: Token in Issued & Vacant);
    case Rejected(original: Token in Issued & Vacant);
}
machine choose(token: Token in Issued & Vacant, accept: bool) -> Outcome {
    transition accept {
        true -> Outcome::Accepted { value: token }
        _ -> Outcome::Rejected { original: token }
    }
}
machine consume(token: Token in Issued & Vacant) -> Token { token as Token }
machine exercise(token: Token in Issued & Vacant, accept: bool) -> Token {
    let outcome: Outcome = choose(token, accept);
    transition outcome {
        Outcome::Accepted { value } -> consume(value)
        Outcome::Rejected { original } -> consume(original)
    }
}
"#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("each selected result payload retains the exact owned qualifications");
    let forwarded = source
        .replace(
            "machine consume(",
            "machine forward(outcome: Outcome) -> Outcome { outcome }\nmachine consume(",
        )
        .replace(
            "let outcome: Outcome = choose(token, accept);",
            "let chosen: Outcome = choose(token, accept); let outcome: Outcome = forward(chosen);",
        );
    lower_typed_trees(parse_typed_trees(&forwarded), &CheckingRequest::settled())
        .expect("a whole-sum wrapper retains conditional payload contracts");
    let copied = source.replace(
        "transition outcome {",
        "let moved: Outcome = outcome; transition moved {",
    );
    lower_typed_trees(parse_typed_trees(&copied), &CheckingRequest::settled())
        .expect("an owned sum move preserves exact case paths");
}

#[test]
fn a_qualified_scalar_return_requires_its_source_case() {
    let source = r#"
domain u64::Secret established by Issuer::issue;
boundary trait Issuer { machine issue() -> u64 in Secret; }
data Outcome { case Qualified(value: u64 in Secret); case Raw(raw: u64); }
machine bad(outcome: Outcome) -> u64 in Secret { outcome.value }
"#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .map(|_| ())
        .expect_err("a scalar qualification cannot escape from an unselected payload");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("selected case")),
        "{diagnostics:#?}"
    );
    let explicit = source.replace(
        "-> u64 in Secret { outcome.value }",
        "-> u64 ensures result in Secret { outcome.value as u64 }",
    );
    let diagnostics = lower_typed_trees(parse_typed_trees(&explicit), &CheckingRequest::settled())
        .map(|_| ())
        .expect_err("an explicit result promise cannot bypass source-case selection");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove")),
        "{diagnostics:#?}"
    );
}

#[test]
fn exclusive_boundary_receiver_establishes_its_exact_routed_result() {
    let source = r#"
pub data Guard [linear] {
    identity: u64;
}

pub domain Guard::Active
established by MaskControl::save;

pub boundary trait MaskControl {
    machine save(&mut self) -> Guard in Active
    ensures
        result in Guard::Active;
}

data Main {
    control: Binding<MaskControl>;
}

machine Main::run(&mut self) -> Guard in Active reaches MaskControl {
    self.control.save()
}
"#;

    let checked = check_service_source(source)
        .expect("an exclusive boundary receiver should establish its routed result");
    let control = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "MaskControl")
        .expect("mask-control boundary trait");
    let save = checked
        .trait_machine_signatures(control)
        .iter()
        .find(|signature| signature.name.as_str() == "save")
        .expect("save requirement");
    let active = checked
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Guard::Active")
        .expect("Active domain");
    assert_eq!(
        active.establishment_routes,
        [DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait: control.symbol,
            requirement: save.symbol,
        }]
    );
    let call_fact = checked
        .facts
        .semantic
        .facts
        .iter()
        .map(|(_, fact)| fact)
        .find(|fact| {
            fact.origin == FactOrigin::CallEnsures
                && matches!(
                    fact.payload,
                    FactPayload::ContractDomainMembership { domain_symbol, .. }
                        if domain_symbol == active.symbol
                )
        })
        .expect("save call should materialize Active membership");
    assert_eq!(
        call_fact.evidence.origin,
        QualificationEvidenceOrigin::AdmittedReceipt
    );
    assert_eq!(call_fact.evidence.source_symbol, control.symbol);
    assert_eq!(call_fact.evidence.requirement_symbol, save.symbol);
}

#[test]
fn boundary_carry_permission_is_admitted_and_transfers_by_exact_atom() {
    let source = r#"
pub data Token [linear] {
    value: u64;
}

pub boundary trait TokenIssuer {
    machine issue(value: u64) -> Token
    ensures
        result in Carry::MovableAddress;
}

data Main {
    issuer: Binding<TokenIssuer>;
}

machine Main::consume(&self, token: Token in Carry::MovableAddress) -> Token {
    token
}

machine Main::run(&mut self) -> Token reaches TokenIssuer {
    let token: Token = self.issuer.issue(7);
    let returned: Token = self.consume(token);
    transition { _ -> returned }
}
"#;

    let checked = check_service_source(source)
        .expect("an exact boundary result may admit one compiler carry permission");
    let issuer = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "TokenIssuer")
        .expect("issuer trait");
    let issue = checked
        .trait_machine_signatures(issuer)
        .iter()
        .find(|signature| signature.name.as_str() == "issue")
        .expect("issue requirement");

    let call_fact = checked
        .facts
        .semantic
        .facts
        .iter()
        .map(|(_, fact)| fact)
        .find(|fact| {
            fact.origin == FactOrigin::CallEnsures
                && matches!(
                    fact.payload,
                    FactPayload::ContractCarryPermission {
                        permission: language_semantics::CarryPermission::MovableAddress,
                        ..
                    }
                )
        })
        .expect("authorized call carry guarantee is materialized");
    assert_eq!(
        call_fact.evidence.origin,
        QualificationEvidenceOrigin::AdmittedReceipt
    );
    assert_eq!(call_fact.evidence.source_symbol, issuer.symbol);
    assert_eq!(call_fact.evidence.requirement_symbol, issue.symbol);

    let transferred = checked
        .facts
        .semantic
        .facts
        .iter()
        .map(|(_, fact)| fact)
        .find(|fact| {
            fact.origin == FactOrigin::StatementTransfer
                && matches!(
                    fact.payload,
                    FactPayload::CarryPermission {
                        permission: language_semantics::CarryPermission::MovableAddress,
                        ..
                    }
                )
        })
        .expect("assignment transfer preserves the carry permission");
    assert_eq!(transferred.evidence, call_fact.evidence);
}

#[test]
fn exact_machine_route_authorizes_its_own_invocation() {
    let source = r#"
data Token {
    value: u64;
}

domain Token::Issued
established by Token::issue;

machine Token::issue(value: u64) -> Token
ensures
    result in Token::Issued
{
    Token { value: value }
}

data Main {
}

machine Main::consume(&self, token: Token)
requires
    token in Token::Issued
{
}

machine Main::run(&mut self) {
    let token: Token = Token::issue(7);
    self.consume(token);
}
"#;

    let checked = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("the exact-machine route may establish its own result domain");
    let issued = checked
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Issued")
        .expect("issued domain");
    let issuer = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Token::issue")
        .expect("issuer machine");
    assert_eq!(
        issued.establishment_routes,
        [DomainEstablishmentRoute::ExactMachine {
            machine: issuer.symbol,
        }]
    );
    let call_fact = checked
        .facts
        .semantic
        .facts
        .iter()
        .map(|(_, fact)| fact)
        .find(|fact| {
            fact.origin == FactOrigin::CallEnsures
                && matches!(
                    fact.payload,
                    FactPayload::ContractDomainMembership { domain_symbol, .. }
                        if domain_symbol == issued.symbol
                )
        })
        .expect("call ensure membership");
    assert_eq!(
        call_fact.evidence.origin,
        QualificationEvidenceOrigin::AuthorizedRouteEstablishment
    );
}

use crate::flow::check_against_whole_pass as lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;
use facts::{FactOrigin, FactPayload};
use language_semantics::{DomainEstablishmentRoute, QualificationEvidenceOrigin};

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
    lower_typed_trees(parse_typed_trees(source))
        .expect("the selected payload's routed qualification reaches its constructor");
    let wrong_case = source.replace(
        "Outcome::Empty -> Receipt { value: fallback }",
        "Outcome::Empty -> Receipt { value: outcome.value }",
    );
    let diagnostics = lower_typed_trees(parse_typed_trees(&wrong_case))
        .expect_err("the sibling arm cannot reuse the qualified arm's selection");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("construction of `Receipt`")),
        "{diagnostics:#?}"
    );
}

#[test]
fn constructor_qualification_observes_operand_call_order() {
    let source = r#"
domain u64::Secret established by Issuer::issue;
boundary trait Issuer { machine issue() -> u64 in Secret; }
data Receipt { stamp: u64; value: u64 in Secret; }
boundary trait Mutator {
    machine clear(value: &mut u64) -> u64;
}
machine rebuild(mutator: &Mutator, input: u64)
requires input in u64::Secret
reaches Mutator {
    let mut value: u64 = input;
    let receipt: Receipt = Receipt { stamp: mutator.clear(&mut value), value: value };
}
"#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an earlier operand call invalidates qualification before capture");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("construction of `Receipt`")),
        "{diagnostics:#?}"
    );
    let selected = source.replace(
        "stamp: mutator.clear(&mut value), value: value",
        "stamp: 0, value: value",
    );
    lower_typed_trees(parse_typed_trees(&selected))
        .expect("without mutation the copied value retains qualification");
    let captured_first = source.replace(
        "stamp: mutator.clear(&mut value), value: value",
        "value: value, stamp: mutator.clear(&mut value)",
    );
    lower_typed_trees(parse_typed_trees(&captured_first))
        .expect("a later operand call cannot revoke an already captured scalar field");
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
    lower_typed_trees(parse_typed_trees(source))
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
    lower_typed_trees(parse_typed_trees(&forwarded))
        .expect("a whole-sum wrapper retains conditional payload contracts");
    let copied = source.replace(
        "transition outcome {",
        "let moved: Outcome = outcome; transition moved {",
    );
    lower_typed_trees(parse_typed_trees(&copied))
        .expect("an owned sum move preserves exact case paths");
}

#[test]
fn inactive_case_payload_cannot_supply_a_qualification() {
    let source = r#"
data Token [linear] { identity: u64; }
domain Token::Issued established by Issuer::issue;
boundary trait Issuer { machine issue() -> Token in Issued; }
data Outcome {
    case Qualified(value: Token in Issued);
    case Raw(raw: Token);
}
machine wrap(token: Token) -> Outcome { Outcome::Raw { raw: token } }
machine consume(token: Token in Issued) -> Token { token as Token }
machine exercise(token: Token) -> Token {
    let outcome: Outcome = wrap(token);
    consume(outcome.value)
}
"#;
    lower_typed_trees(parse_typed_trees(source))
        .expect_err("the absent qualified alternative cannot authorize raw custody");
    let affine = source.replace("Token [linear]", "Token");
    let diagnostics = lower_typed_trees(parse_typed_trees(&affine))
        .map(|_| ())
        .expect_err("case qualification selection is required even without linear debt");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn replacing_a_sum_retires_its_payload_qualification() {
    let source = r#"
data Token { identity: u64; }
domain Token::Issued established by Issuer::issue;
boundary trait Issuer { machine issue() -> Token in Issued; }
data Outcome { case Qualified(value: Token in Issued); case Raw(raw: Token); }
machine consume(token: Token in Issued) -> Token { token as Token }
machine exercise(token: Token in Issued, raw: Token) -> Token {
    let mut outcome: Outcome = Outcome::Qualified { value: token };
    outcome = Outcome::Raw { raw: raw };
    consume(outcome.value)
}
"#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .map(|_| ())
        .expect_err("replacing the sum cannot retain its old selected case");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
    let valid = source.replace("outcome = Outcome::Raw { raw: raw };", "");
    lower_typed_trees(parse_typed_trees(&valid))
        .expect("an unchanged constructed tag authorizes its own payload");
}

#[test]
fn case_construction_cannot_mint_a_payload_qualification() {
    let source = r#"
data Token [linear] { identity: u64; }
domain Token::Issued established by Issuer::issue;
boundary trait Issuer { machine issue() -> Token in Issued; }
data Outcome { case Qualified(value: Token in Issued); case Empty; }
machine wrap(token: Token) -> Outcome { Outcome::Qualified { value: token } }
"#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a case field declaration is not issuer evidence");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")
                || diagnostic.message.contains("cannot prove")),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_qualified_scalar_return_requires_its_source_case() {
    let source = r#"
domain u64::Secret established by Issuer::issue;
boundary trait Issuer { machine issue() -> u64 in Secret; }
data Outcome { case Qualified(value: u64 in Secret); case Raw(raw: u64); }
machine bad(outcome: Outcome) -> u64 in Secret { outcome.value }
"#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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
    let diagnostics = lower_typed_trees(parse_typed_trees(&explicit))
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
fn checked_boundary_adapter_defers_issuance_to_the_boundary_call() {
    let source = r#"
data Token { value: u64; }
domain Token::Issued established by TokenIssuer::issue;
boundary trait TokenIssuer {
    machine issue(token: Token) -> Token ensures result in Token::Issued;
}
data Adapter {}
machine Adapter::issue(token: Token) -> Token satisfies TokenIssuer::issue { token }
data Main { issuer: TokenIssuer; }
machine consume(token: Token in Issued) -> Token { token as Token }
machine Main::run(&self, token: Token) -> Token reaches TokenIssuer { let issued: Token = self.issuer.issue(token); consume(issued) }
"#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("issuance belongs to the admitted boundary, not the adapter body");
    let delegated = source.replace(
        "satisfies TokenIssuer::issue { token }",
        "satisfies TokenIssuer::issue { transition { _ -> done(token) } state done(token: Token) -> Token { token } }",
    );
    lower_typed_trees(parse_typed_trees(&delegated))
        .expect("the boundary result is admitted even when the adapter returns from a named state");
    let direct = source.replace("self.issuer.issue(token)", "Adapter::issue(token)");
    let diagnostics = lower_typed_trees(parse_typed_trees(&direct))
        .expect_err("a direct adapter call must not originate the boundary qualification");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
    let explicit = source.replace(
        "satisfies TokenIssuer::issue { token }",
        "satisfies TokenIssuer::issue ensures result in Token::Issued { token }",
    );
    let diagnostics = lower_typed_trees(parse_typed_trees(&explicit))
        .expect_err("an authored adapter guarantee still requires proof");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
        "{diagnostics:#?}"
    );
}

#[test]
fn exclusive_boundary_receiver_establishes_its_exact_routed_result() {
    let source = r#"
data Guard [linear] {
    identity: u64;
}

domain Guard::Active
established by MaskControl::save;

boundary trait MaskControl {
    machine save(&mut self) -> Guard in Active
    ensures
        result in Guard::Active;
}

data Main {
    control: MaskControl;
}

machine Main::run(&mut self) -> Guard in Active reaches MaskControl {
    self.control.save()
}
"#;

    let checked = lower_typed_trees(parse_typed_trees(source))
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
fn boundary_result_authorization_retains_requirement_identity() {
    let source = r#"
data Token {
    value: u64;
}

domain Token::Issued
established by TokenIssuer::issue;

boundary trait TokenIssuer {
    machine issue(value: u64) -> Token
    ensures
        result in Token::Issued;
}

data Main {
    issuer: TokenIssuer;
}

machine Main::run(&mut self) reaches TokenIssuer {
    let token: Token = self.issuer.issue(7);
}
"#;

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("an exact boundary result qualification should lower");
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
    let authorization = checked
        .facts
        .proof
        .contract_facts
        .iter()
        .map(|(_, fact)| fact)
        .find_map(|fact| fact.qualification_authorization)
        .expect("checked proof fact retains boundary authorization");
    assert_eq!(authorization.requirement_symbol, issuer.symbol);
    assert_eq!(authorization.signature_symbol, issue.symbol);

    let issued = checked
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Issued")
        .expect("issued domain");
    assert!(
        issued
            .establishment_routes
            .contains(&DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait: issuer.symbol,
                requirement: issue.symbol,
            })
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
        .expect("authorized call guarantee is materialized");
    assert_eq!(
        call_fact.evidence.origin,
        QualificationEvidenceOrigin::AdmittedReceipt
    );
    assert_eq!(call_fact.evidence.source_symbol, issuer.symbol);
    assert_eq!(call_fact.evidence.requirement_symbol, issue.symbol);
}

#[test]
fn boundary_carry_permission_is_admitted_and_transfers_by_exact_atom() {
    let source = r#"
data Token [linear] {
    value: u64;
}

boundary trait TokenIssuer {
    machine issue(value: u64) -> Token
    ensures
        result in Carry::MovableAddress;
}

data Main {
    issuer: TokenIssuer;
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

    let checked = lower_typed_trees(parse_typed_trees(source))
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
fn checked_conformance_establishes_bodyless_result_and_call_retains_origin() {
    let source = r#"
data Token {
    value: u64;
}

domain Token::Issued
established by TokenIssuer::issue;

trait TokenIssuer {
    machine issue(value: u64) -> Token
    ensures
        result in Token::Issued;
}

machine Token::issue(value: u64) -> Token
satisfies TokenIssuer::issue
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

    let checked = lower_typed_trees(parse_typed_trees(source))
        .expect("the exact checked conformance may establish its routed fact");
    let issued = checked
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Issued")
        .expect("issued domain");
    let issuer = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "TokenIssuer")
        .expect("issuer trait");
    let requirement = checked
        .trait_machine_signatures(issuer)
        .first()
        .expect("issue requirement");
    assert_eq!(
        issued.establishment_routes,
        [DomainEstablishmentRoute::CheckedRequirement {
            trait_definition: issuer.symbol,
            requirement: requirement.symbol,
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
    assert!(call_fact.evidence.source_symbol.is_valid());
    assert_eq!(call_fact.evidence.receipt_identity, 0);

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
                    FactPayload::DomainMembership { domain_symbol, .. }
                        if domain_symbol == issued.symbol
                )
        })
        .expect("assignment transfer preserves qualification");
    assert_eq!(transferred.evidence, call_fact.evidence);
}

#[test]
fn checked_conformance_authority_is_consumed_from_the_normalized_route_record() {
    let mut typed = parse_typed_trees(
        r#"
data Token {
    value: u64;
}

domain Token::Issued
established by TokenIssuer::issue;

trait TokenIssuer {
    machine issue(value: u64) -> Token
    ensures
        result in Token::Issued;
}

machine Token::issue(value: u64) -> Token
satisfies TokenIssuer::issue
ensures
    result in Token::Issued
{
    Token { value: value }
}

data Main {
}

machine Main::run(&mut self) {
}
"#,
    );
    let roots = typed.roots.domain_definitions;
    let [issued] = typed.tables.domain_definitions.span_mut_or_empty(roots) else {
        panic!("one domain");
    };
    issued.establishment_routes.clear();

    let diagnostics = lower_typed_trees(typed)
        .expect_err("checked lowering must not reconstruct route authority from conformance names");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("cannot prove ensures contract") }),
        "{diagnostics:#?}"
    );
}

#[test]
fn unrelated_checked_machine_cannot_originate_bodyless_membership() {
    let source = r#"
data Token {
    value: u64;
}

domain Token::Issued;

data Forger {
}

machine Forger::issue(&self, value: u64) -> Token
ensures
    result in Token::Issued
{
    Token { value: value }
}

data Main {
}

machine Main::run(&mut self) {
}
"#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a non-owner cannot mint another carrier's bodyless fact");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
        "{diagnostics:#?}"
    );
}

#[test]
fn owner_machine_still_proves_bodyful_membership() {
    let source = r#"
data Token {
    value: u64;
}

domain Token::Positive
requires
    self.value > 0;

machine Token::zero() -> Token
ensures
    result in Token::Positive
{
    Token { value: 0 }
}

data Main {
}

machine Main::run(&mut self) {
}
"#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("ownership must not bypass a predicate body");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
        "{diagnostics:#?}"
    );
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

    let checked = lower_typed_trees(parse_typed_trees(source))
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

#[test]
fn exact_machine_route_does_not_authorize_other_machines() {
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

data Forger {
}

machine Forger::mint(&self, value: u64) -> Token
ensures
    result in Token::Issued
{
    Token { value: value }
}
"#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an exact-machine route authorizes only the named machine");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
        "{diagnostics:#?}"
    );
}

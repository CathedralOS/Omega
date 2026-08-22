use super::*;
use psi_facts::{FactOrigin, FactPayload};
use psi_language_semantics::{DomainEstablishmentRoute, QualificationEvidenceOrigin};

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

machine Main::run(&mut self) -> Guard in Active {
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

domain Token::Issued {
    TokenIssuer::issue;
}

boundary trait TokenIssuer {
    machine issue(value: u64) -> Token
    ensures
        result in Token::Issued;
}

data Main {
    issuer: TokenIssuer;
}

machine Main::run(&mut self) {
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

machine Main::run(&mut self) -> Token {
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
                        permission: psi_language_semantics::CarryPermission::MovableAddress,
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
                        permission: psi_language_semantics::CarryPermission::MovableAddress,
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

domain Token::Issued {
    TokenIssuer::issue;
}

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

domain Token::Issued {
    TokenIssuer::issue;
}

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

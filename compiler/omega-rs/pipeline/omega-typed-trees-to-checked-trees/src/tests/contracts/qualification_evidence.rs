use super::*;
use omega_core::semantics::QualificationEvidenceOrigin;
use omega_facts::{FactOrigin, FactPayload};

#[test]
fn carrier_owner_establishes_bodyless_result_and_call_retains_origin() {
    let source = r#"
data Token {
    value: u64;
}

domain Token::Issued;

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
        .expect("the carrier owner may establish its bodyless fact");
    let issued = checked
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Issued")
        .expect("issued domain");
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
        QualificationEvidenceOrigin::OwnerEstablishment
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

domain Token::Positive {
    self.value > 0;
}

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

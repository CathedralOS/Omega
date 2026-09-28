//! Accepted component-assumption digests:
//! `builder.accept_component_assumption("hex")`.
//!
//! A verified component description binds its environment-mediating
//! mechanisms — an immediate port-space write is the current producer —
//! to derived assumption digests the consumer's verification request must
//! accept (wiki/spec/build/component_publication.md). Acceptances are
//! declarations harvested statically from the authoritative build machine's
//! checked call scope, the same authority `select_provider` selections
//! draw on: spelling the call outside that machine accepts nothing. Each
//! call names one exact 32-byte digest by its hex spelling — the same text
//! `UnacceptedAssumption` diagnostics and the published description
//! report — and the authored row retains its span so a malformed spelling
//! rejects at its own declaration.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// One authored `accept_component_assumption` declaration: the exact
/// assumption digest the build accepts, bound to the machine and span that
/// declared it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredComponentAssumptionAcceptance {
    pub digest: [u8; 32],
    pub selecting_machine: SymbolHandle,
    pub source_span: source::SourceSpan,
}

/// Collect the `builder.accept_component_assumption("<hex>")` declarations
/// from the one authoritative build machine. A malformed digest — wrong
/// length or non-hex text — rejects at its authored span rather than
/// silently accepting nothing.
pub fn harvest_component_assumption_acceptances(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<AuthoredComponentAssumptionAcceptance>, Vec<Diagnostic>> {
    let mut acceptances: Vec<AuthoredComponentAssumptionAcceptance> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |target: &str,
                      value_arguments: &[typed_trees::expression::ExpressionHandle],
                      source_span: source::SourceSpan| {
        if target != "accept_component_assumption" {
            return;
        }
        let [argument] = value_arguments else {
            diagnostics.push(
                Diagnostic::error(
                    "component assumption acceptance takes exactly one digest spelling",
                )
                .with_source_span(source_span),
            );
            return;
        };
        let typed_trees::expression::ExpressionNode::String(spelling) =
            typed.expression_table.expression(*argument)
        else {
            diagnostics.push(
                Diagnostic::error(
                    "component assumption acceptance takes one literal digest spelling",
                )
                .with_source_span(source_span),
            );
            return;
        };
        match decode_assumption_digest(spelling) {
            Ok(digest) => {
                if !acceptances
                    .iter()
                    .any(|acceptance| acceptance.digest == digest)
                {
                    acceptances.push(AuthoredComponentAssumptionAcceptance {
                        digest,
                        selecting_machine: machine.symbol,
                        source_span,
                    });
                }
            }
            Err(diagnostic) => {
                diagnostics.push(diagnostic.with_source_span(source_span));
            }
        }
    };
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                typed_trees::statement::StatementNode::Expression(expression) => {
                    if let typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                    {
                        record(
                            call.target.as_str(),
                            typed.expression_table.expression_handles(call.arguments),
                            typed.expression_table.source_span(*expression),
                        );
                    }
                }
                typed_trees::statement::StatementNode::Call(call) => {
                    record(
                        call.target.as_str(),
                        typed.statement_table.expression_handles(call.arguments),
                        call.source_span,
                    );
                }
                _ => {}
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(acceptances)
    } else {
        Err(diagnostics)
    }
}

/// Decode one authored digest spelling: exactly 64 hexadecimal characters
/// naming the 32 bytes in order. Uppercase letters admit — the bytes are
/// the contract — while any other shape rejects with a diagnostic naming
/// the exact spelling requirement.
fn decode_assumption_digest(spelling: &[u8]) -> Result<[u8; 32], Diagnostic> {
    let malformed = || {
        Diagnostic::error(
            "component assumption acceptance takes one digest spelled as 64 hexadecimal characters",
        )
    };
    if spelling.len() != 64 {
        return Err(malformed());
    }
    let mut digest = [0u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        let high = hex_value(spelling[2 * index]).ok_or_else(malformed)?;
        let low = hex_value(spelling[2 * index + 1]).ok_or_else(malformed)?;
        *byte = high << 4 | low;
    }
    Ok(digest)
}

fn hex_value(character: u8) -> Option<u8> {
    match character {
        b'0'..=b'9' => Some(character - b'0'),
        b'a'..=b'f' => Some(character - b'a' + 10),
        b'A'..=b'F' => Some(character - b'A' + 10),
        _ => None,
    }
}

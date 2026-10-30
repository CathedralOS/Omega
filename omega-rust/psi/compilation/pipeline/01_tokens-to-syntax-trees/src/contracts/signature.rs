use crate::contracts::facts::parse_proof_facts_until;
use crate::input::token_cursor::Input;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{CapabilityContract, CapabilityContractKind};
use tokens::{KeywordKind, PunctuationKind};

/// The fact-list terminator shared by every signature contract clause: a
/// clause ends at `;`, `{`, the next contract keyword, or input end.
fn signature_clause_boundary(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::Semicolon)
        || input.at_punctuation(PunctuationKind::LeftBrace)
        || input.at_contextual("requires")
        || input.at_contextual("ensures")
        || input.at_contextual("terminates")
        || input.at_contextual("reaches")
        || input.at_contextual("effects")
        || input.at_contextual("invokes")
        || input.at_contextual("suspends")
        || input.at_contextual("blocks")
        || input.at_contextual("crashes")
        || input.at_contextual("where")
        || input.tokens.is_empty()
}

/// The `reaches`/`invokes`/operational/contract/`where` rows parsed after a
/// bodyless machine signature, field-named so the two owners that fold them
/// into a `StateSignature` never destructure an eleven-position tuple.
pub(crate) struct SignatureClauses {
    pub service_reaches: HandleSpan<syntax_trees::identifier::Identifier>,
    pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
    pub service_reach_is_installation_bound: bool,
    pub invokes: HandleSpan<syntax_trees::identifier::Identifier>,
    pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
    pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<CapabilityContract>,
    // TPR4 (decision 23): authored bare `terminates` -- the bodyless
    // requirement's PUBLIC guarantee.
    pub terminates_guarantee: bool,
    // Signature-level `where` proof facts (the finite generic method family
    // roster is authored here as explicit equality disjunctions).
    pub where_facts: HandleSpan<syntax_trees::item::ProofFact>,
}

/// Parses the `reaches`/`requires`/`ensures` clauses that may follow a bodyless
/// machine signature: trait machine signatures and platform entry signatures
/// share this clause grammar.
pub(crate) fn parse_signature_clauses<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    allow_clauses_after_operational_separator: bool,
) -> Result<(SignatureClauses, Input<'tokens, 'source>), crate::diagnostics::parse_error::ParseError>
{
    let mut service_reaches = crate::handle_spans::HandleSpanBuilder::new();
    let mut service_reach_keyword_source_spans = Vec::new();
    let mut service_reach_is_installation_bound = false;
    let mut invokes = crate::handle_spans::HandleSpanBuilder::new();
    let mut suspends_keyword_source_spans = Vec::new();
    let mut blocks_keyword_source_spans = Vec::new();
    let mut suspends = false;
    let mut blocks = false;
    let mut contracts = crate::handle_spans::HandleSpanBuilder::new();
    let mut terminates_guarantee = false;
    let mut where_facts = crate::handle_spans::HandleSpanBuilder::new();

    while !input.at_punctuation(PunctuationKind::Semicolon)
        && !input.at_punctuation(PunctuationKind::LeftBrace)
    {
        if input.at_contextual("where") {
            // Repeated machine-parameter requirements use one `where machine`
            // clause per symbol, and `where proposition` belongs to the
            // trait-parameter parser. Leave both to the owning parser rather
            // than swallowing them as signature trivia. Any other `where`
            // clause carries signature-level proof facts: a finite generic
            // method family is authored here as explicit equality
            // disjunctions such as `where Width == 16 || Width == 32`.
            let after_where = input.restarted(input.tokens.get(1..).unwrap_or_default());
            if after_where.at_keyword(KeywordKind::Machine)
                || after_where.at_contextual("proposition")
            {
                break;
            }
            if where_facts.count() != 0 {
                return Err(input.error_here(
                    "a signature accepts one `where` clause; combine its facts in the \
                     authored clause (a finite family is one explicit disjunction, not \
                     independently listed binder values)",
                ));
            }
            input = input.take_contextual("where")?;
            let ((facts, _token_count), rest) =
                parse_proof_facts_until(syntax_trees, input, signature_clause_boundary)?;
            where_facts.push_span(facts);
            input = rest;
            continue;
        }
        if input.at_punctuation(PunctuationKind::RightBrace) {
            return Err(input.expected_one_of_here(&["`;`", "`{`"]));
        }

        if input.at_contextual("effects") {
            return Err(input.error_here(
                "the `effects` reach clause is retired; write `reaches <Service> + ...`",
            ));
        }

        if input.at_contextual("reaches") {
            input = super::take_operational_reaches_clause(
                input,
                syntax_trees,
                &mut service_reach_keyword_source_spans,
                &mut service_reaches,
                &mut service_reach_is_installation_bound,
                signature_clause_boundary,
            )?;
            continue;
        }

        if input.at_contextual("invokes") {
            input = input.take_contextual("invokes")?;
            let (binding, after_binding) = input.take_identifier()?;
            let handle = syntax_trees.items.append_identifier_path_member(binding);
            invokes.push(handle);
            input = take_invokes_signature_clause(
                after_binding,
                allow_clauses_after_operational_separator,
            )?;
            continue;
        }

        if input.at_contextual("suspends") {
            suspends_keyword_source_spans.push(input.current_source_span());
            if suspends {
                return Err(input.error_here("duplicate `suspends;` operational clause"));
            }
            suspends = true;
            input = take_invokes_signature_clause(
                input.take_contextual("suspends")?,
                allow_clauses_after_operational_separator,
            )?;
            continue;
        }

        if input.at_contextual("blocks") {
            blocks_keyword_source_spans.push(input.current_source_span());
            if blocks {
                return Err(input.error_here("duplicate `blocks;` operational clause"));
            }
            blocks = true;
            input = take_invokes_signature_clause(
                input.take_contextual("blocks")?,
                allow_clauses_after_operational_separator,
            )?;
            continue;
        }

        if input.at_contextual("crashes") {
            let keyword_source_span = Some(input.current_source_span());
            let after_keyword = input.take_contextual("crashes")?;
            let (cause, after_cause) = after_keyword.take_identifier()?;
            let cause = crate::contracts::parse_crash_cause(&cause, after_cause)?;
            let after_header = after_cause;
            let header_token_count = 2usize;
            let ((facts, fact_token_count), rest) =
                parse_proof_facts_until(syntax_trees, after_header, signature_clause_boundary)?;
            super::record_capability_contract(
                syntax_trees,
                CapabilityContract {
                    kind: CapabilityContractKind::Crashes { cause },
                    keyword_source_span,
                    binding: None,
                    facts,
                    token_count: fact_token_count
                        .checked_add(header_token_count)
                        .expect("crash contract token count overflow"),
                },
                &mut contracts,
            );
            input = rest;
            continue;
        }

        if input.at_contextual("requires") || input.at_contextual("ensures") {
            let keyword_source_span = Some(input.current_source_span());
            let kind = if input.at_contextual("requires") {
                input = input.take_contextual("requires")?;
                CapabilityContractKind::Requires
            } else {
                input = input.take_contextual("ensures")?;
                CapabilityContractKind::Ensures
            };
            let (binding, fact_input) = input.take_optional_label()?;
            let ((facts, token_count), rest) =
                crate::contracts::facts::parse_required_proof_facts_until(
                    syntax_trees,
                    fact_input,
                    signature_clause_boundary,
                    false,
                )?;
            if binding.is_some() && facts.count() != 1 {
                return Err(fact_input.error_here(
                    "a named bodyless-signature contract must contain exactly one proposition",
                ));
            }
            super::record_capability_contract(
                syntax_trees,
                CapabilityContract {
                    kind,
                    keyword_source_span,
                    binding,
                    facts,
                    token_count,
                },
                &mut contracts,
            );
            input = rest;
            continue;
        }

        if input.at_contextual("terminates") {
            input = input.take_contextual("terminates")?;
            // Decision 23 (TPR4): a bodyless requirement authors the PUBLIC
            // guarantee with bare `terminates` (the signature's own `;`
            // terminates it) -- its implementations inherit the claim. The
            // witness belongs to implementations, never the requirement.
            if input.at_contextual("by") {
                return Err(input.error_here(
                    "a ranking witness (`terminates by ...`) does not belong on a \
                     bodyless requirement (decision 23): the requirement authors the \
                     guarantee with bare `terminates`; the implementation supplies \
                     the witness that discharges it",
                ));
            }
            input =
                take_invokes_signature_clause(input, allow_clauses_after_operational_separator)?;
            terminates_guarantee = true;
            continue;
        }

        let (_, rest) = input.expect_token()?;
        input = rest;
    }

    Ok((
        SignatureClauses {
            service_reaches: service_reaches.finish(),
            service_reach_keyword_source_spans,
            service_reach_is_installation_bound,
            invokes: invokes.finish(),
            suspends_keyword_source_spans,
            blocks_keyword_source_spans,
            suspends,
            blocks,
            contracts: contracts.finish(),
            terminates_guarantee,
            where_facts: where_facts.finish(),
        },
        input,
    ))
}

fn take_invokes_signature_clause<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    allow_following_contract_clauses: bool,
) -> Result<Input<'tokens, 'source>, crate::diagnostics::parse_error::ParseError> {
    let after_semicolon = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if after_semicolon.at_punctuation(PunctuationKind::LeftBrace)
        || after_semicolon.at_contextual("reaches")
        || after_semicolon.at_contextual("effects")
        || after_semicolon.at_contextual("invokes")
        || after_semicolon.at_contextual("suspends")
        || after_semicolon.at_contextual("blocks")
        || after_semicolon.at_contextual("crashes")
        || after_semicolon.at_contextual("terminates")
        || (allow_following_contract_clauses
            && (after_semicolon.at_contextual("requires")
                || after_semicolon.at_contextual("ensures")
                || after_semicolon.at_contextual("where")))
    {
        Ok(after_semicolon)
    } else {
        // A bodyless requirement shares its final clause semicolon with the
        // signature terminator; leave it for the owning parser.
        Ok(input)
    }
}

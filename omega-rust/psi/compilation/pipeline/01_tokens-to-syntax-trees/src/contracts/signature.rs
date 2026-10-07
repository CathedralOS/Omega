use crate::contracts::facts::parse_proof_facts_until;
use crate::input::token_cursor::Input;
use arena::{Handle, HandleSpan};
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
    let mut service_start = Handle::invalid();
    let mut service_count = 0u32;
    let mut service_reach_keyword_source_spans = Vec::new();
    let mut service_reach_is_installation_bound = false;
    let mut invokes_start = Handle::invalid();
    let mut invokes_count = 0u32;
    let mut suspends_keyword_source_spans = Vec::new();
    let mut blocks_keyword_source_spans = Vec::new();
    let mut suspends = false;
    let mut blocks = false;
    let mut contract_start = Handle::invalid();
    let mut contract_count = 0u32;
    let mut terminates_guarantee = false;
    let mut where_fact_start = Handle::invalid();
    let mut where_fact_count = 0u32;

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
            let after_where =
                Input::new(input.source_id, input.tokens.get(1..).unwrap_or_default());
            if after_where.at_keyword(KeywordKind::Machine)
                || after_where.at_contextual("proposition")
            {
                break;
            }
            if where_fact_count != 0 {
                return Err(input.error_here(
                    "a signature accepts one `where` clause; combine its facts in the \
                     authored clause (a finite family is one explicit disjunction, not \
                     independently listed binder values)",
                ));
            }
            input = input.take_contextual("where")?;
            let ((facts, _token_count), rest) =
                parse_proof_facts_until(syntax_trees, input, signature_clause_boundary)?;
            where_fact_start = facts.start();
            where_fact_count = facts.count();
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
            service_reach_keyword_source_spans.push(input.current_source_span());
            input = input.take_contextual("reaches")?;
            let service_count_before_clause = service_count;
            if input.at_punctuation(PunctuationKind::LessEqual) {
                if service_reach_is_installation_bound || service_count != 0 {
                    return Err(input.error_here(
                        "an installation-bound reach row must be declared once as `reaches <= Bound`",
                    ));
                }
                service_reach_is_installation_bound = true;
                input = input.take_punctuation(PunctuationKind::LessEqual, "<=")?;
            } else if service_reach_is_installation_bound {
                return Err(input.error_here(
                    "an installation-bound reach bound cannot be combined with another `reaches` clause",
                ));
            }
            while !input.at_punctuation(PunctuationKind::Semicolon)
                && !input.at_punctuation(PunctuationKind::LeftBrace)
                && !input.at_contextual("requires")
                && !input.at_contextual("ensures")
                && !input.at_contextual("terminates")
                && !input.at_contextual("reaches")
                && !input.at_contextual("effects")
                && !input.at_contextual("invokes")
                && !input.at_contextual("suspends")
                && !input.at_contextual("blocks")
                && !input.at_contextual("crashes")
                && !input.at_contextual("where")
            {
                let (service, rest) = input.take_identifier()?;
                super::reject_retired_operational_reach(&service, rest)?;
                let handle = syntax_trees.items.append_identifier_path_member(service);
                if service_count == 0 {
                    service_start = handle;
                }
                service_count = service_count
                    .checked_add(1)
                    .expect("trait machine service span count overflow");
                input = rest;

                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                } else if input.at_punctuation(PunctuationKind::Plus) {
                    input = input.take_punctuation(PunctuationKind::Plus, "+")?;
                }
            }
            if service_reach_is_installation_bound && service_count == service_count_before_clause {
                return Err(input.error_here(
                    "an installation-bound reach row requires a nonempty upper bound after `reaches <=`",
                ));
            }
            continue;
        }

        if input.at_contextual("invokes") {
            input = input.take_contextual("invokes")?;
            let (binding, after_binding) = input.take_identifier()?;
            let handle = syntax_trees.items.append_identifier_path_member(binding);
            if invokes_count == 0 {
                invokes_start = handle;
            }
            invokes_count = invokes_count
                .checked_add(1)
                .expect("trait machine invocation span count overflow");
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
            let handle = syntax_trees
                .items
                .append_capability_contract(CapabilityContract {
                    kind: CapabilityContractKind::Crashes { cause },
                    keyword_source_span,
                    binding: None,
                    facts,
                    token_count: fact_token_count
                        .checked_add(header_token_count)
                        .expect("crash contract token count overflow"),
                });
            if contract_count == 0 {
                contract_start = handle;
            }
            contract_count = contract_count
                .checked_add(1)
                .expect("trait machine contract span count overflow");
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
            let (binding, fact_input) = if let Ok((binding, after_binding)) =
                input.take_identifier()
                && after_binding.at_punctuation(PunctuationKind::Colon)
            {
                (
                    Some(binding),
                    after_binding.take_punctuation(PunctuationKind::Colon, ":")?,
                )
            } else {
                (None, input)
            };
            let ((facts, token_count), rest) =
                parse_proof_facts_until(syntax_trees, fact_input, signature_clause_boundary)?;
            if binding.is_some() && facts.count() != 1 {
                return Err(fact_input.error_here(
                    "a named bodyless-signature contract must contain exactly one proposition",
                ));
            }
            let handle = syntax_trees
                .items
                .append_capability_contract(CapabilityContract {
                    kind,
                    keyword_source_span,
                    binding,
                    facts,
                    token_count,
                });
            if contract_count == 0 {
                contract_start = handle;
            }
            contract_count = contract_count
                .checked_add(1)
                .expect("trait machine contract span count overflow");
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
            terminates_guarantee = true;
            continue;
        }

        let (_, rest) = input.expect_token()?;
        input = rest;
    }

    let service_reaches = if service_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(service_start, service_count)
    };
    let invokes = if invokes_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(invokes_start, invokes_count)
    };
    let contracts = if contract_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(contract_start, contract_count)
    };
    let where_facts = if where_fact_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(where_fact_start, where_fact_count)
    };
    Ok((
        SignatureClauses {
            service_reaches,
            service_reach_keyword_source_spans,
            service_reach_is_installation_bound,
            invokes,
            suspends_keyword_source_spans,
            blocks_keyword_source_spans,
            suspends,
            blocks,
            contracts,
            terminates_guarantee,
            where_facts,
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

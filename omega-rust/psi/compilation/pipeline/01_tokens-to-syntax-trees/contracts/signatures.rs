use crate::contracts::facts::parse_proof_facts_until;
use crate::parameters::requirements::parse_machine_parameter_contracts;
use crate::token_input::cursor::Input;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{CapabilityContract, CapabilityContractKind};
use tokens::{KeywordKind, PunctuationKind};

fn signature_clause_boundary(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::Semicolon)
        || input.at_punctuation(PunctuationKind::LeftBrace)
        || input.at_contextual("requires")
        || input.at_contextual("ensures")
        || ((input.at_contextual("terminates")
            || input.at_contextual("reaches")
            || input.at_contextual("effects")
            || input.at_contextual("invokes")
            || input.at_contextual("suspends")
            || input.at_contextual("blocks")
            || input.at_contextual("crashes"))
            && !input.clause_word_is_an_operand())
        || input.tokens.is_empty()
}

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
    pub terminates_guarantee: bool,
    /// The signature wrote `crashes none` (see `MachineClauses`).
    pub declares_no_crash_route: bool,
    /// The signature wrote a `crashes` clause: `crashes none` or at
    /// least one `crashes Cause` route. A boundary requirement must.
    pub writes_crash_clause: bool,
}

pub(crate) fn parse_signature_clauses<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    type_parameters: HandleSpan<syntax_trees::item::TypeParameter>,
) -> Result<(SignatureClauses, Input<'tokens, 'source>), crate::errors::parse_error::ParseError> {
    let mut service_reaches = Vec::new();
    let mut service_reach_keyword_source_spans = Vec::new();
    let mut service_reach_is_installation_bound = false;
    let mut invokes = Vec::new();
    let mut suspends_keyword_source_spans = Vec::new();
    let mut blocks_keyword_source_spans = Vec::new();
    let mut suspends = false;
    let mut blocks = false;
    let mut contracts = Vec::new();
    let mut crashes_none_declared = false;
    let mut terminates_guarantee = false;

    while !input.at_punctuation(PunctuationKind::Semicolon)
        && !input.at_punctuation(PunctuationKind::LeftBrace)
        && !input.tokens.is_empty()
    {
        if input.at_contextual("requires")
            && input
                .take_contextual("requires")?
                .at_keyword(KeywordKind::Machine)
        {
            let ((), rest) =
                parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
            input = rest;
            continue;
        }
        if input.at_contextual("requires")
            && input
                .restarted(input.tokens.get(1..).unwrap_or_default())
                .at_contextual("proposition")
        {
            break;
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
            input = crate::contracts::effects::take_operational_reaches_clause(
                input,
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
            invokes.push(binding);
            input = after_binding;
            continue;
        }

        if input.at_contextual("suspends") {
            if !suspends_keyword_source_spans.is_empty() {
                return Err(input.error_here("duplicate `suspends` operational clause"));
            }
            suspends_keyword_source_spans.push(input.current_source_span());
            let after_keyword = input.take_contextual("suspends")?;
            (suspends, input) = crate::contracts::effects::operational_bound(after_keyword)?;
            continue;
        }

        if input.at_contextual("blocks") {
            if !blocks_keyword_source_spans.is_empty() {
                return Err(input.error_here("duplicate `blocks` operational clause"));
            }
            blocks_keyword_source_spans.push(input.current_source_span());
            let after_keyword = input.take_contextual("blocks")?;
            (blocks, input) = crate::contracts::effects::operational_bound(after_keyword)?;
            continue;
        }

        if input.at_contextual("crashes") {
            let keyword_source_span = Some(input.current_source_span());
            let after_keyword = input.take_contextual("crashes")?;
            if after_keyword.at_contextual("none") {
                let other_route = contracts.iter().any(|contract: &CapabilityContract| {
                    matches!(contract.kind, CapabilityContractKind::Crashes { .. })
                });
                input = crate::contracts::effects::finish_crashes_none(
                    after_keyword.take_contextual("none")?,
                    other_route,
                    signature_clause_boundary,
                )?;
                crashes_none_declared = true;
                continue;
            }
            if crashes_none_declared {
                return Err(
                    input.error_here("a `crashes Cause` route cannot follow `crashes none`")
                );
            }
            let (cause, after_cause) = after_keyword.take_identifier()?;
            let cause = crate::contracts::effects::parse_crash_cause(&cause, after_cause)?;
            let after_header = after_cause;
            let header_token_count = 2usize;
            let ((facts, fact_token_count), rest) =
                parse_proof_facts_until(syntax_trees, after_header, signature_clause_boundary)?;
            contracts.push(CapabilityContract {
                kind: CapabilityContractKind::Crashes { cause },
                keyword_source_span,
                binding: None,
                facts,
                token_count: fact_token_count
                    .checked_add(header_token_count)
                    .expect("crash contract token count overflow"),
            });
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
            contracts.push(CapabilityContract {
                kind,
                keyword_source_span,
                binding,
                facts,
                token_count,
            });
            input = rest;
            continue;
        }

        if input.at_contextual("terminates") {
            input = input.take_contextual("terminates")?;
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

        return Err(input.expected_one_of_here(&[
            "`requires`",
            "`ensures`",
            "`reaches`",
            "`invokes`",
            "`suspends`",
            "`blocks`",
            "`crashes`",
            "`terminates`",
            "`;`",
            "`{`",
        ]));
    }

    let writes_crash_clause = crashes_none_declared
        || contracts.iter().any(|contract: &CapabilityContract| {
            matches!(contract.kind, CapabilityContractKind::Crashes { .. })
        });
    Ok((
        SignatureClauses {
            service_reaches: syntax_trees
                .items
                .insert_identifier_path_members(service_reaches),
            service_reach_keyword_source_spans,
            service_reach_is_installation_bound,
            invokes: syntax_trees.items.insert_identifier_path_members(invokes),
            suspends_keyword_source_spans,
            blocks_keyword_source_spans,
            suspends,
            blocks,
            contracts: crate::contracts::effects::publish_capability_contracts(
                syntax_trees,
                contracts,
            ),
            terminates_guarantee,
            declares_no_crash_route: crashes_none_declared,
            writes_crash_clause,
        },
        input,
    ))
}

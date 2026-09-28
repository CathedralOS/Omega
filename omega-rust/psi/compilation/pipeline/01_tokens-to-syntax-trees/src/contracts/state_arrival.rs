use crate::input::token_cursor::{Input, ParseResult};
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{CapabilityContract, CapabilityContractKind};
use tokens::PunctuationKind;

/// Parse a state's explicit arrival contract. Unlike a machine signature, a
/// state has no exit contract or behavior surface: `requires` is the induction
/// hypothesis that every named incoming edge must establish.
pub(crate) fn parse_state_arrival_contracts<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<CapabilityContract>> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    while input.at_contextual("requires") {
        let keyword_source_span = Some(input.current_source_span());
        input = input.take_contextual("requires")?;
        let (binding, fact_input) = if let Ok((binding, after_binding)) = input.take_identifier()
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
            crate::contracts::facts::parse_proof_facts_until_with_machine_semicolon(
                syntax_trees,
                fact_input,
                |input| {
                    input.at_punctuation(PunctuationKind::LeftBrace)
                        || input.at_contextual("requires")
                        || input.at_contextual("ensures")
                        || input.at_contextual("reaches")
                        || input.at_contextual("effects")
                        || input.at_contextual("terminates")
                        || input.tokens.is_empty()
                },
                true,
            )?;
        if binding.is_some() && facts.count() != 1 {
            return Err(fact_input
                .error_here("a named state requires clause must contain exactly one proposition"));
        }
        let handle = syntax_trees
            .items
            .append_capability_contract(CapabilityContract {
                kind: CapabilityContractKind::Requires,
                keyword_source_span,
                binding,
                facts,
                token_count,
            });
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("state arrival contract span count overflow");
        input = rest;
    }

    if input.at_contextual("ensures")
        || input.at_contextual("reaches")
        || input.at_contextual("effects")
        || input.at_contextual("terminates")
    {
        return Err(input.error_here(
            "state signatures admit only arrival `requires`; put exit guarantees, service reach, and termination policy on the owning machine",
        ));
    }

    Ok((
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        },
        input,
    ))
}

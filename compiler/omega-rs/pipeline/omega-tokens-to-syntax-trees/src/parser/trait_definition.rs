use crate::parser::data::parse_type_parameters;
use crate::parser::input::{Input, ParseResult};
use crate::parser::machine::{ParsedTerminationClause, parse_termination_clause};
use crate::parser::proof_fact::parse_proof_facts_until;
use crate::parser::state::{parse_optional_return_type, parse_optional_state_parameters};
use crate::parser::statement::parse_statement_handle;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, StateSignature, TraitDefinition,
};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_trait_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    is_boundary: bool,
) -> ParseResult<'tokens, 'source, TraitDefinition> {
    let input = input.take_contextual("trait")?;
    let (name, mut input) = input.take_identifier()?;
    let (type_parameters, next) = parse_type_parameters(syntax_trees, input)?;
    input = next;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut required_trait_start = Handle::invalid();
    let mut required_trait_count = 0u32;
    let mut invariants = HandleSpan::empty();
    let mut machine_start = Handle::invalid();
    let mut machine_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_keyword(KeywordKind::Invariant) {
            input = input.take_keyword(KeywordKind::Invariant, "invariant")?;
            let ((facts, _token_count), rest) =
                parse_proof_facts_until(syntax_trees, input, |input| {
                    input.at_punctuation(PunctuationKind::Semicolon)
                        || input.at_keyword(KeywordKind::Machine)
                        || input.at_punctuation(PunctuationKind::RightBrace)
                        || input.tokens.is_empty()
                })?;
            extend_contiguous_span(&mut invariants, facts);
            input = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
            continue;
        }

        if input.at_contextual("requires") {
            let (required_trait, rest) = parse_trait_requirement(input)?;
            let handle = syntax_trees
                .items
                .append_identifier_path_member(required_trait);
            if required_trait_count == 0 {
                required_trait_start = handle;
            }
            required_trait_count = required_trait_count
                .checked_add(1)
                .expect("trait requirement span count overflow");
            input = rest;
            continue;
        }

        if input.at_contextual("default") {
            return Err(input.error_here(
                "the trait `default` keyword is retired; a trait machine body is the default implementation",
            ));
        }
        input = input.take_keyword(KeywordKind::Machine, "machine")?;
        let (mut signature, rest) = parse_trait_machine_signature(syntax_trees, input)?;
        let ((termination_guarantee, ranking_witness, effects, contracts), rest) =
            parse_signature_clauses(syntax_trees, rest)?;
        let has_body = rest.at_punctuation(PunctuationKind::LeftBrace);
        signature.is_default = has_body;
        signature.termination_guarantee = termination_guarantee;
        signature.ranking_witness = ranking_witness;
        signature.effects = effects;
        signature.contracts = contracts;
        let handle = syntax_trees.items.insert_state_signature(&signature);
        let handle = syntax_trees.items.append_state_signature_handle(handle);
        if machine_count == 0 {
            machine_start = handle;
        }
        machine_count = machine_count
            .checked_add(1)
            .expect("trait machine signature span count overflow");
        input = if has_body {
            parse_trait_machine_body(syntax_trees, rest)?
        } else {
            rest.take_punctuation(PunctuationKind::Semicolon, ";")?
        };
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let requires = if required_trait_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(required_trait_start, required_trait_count)
    };
    let machines = if machine_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(machine_start, machine_count)
    };
    Ok((
        TraitDefinition {
            is_boundary,
            name,
            type_parameters,
            invariants,
            requires,
            machines,
        },
        input,
    ))
}

fn parse_trait_machine_signature<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StateSignature> {
    let (name, input) = parse_trait_machine_name(input)?;
    let (parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let (return_type, input) = parse_optional_return_type(syntax_trees, input)?;

    Ok((
        StateSignature {
            name,
            is_default: false,
            parameters,
            return_type,
            termination_guarantee: false,
            ranking_witness: omega_syntax_trees::item::RankingWitnessSyntax::default(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
        },
        input,
    ))
}

fn parse_trait_machine_name<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Identifier> {
    let (mut name, mut input) = if input.at_keyword(KeywordKind::SelfType) {
        let input = input.take_keyword(KeywordKind::SelfType, "Self")?;
        (Identifier::generated("Self"), input)
    } else {
        input.take_identifier()?
    };

    while input.at_punctuation(PunctuationKind::ColonColon) {
        input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (member, next) = input.take_identifier()?;
        name = member;
        input = next;
    }

    Ok((name, input))
}

fn parse_trait_machine_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Input<'tokens, 'source>, crate::parse_error::ParseError> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (statement, rest) = parse_statement_handle(syntax_trees, input)?;
        let _ = syntax_trees.items.append_statement_handle(statement);
        input = rest;
    }
    input.take_punctuation(PunctuationKind::RightBrace, "}")
}

fn parse_trait_requirement<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, omega_syntax_trees::identifier::Identifier> {
    let input = input.take_contextual("requires")?;
    let (required_trait, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((required_trait, input))
}

/// Parses the `effects`/`requires`/`ensures` clauses that may follow a bodyless
/// machine signature: trait machine signatures and platform entry signatures
/// share this clause grammar.
pub(super) fn parse_signature_clauses<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> Result<
    (
        (
            bool,
            omega_syntax_trees::item::RankingWitnessSyntax,
            HandleSpan<omega_syntax_trees::identifier::Identifier>,
            HandleSpan<CapabilityContract>,
        ),
        Input<'tokens, 'source>,
    ),
    crate::parse_error::ParseError,
> {
    let mut effect_start = Handle::invalid();
    let mut effect_count = 0u32;
    let mut contract_start = Handle::invalid();
    let mut contract_count = 0u32;
    let mut termination_guarantee = false;
    let mut ranking_witness = omega_syntax_trees::item::RankingWitnessSyntax::default();

    while !input.at_punctuation(PunctuationKind::Semicolon)
        && !input.at_punctuation(PunctuationKind::LeftBrace)
    {
        if input.at_punctuation(PunctuationKind::RightBrace) {
            return Err(input.expected_one_of_here(&["`;`", "`{`"]));
        }

        if input.at_contextual("terminates") {
            let (clause, rest) = parse_termination_clause(syntax_trees, input)?;
            input = rest;
            match clause {
                ParsedTerminationClause::Guarantee => {
                    if termination_guarantee {
                        return Err(input.error_here("duplicate `terminates;` guarantee"));
                    }
                    termination_guarantee = true;
                }
                ParsedTerminationClause::Ranking(witness) => {
                    if ranking_witness.is_present() {
                        return Err(input.error_here("duplicate `terminates by` ranking witness"));
                    }
                    ranking_witness = witness;
                }
            }
            continue;
        }

        if input.at_contextual("decreases") || input.at_contextual("increases") {
            return Err(input.error_here(
                "standalone `decreases`/`increases` clauses are retired; use `terminates by subjects -> View;`",
            ));
        }

        if input.at_contextual("effects") {
            input = input.take_contextual("effects")?;
            while !input.at_punctuation(PunctuationKind::Semicolon)
                && !input.at_punctuation(PunctuationKind::LeftBrace)
                && !input.at_contextual("requires")
                && !input.at_contextual("ensures")
                && !input.at_contextual("where")
                && !input.at_contextual("terminates")
            {
                let (effect, rest) = input.take_identifier()?;
                let handle = syntax_trees.items.append_identifier_path_member(effect);
                if effect_count == 0 {
                    effect_start = handle;
                }
                effect_count = effect_count
                    .checked_add(1)
                    .expect("trait machine effect span count overflow");
                input = rest;

                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                }
            }
            continue;
        }

        if input.at_contextual("requires") || input.at_contextual("ensures") {
            let kind = if input.at_contextual("requires") {
                input = input.take_contextual("requires")?;
                CapabilityContractKind::Requires
            } else {
                input = input.take_contextual("ensures")?;
                CapabilityContractKind::Ensures
            };
            let ((facts, token_count), rest) =
                parse_proof_facts_until(syntax_trees, input, |input| {
                    input.at_punctuation(PunctuationKind::Semicolon)
                        || input.at_punctuation(PunctuationKind::LeftBrace)
                        || input.at_contextual("requires")
                        || input.at_contextual("ensures")
                        || input.at_contextual("effects")
                        || input.at_contextual("where")
                        || input.at_contextual("terminates")
                        || input.tokens.is_empty()
                })?;
            let handle = syntax_trees
                .items
                .append_capability_contract(CapabilityContract {
                    kind,
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

        let (_, rest) = input.expect_token()?;
        input = rest;
    }

    let effects = if effect_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(effect_start, effect_count)
    };
    let contracts = if contract_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(contract_start, contract_count)
    };
    Ok((
        (termination_guarantee, ranking_witness, effects, contracts),
        input,
    ))
}

fn extend_contiguous_span<T>(target: &mut HandleSpan<T>, source: HandleSpan<T>) {
    let mut index = source.start().arena_index();
    let generation = source.start().generation();
    for _ in 0..source.count() {
        target.push_contiguous(Handle::from_parts(index, generation));
        index = index
            .checked_add(1)
            .expect("trait invariant proof fact span index overflow");
    }
}

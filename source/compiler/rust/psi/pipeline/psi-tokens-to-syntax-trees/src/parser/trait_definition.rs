use crate::parser::data::{parse_machine_type_parameters, parse_trait_type_parameters};
use crate::parser::input::{Input, ParseResult};
use crate::parser::proof_fact::parse_proof_facts_until;
use crate::parser::state::{parse_optional_return_type, parse_optional_state_parameters};
use crate::parser::statement::{parse_asm_block_statement_handles, parse_statement_handle};
use crate::parser::type_reference::parse_type_reference_handle;
use psi_arena::{Handle, HandleSpan};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, CrashCause, StateSignature, TraitDefinition,
};
use psi_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_trait_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    is_boundary: bool,
) -> ParseResult<'tokens, 'source, TraitDefinition> {
    let input = input.take_contextual("trait")?;
    let (name, mut input) = input.take_identifier()?;
    let (generic_parameters, next) = parse_trait_type_parameters(syntax_trees, input)?;
    input = next;
    let type_parameters = generic_parameters.type_parameters;
    let (parents, next) = parse_trait_parents(syntax_trees, input)?;
    input = next;
    let ((), next) = parse_proposition_parameter_contracts(syntax_trees, type_parameters, input)?;
    input = next;
    let mut conformance_bounds = generic_parameters.conformance_bounds;
    if input.at_contextual("where") {
        let (mut bounds, next) =
            crate::parser::machine::parse_generic_conformance_bounds(syntax_trees, input)?;
        input = next;
        conformance_bounds.append(&mut bounds);
    }
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut required_trait_start = Handle::invalid();
    let mut required_trait_count = 0u32;
    let mut machine_start = Handle::invalid();
    let mut machine_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_contextual("invariant") {
            return Err(input.error_here(
                "the `invariant` clause is retired: traits publish proof obligations through \
                 explicit `requires` and `ensures`; value-wide facts belong to the carrier's \
                 default domain",
            ));
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

        // The `default` KEYWORD IS KILLED (owner, 2026-07-18): a trait
        // machine WITH A BODY is the default -- body presence is the
        // marker. The old spelling refuses with direction.
        if input.at_contextual("default") {
            return Err(input.error_here(
                "the `default` keyword is retired: a trait machine with a body IS \
                 the default -- drop the keyword and keep the body",
            ));
        }
        let spelling = if input.at_contextual("operator") {
            input = input.take_contextual("operator")?;
            if input
                .tokens
                .first()
                .is_some_and(|token| token.punctuation().is_some())
            {
                let (spelling, rest) = crate::parser::operator::parse_operator_spelling(input)?;
                input = rest;
                Some(spelling)
            } else {
                None
            }
        } else {
            input = input.take_keyword(KeywordKind::Machine, "machine")?;
            None
        };
        let (mut signature, rest) = parse_trait_machine_signature(syntax_trees, input)?;
        signature.spelling = spelling;
        let (
            (
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
            ),
            rest,
        ) = parse_signature_clauses(syntax_trees, rest, true)?;
        // Body presence = the default marker.
        let is_default = rest.at_punctuation(PunctuationKind::LeftBrace);
        signature.is_default = is_default;
        if service_reach_is_installation_bound && (!is_boundary || is_default) {
            return Err(rest.error_here(
                "`reaches <= Bound` is permitted only on a bodyless boundary-trait requirement",
            ));
        }
        signature.service_reach_is_installation_bound = service_reach_is_installation_bound;
        signature.service_reach_keyword_source_spans = service_reach_keyword_source_spans;
        signature.service_reaches = service_reaches;
        signature.invokes = invokes;
        signature.suspends_keyword_source_spans = suspends_keyword_source_spans;
        signature.blocks_keyword_source_spans = blocks_keyword_source_spans;
        signature.suspends = suspends;
        signature.blocks = blocks;
        signature.contracts = contracts;
        signature.terminates_guarantee = terminates_guarantee;
        let (default_body, next) = if is_default {
            parse_trait_default_machine_body(syntax_trees, rest)?
        } else {
            (
                HandleSpan::empty(),
                rest.take_punctuation(PunctuationKind::Semicolon, ";")?,
            )
        };
        signature.default_body = default_body;
        let handle = syntax_trees.items.insert_state_signature(&signature);
        let handle = syntax_trees.items.append_state_signature_handle(handle);
        if machine_count == 0 {
            machine_start = handle;
        }
        machine_count = machine_count
            .checked_add(1)
            .expect("trait machine signature span count overflow");
        input = next;
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
            is_public: false,
            name,
            lifetime_parameters: generic_parameters.lifetime_parameters,
            type_parameters,
            conformance_bounds,
            parents,
            requires,
            machines,
        },
        input,
    ))
}

fn parse_proposition_parameter_contracts<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_parameters: HandleSpan<psi_syntax_trees::item::TypeParameter>,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ()> {
    loop {
        if !input.at_contextual("where") {
            break;
        }
        let after_where = input.take_contextual("where")?;
        if !after_where.at_contextual("proposition") {
            break;
        }
        let after_proposition = after_where.take_contextual("proposition")?;
        let (name, after_name) = after_proposition.take_identifier()?;
        let Some(parameter_index) = syntax_trees
            .items
            .type_parameters(type_parameters)
            .iter()
            .position(|parameter| parameter.name == name)
        else {
            return Err(after_name.error_here(format!(
                "`where proposition {}` has no matching `<proposition {}>` parameter",
                name.as_str(),
                name.as_str(),
            )));
        };
        match &syntax_trees.items.type_parameters(type_parameters)[parameter_index].kind {
            psi_syntax_trees::item::TypeParameterKind::Proposition { contract: None } => {}
            psi_syntax_trees::item::TypeParameterKind::Proposition { contract: Some(_) } => {
                return Err(after_name.error_here(format!(
                    "proposition parameter `{}` already has a declaration-site signature",
                    name.as_str(),
                )));
            }
            _ => {
                return Err(after_name.error_here(format!(
                    "`{}` is not a proposition parameter; declare it as `<proposition {}>` first",
                    name.as_str(),
                    name.as_str(),
                )));
            }
        }
        let (parameters, rest) = parse_optional_state_parameters(syntax_trees, after_name)?;
        let rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
        syntax_trees.items.type_parameters_mut(type_parameters)[parameter_index].kind =
            psi_syntax_trees::item::TypeParameterKind::Proposition {
                contract: Some(psi_syntax_trees::item::PropositionParameterSignature {
                    name,
                    parameters,
                }),
            };
        input = rest;
    }

    if let Some(missing) = syntax_trees
        .items
        .type_parameters(type_parameters)
        .iter()
        .find(|parameter| {
            matches!(
                parameter.kind,
                psi_syntax_trees::item::TypeParameterKind::Proposition { contract: None }
            )
        })
    {
        return Err(input.error_here(format!(
            "proposition parameter `{}` requires an authored declaration-site signature: write `where proposition {}(...)`;",
            missing.name.as_str(),
            missing.name.as_str(),
        )));
    }
    Ok(((), input))
}

fn parse_trait_parents<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<psi_syntax_trees::types::TypeReferenceHandle>> {
    if !input.at_punctuation(PunctuationKind::Colon) {
        return Ok((HandleSpan::empty(), input));
    }

    input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let mut parents = Vec::new();
    loop {
        let (parent, rest) = parse_type_reference_handle(syntax_trees, input)?;
        parents.push(parent);
        input = rest;
        if !input.at_punctuation(PunctuationKind::Plus) {
            break;
        }
        input = input.take_punctuation(PunctuationKind::Plus, "+")?;
    }
    Ok((
        syntax_trees
            .type_references
            .insert_type_reference_handles(parents),
        input,
    ))
}

fn parse_trait_machine_signature<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StateSignature> {
    let (name, input) = parse_trait_machine_name(input)?;
    let (generic_parameters, input) = parse_machine_type_parameters(syntax_trees, input)?;
    let (parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let (return_type, input) = parse_optional_return_type(syntax_trees, input)?;
    let ((), input) = crate::parser::machine::parse_machine_parameter_contracts(
        syntax_trees,
        generic_parameters.type_parameters,
        input,
    )?;

    Ok((
        StateSignature {
            name,
            spelling: None,
            lifetime_parameters: generic_parameters.lifetime_parameters,
            type_parameters: generic_parameters.type_parameters,
            is_default: false,
            parameters,
            return_type,
            service_reach_is_installation_bound: false,
            service_reach_keyword_source_spans: Vec::new(),
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends_keyword_source_spans: Vec::new(),
            blocks_keyword_source_spans: Vec::new(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            default_body: HandleSpan::empty(),
            terminates_guarantee: false,
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

fn parse_trait_default_machine_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<
    (
        HandleSpan<psi_syntax_trees::statement::StatementHandle>,
        Input<'tokens, 'source>,
    ),
    crate::parse_error::ParseError,
> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut start = Handle::invalid();
    let mut count = 0u32;
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (statements, rest) = if input.at_contextual("asm") {
            parse_asm_block_statement_handles(syntax_trees, input)?
        } else {
            let (statement, rest) = parse_statement_handle(syntax_trees, input)?;
            let handle = syntax_trees.items.append_statement_handle(statement);
            (HandleSpan::from_parts(handle, 1), rest)
        };
        if count == 0 {
            start = statements.start();
        }
        count = count
            .checked_add(statements.count())
            .expect("trait default statement span count overflow");
        input = rest;
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let body = if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    };
    Ok((body, input))
}

fn parse_trait_requirement<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, psi_syntax_trees::identifier::Identifier> {
    let input = input.take_contextual("requires")?;
    let (required_trait, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((required_trait, input))
}

/// Parses the `reaches`/`requires`/`ensures` clauses that may follow a bodyless
/// machine signature: trait machine signatures and platform entry signatures
/// share this clause grammar.
pub(super) fn parse_signature_clauses<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    allow_clauses_after_operational_separator: bool,
) -> Result<
    (
        (
            HandleSpan<psi_syntax_trees::identifier::Identifier>,
            Vec<psi_source::SourceSpan>,
            bool,
            HandleSpan<psi_syntax_trees::identifier::Identifier>,
            Vec<psi_source::SourceSpan>,
            Vec<psi_source::SourceSpan>,
            bool,
            bool,
            HandleSpan<CapabilityContract>,
            // TPR4 (decision 23): authored bare `terminates` -- the bodyless
            // requirement's PUBLIC guarantee.
            bool,
        ),
        Input<'tokens, 'source>,
    ),
    crate::parse_error::ParseError,
> {
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

    while !input.at_punctuation(PunctuationKind::Semicolon)
        && !input.at_punctuation(PunctuationKind::LeftBrace)
    {
        // Repeated machine-parameter requirements use one `where machine`
        // clause per symbol. Leave the next `where` to the owning machine
        // parser rather than swallowing it as signature trivia.
        if input.at_contextual("where") {
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
                reject_retired_operational_reach(&service, rest)?;
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
            input = take_operational_signature_clause(
                input,
                "suspends",
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
            input = take_operational_signature_clause(
                input,
                "blocks",
                allow_clauses_after_operational_separator,
            )?;
            continue;
        }

        if input.at_contextual("crashes") {
            let keyword_source_span = Some(input.current_source_span());
            let after_keyword = input.take_contextual("crashes")?;
            let (cause, after_cause) = after_keyword.take_identifier()?;
            let cause = match cause.as_str() {
                "Trap" => CrashCause::Trap,
                "Abort" => CrashCause::Abort,
                _ => {
                    return Err(after_cause.error_here(format!(
                        "unknown crash cause `{}`; expected `Trap` or `Abort`",
                        cause.as_str()
                    )));
                }
            };
            let after_header = after_cause;
            let header_token_count = 2usize;
            let ((facts, fact_token_count), rest) =
                parse_proof_facts_until(syntax_trees, after_header, |input| {
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
                })?;
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
                parse_proof_facts_until(syntax_trees, fact_input, |input| {
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
                })?;
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
    Ok((
        (
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
        ),
        input,
    ))
}

fn take_invokes_signature_clause<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    allow_following_contract_clauses: bool,
) -> Result<Input<'tokens, 'source>, crate::parse_error::ParseError> {
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
        Ok(input)
    }
}

fn reject_retired_operational_reach(
    service: &Identifier,
    input: Input<'_, '_>,
) -> Result<(), crate::parse_error::ParseError> {
    let replacement = match service.as_str() {
        "Suspend" => "suspends;",
        "Block" => "blocks;",
        "thread_block" => "blocks;",
        "sync_wait" => "the appropriate independent `suspends;` and/or `blocks;` clause",
        _ => return Ok(()),
    };
    Err(input.error_here(format!(
        "`reaches {}` is invalid: `reaches` contains boundary-service identities only; write `{replacement}` as an independent operational clause",
        service.as_str()
    )))
}

fn take_operational_signature_clause<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    name: &str,
    allow_following_contract_clauses: bool,
) -> Result<Input<'tokens, 'source>, crate::parse_error::ParseError> {
    let after_name = input.take_contextual(name)?;
    let after_semicolon = after_name.take_punctuation(PunctuationKind::Semicolon, ";")?;
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
        Ok(after_name)
    }
}

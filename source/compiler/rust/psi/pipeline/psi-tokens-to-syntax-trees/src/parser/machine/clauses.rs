use crate::parser::expression::{
    parse_expression_handle_without_struct_literals,
    parse_expression_handle_without_struct_literals_or_membership,
};
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::type_reference::parse_type_reference_handle;
use psi_arena::{Handle, HandleSpan};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, CrashCause, GenericConformanceBound,
    SatisfiesClause,
};
use psi_tokens::{KeywordKind, PunctuationKind};

type MachineClauses = (
    // TPR2: authored BARE `terminates;` (the public guarantee); the by-form
    // supplies only the witness and leaves this false.
    bool,
    HandleSpan<psi_syntax_trees::expression::ExpressionHandle>,
    HandleSpan<Identifier>,
    // TPR3: argumented-view arguments (`-> Nat::IncreasingTo(limit)`).
    HandleSpan<psi_syntax_trees::expression::ExpressionHandle>,
    psi_syntax_trees::expression::ExpressionHandle,
    bool,
    HandleSpan<Identifier>,
    HandleSpan<Identifier>,
    bool,
    bool,
    HandleSpan<CapabilityContract>,
    psi_syntax_trees::types::TypeReferenceHandle,
    Vec<GenericConformanceBound>,
);

type RankedSubjects = (
    HandleSpan<psi_syntax_trees::expression::ExpressionHandle>,
    HandleSpan<Identifier>,
    // TPR3: an argumented view's arguments (`-> Nat::IncreasingTo(limit)`).
    HandleSpan<psi_syntax_trees::expression::ExpressionHandle>,
    psi_syntax_trees::expression::ExpressionHandle,
);

pub(super) fn parse_machine_clauses<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, MachineClauses> {
    let mut terminates_guarantee = false;
    let mut ranking_subjects = HandleSpan::empty();
    let mut ranking_view = HandleSpan::empty();
    let mut ranking_view_arguments = HandleSpan::empty();
    let mut ranking_range = psi_syntax_trees::expression::ExpressionHandle::invalid();
    let mut service_reach_is_installation_bound = false;
    let mut service_start = Handle::invalid();
    let mut service_count = 0u32;
    let mut invokes_start = Handle::invalid();
    let mut invokes_count = 0u32;
    let mut suspends = false;
    let mut blocks = false;
    let mut contract_start = Handle::invalid();
    let mut contract_count = 0u32;
    let mut outcome_case_groups = Vec::<String>::new();
    let mut public_selectors = Vec::<String>::new();
    let mut return_type = psi_syntax_trees::types::TypeReferenceHandle::invalid();
    let mut conformance_bounds = Vec::new();

    while !input.at_punctuation(PunctuationKind::LeftBrace)
        // CH10 bodyless machines end at `;` in body position -- the clause
        // loop stands down and leaves the semicolon for parse_machine.
        && !input.at_punctuation(PunctuationKind::Semicolon)
    {
        if input.at_contextual("terminates") {
            input = input.take_contextual("terminates")?;
            // Decision 23 (TPR1): bare `terminates;` authors the public
            // guarantee; `terminates by <subjects> [-> View] [in <range>];`
            // supplies the private ranking witness. The old block form is
            // RETIRED loudly below.
            if input.at_contextual("by") {
                let by_input = input.take_contextual("by")?;
                let ((clause_subjects, clause_view, clause_view_arguments, clause_range), rest) =
                    parse_ranked_subjects(syntax_trees, by_input)?;
                input = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
                ranking_subjects = clause_subjects;
                ranking_view = clause_view;
                ranking_view_arguments = clause_view_arguments;
                ranking_range = clause_range;
                continue;
            }
            if input.at_punctuation(PunctuationKind::Semicolon) {
                input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
                // TPR2: the bare form authors the PUBLIC guarantee.
                terminates_guarantee = true;
                continue;
            }
            if starts_termination_clause_block(input) {
                return Err(input.error_here(
                    "the `terminates { decreases ...; }` block form is retired \
                     (decision 23): spell the ranking witness as `terminates by \
                     <subjects> [-> View] [in <range>];`, or bare `terminates;` \
                     for the guarantee alone",
                ));
            }
            // Tolerated no-semicolon bare form (immediately before the body
            // brace or another clause): still the authored public guarantee.
            terminates_guarantee = true;
            continue;
        }

        if input.at_contextual("decreases") {
            return Err(input.error_here(
                "a standalone `decreases` clause is retired (decision 23): \
                 attach the ranking witness to the guarantee as `terminates by \
                 <subjects> [-> View] [in <range>];`",
            ));
        }

        if input.at_contextual("effects") {
            return Err(input.error_here(
                "the `effects` reach clause is retired; write `reaches <Service> + ...`",
            ));
        }

        if input.at_contextual("reaches") {
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
            while !input.at_punctuation(PunctuationKind::LeftBrace)
                && !input.at_punctuation(PunctuationKind::Semicolon)
                && !input.at_contextual("requires")
                && !input.at_contextual("ensures")
                && !input.at_contextual("terminates")
                && !input.at_contextual("decreases")
                && !input.at_contextual("reaches")
                && !input.at_contextual("effects")
                && !input.at_contextual("invokes")
                && !input.at_contextual("suspends")
                && !input.at_contextual("blocks")
                && !input.at_contextual("crashes")
                && !input.at_contextual("boundary")
                && !input.at_contextual("where")
                && !input.at_contextual("satisfies")
            {
                let (service, rest) = input.take_identifier()?;
                reject_retired_operational_reach(&service, rest)?;
                let handle = syntax_trees.items.append_identifier_path_member(service);
                if service_count == 0 {
                    service_start = handle;
                }
                service_count = service_count
                    .checked_add(1)
                    .expect("machine service span count overflow");
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
                .expect("machine invocation span count overflow");
            input = take_invokes_clause_terminator(after_binding)?;
            continue;
        }

        if input.at_contextual("suspends") {
            if suspends {
                return Err(input.error_here("duplicate `suspends;` operational clause"));
            }
            suspends = true;
            input = take_operational_clause(input, "suspends")?;
            continue;
        }

        if input.at_contextual("blocks") {
            if blocks {
                return Err(input.error_here("duplicate `blocks;` operational clause"));
            }
            blocks = true;
            input = take_operational_clause(input, "blocks")?;
            continue;
        }

        if input.at_contextual("crashes") {
            let keyword_source_span = Some(input.current_source_span());
            let ((cause, header_token_count), after_header) = parse_crash_header(input)?;
            let ((facts, fact_token_count), rest) =
                crate::parser::proof_fact::parse_proof_facts_until_with_machine_semicolon(
                    syntax_trees,
                    after_header,
                    |input| {
                        input.at_punctuation(PunctuationKind::LeftBrace)
                            || input.at_keyword(KeywordKind::Machine)
                            || input.at_keyword(KeywordKind::Data)
                            || input.at_keyword(KeywordKind::Use)
                            || input.at_contextual("requires")
                            || input.at_contextual("ensures")
                            || input.at_contextual("terminates")
                            || input.at_contextual("decreases")
                            || input.at_contextual("reaches")
                            || input.at_contextual("effects")
                            || input.at_contextual("invokes")
                            || input.at_contextual("suspends")
                            || input.at_contextual("blocks")
                            || input.at_contextual("crashes")
                            || input.at_contextual("boundary")
                            || input.at_contextual("where")
                            || input.at_contextual("satisfies")
                            || input.tokens.is_empty()
                    },
                    true,
                )?;
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
                .expect("machine contract span count overflow");
            input = rest;
            continue;
        }

        if input.at_contextual("boundary") {
            return Err(input.error_here(
                "trailing `boundary host` and `boundary Name` contract clauses are retired; use a leading `boundary trait`, `boundary machine`, or `boundary operator` declaration and realize exact requirements with `satisfies Trait::requirement via Binding::...`",
            ));
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
            if matches!(kind, CapabilityContractKind::Ensures)
                && outcome_case_path_followed_by_arrow(input)?
            {
                let ((contracts, group_name), rest) =
                    parse_outcome_specific_ensures_group(syntax_trees, input, keyword_source_span)?;
                if outcome_case_groups
                    .iter()
                    .any(|existing| existing == &group_name)
                {
                    return Err(input.error_here(format!(
                        "duplicate outcome-specific ensures group for `{group_name}`"
                    )));
                }
                outcome_case_groups.push(group_name);
                for contract in contracts {
                    if let Some(binding) = contract.binding.as_ref() {
                        if public_selectors
                            .iter()
                            .any(|existing| existing == binding.as_str())
                        {
                            return Err(input.error_here(format!(
                                "duplicate machine-wide public ensures selector `{binding}`"
                            )));
                        }
                        public_selectors.push(binding.as_str().to_owned());
                    }
                    let handle = syntax_trees.items.append_capability_contract(contract);
                    if contract_count == 0 {
                        contract_start = handle;
                    }
                    contract_count = contract_count
                        .checked_add(1)
                        .expect("machine contract span count overflow");
                }
                input = rest;
                continue;
            }
            if matches!(kind, CapabilityContractKind::Ensures) {
                reject_ambiguous_outcome_specific_ensures(input)?;
            }
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
                crate::parser::proof_fact::parse_proof_facts_until_with_machine_semicolon(
                    syntax_trees,
                    fact_input,
                    |input| {
                        input.at_punctuation(PunctuationKind::LeftBrace)
                        // CH10 bodyless machines (`ensures <fact>;` then the
                        // next item): a HARD item keyword after the facts
                        // terminates the list -- a fact expression can never
                        // begin with one.
                        || input.at_keyword(KeywordKind::Machine)
                        || input.at_keyword(KeywordKind::Data)
                        || input.at_keyword(KeywordKind::Use)
                        || input.at_contextual("requires")
                        || input.at_contextual("ensures")
                        || input.at_contextual("terminates")
                        || input.at_contextual("decreases")
                        || input.at_contextual("reaches")
                        || input.at_contextual("effects")
                        || input.at_contextual("invokes")
                        || input.at_contextual("suspends")
                        || input.at_contextual("blocks")
                        || input.at_contextual("crashes")
                        || input.at_contextual("boundary")
                        || input.at_contextual("where")
                        || input.at_contextual("satisfies")
                        || input.tokens.is_empty()
                    },
                    true,
                )?;
            if binding.is_some() && facts.count() != 1 {
                return Err(fact_input.error_here(
                    "a named requires or ensures clause must contain exactly one proposition",
                ));
            }
            if matches!(kind, CapabilityContractKind::Ensures)
                && let Some(binding) = binding.as_ref()
            {
                if public_selectors
                    .iter()
                    .any(|existing| existing == binding.as_str())
                {
                    return Err(fact_input.error_here(format!(
                        "duplicate machine-wide public ensures selector `{binding}`"
                    )));
                }
                public_selectors.push(binding.as_str().to_owned());
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
                .expect("machine contract span count overflow");
            input = rest;
            continue;
        }

        // A return type may follow the clauses (`machine f(..) terminates
        // by ..; -> usize { .. }`). This used to be eaten by a skip-any-token
        // fallback, so the machine silently parsed as VOID -- the declared
        // `-> usize` never reached any state.
        if input.at_punctuation(PunctuationKind::Arrow) {
            let (parsed, rest) =
                crate::parser::state::parse_optional_return_type(syntax_trees, input)?;
            return_type = parsed;
            input = rest;
            continue;
        }

        // Ordinary generic conformance tests are semantic input, not header
        // trivia: retain every subject, trait specialization, and optional
        // named-conformance selection for resolution and checking.
        if input.at_contextual("where") {
            let after_where = input.take_contextual("where")?;
            if after_where.at_keyword(KeywordKind::Machine) {
                return Err(after_where.error_here(
                    "one-off `where machine T::member(...)` requirements are unsupported; declare a trait and bind an explicit conformance",
                ));
            }
            let (bounds, rest) = parse_generic_conformance_bounds(syntax_trees, input)?;
            conformance_bounds = bounds;
            input = rest;
            continue;
        }

        // Anything else between the machine header and its `{` is a mistake --
        // silently skipping it hid dropped return types and typo'd clauses.
        return Err(input.expected_one_of_here(&[
            "`terminates`",
            "`decreases`",
            "`reaches`",
            "`invokes <binding>;`",
            "`suspends;`",
            "`blocks;`",
            "`crashes <Cause>`",
            "`boundary`",
            "`requires`",
            "`ensures`",
            "`->`",
            "`{`",
        ]));
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
            terminates_guarantee,
            ranking_subjects,
            ranking_view,
            ranking_view_arguments,
            ranking_range,
            service_reach_is_installation_bound,
            service_reaches,
            invokes,
            suspends,
            blocks,
            contracts,
            return_type,
            conformance_bounds,
        ),
        input,
    ))
}

fn outcome_case_path_followed_by_arrow(
    input: Input<'_, '_>,
) -> Result<bool, crate::parse_error::ParseError> {
    if !input.at_name_like() {
        return Ok(false);
    }
    let (_, mut rest) = input.take_identifier()?;
    let mut member_count = 1usize;
    while rest.at_punctuation(PunctuationKind::ColonColon) {
        rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (_, next) = rest.take_identifier()?;
        rest = next;
        member_count += 1;
    }
    Ok(member_count >= 2 && rest.at_punctuation(PunctuationKind::Arrow))
}

fn reject_ambiguous_outcome_specific_ensures(
    input: Input<'_, '_>,
) -> Result<(), crate::parse_error::ParseError> {
    let Ok((guard, arrow)) = input.split_at_top_level_punctuation(
        PunctuationKind::Arrow,
        "outcome-specific ensures requires an arrow",
    ) else {
        return Ok(());
    };
    let after_arrow = arrow.take_punctuation(PunctuationKind::Arrow, "->")?;
    if !after_arrow.at_punctuation(PunctuationKind::LeftBrace) {
        return Ok(());
    }
    // Do not let the diagnostic lookahead cross the end of the current
    // ordinary guarantee and reinterpret a later guarded group. The ordinary
    // fact parser owns these clause boundaries.
    if guard.tokens.iter().any(|token| {
        token.punctuation() == Some(PunctuationKind::Semicolon)
            || matches!(
                token.lexeme.as_str(),
                "requires"
                    | "ensures"
                    | "crashes"
                    | "terminates"
                    | "decreases"
                    | "reaches"
                    | "effects"
                    | "invokes"
                    | "suspends"
                    | "blocks"
                    | "where"
                    | "satisfies"
            )
    }) {
        return Ok(());
    }
    if guard
        .tokens
        .iter()
        .any(|token| token.punctuation() == Some(PunctuationKind::EqualEqual))
    {
        return Err(input.error_here(
            "outcome-specific ensures rejects Boolean guards; write the exact declared result case as `Result::Case -> { guarantees }`",
        ));
    }
    if guard.tokens.iter().any(|token| {
        matches!(
            token.punctuation(),
            Some(PunctuationKind::LeftParen | PunctuationKind::LeftBrace)
        )
    }) {
        return Err(input.error_here(
            "outcome-specific ensures rejects case-literal-shaped selectors; write only the exact nominal path `Result::Case -> { guarantees }`",
        ));
    }
    Err(input.error_here(
        "outcome-specific ensures requires the exact nominal result-case path `Result::Case -> { guarantees }`",
    ))
}

fn parse_outcome_specific_ensures_group<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    keyword_source_span: Option<psi_source::SourceSpan>,
) -> ParseResult<'tokens, 'source, (Vec<CapabilityContract>, String)> {
    let (result_case, rest) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
    let group_name = syntax_trees
        .items
        .identifier_path_members(result_case)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let mut input = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut contracts = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let row_input = input;
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
        let ((facts, token_count), rest) = crate::parser::proof_fact::parse_proof_facts_until(
            syntax_trees,
            fact_input,
            |input| {
                input.at_punctuation(PunctuationKind::Semicolon)
                    || input.at_punctuation(PunctuationKind::RightBrace)
                    || input.tokens.is_empty()
            },
        )?;
        if facts.count() != 1 {
            return Err(row_input.error_here(
                "each outcome-specific ensures row must contain exactly one guarantee",
            ));
        }
        contracts.push(CapabilityContract {
            kind: CapabilityContractKind::EnsuresForResultCase { result_case },
            keyword_source_span,
            binding,
            facts,
            token_count,
        });
        input = rest;
        if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        } else if !input.at_punctuation(PunctuationKind::RightBrace) {
            return Err(input.error_here("expected `;` or `}` after outcome-specific ensures row"));
        }
    }
    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    if contracts.is_empty() {
        return Err(input
            .error_here("an outcome-specific ensures group must contain at least one guarantee"));
    }
    Ok(((contracts, group_name), input))
}

pub(in crate::parser) fn parse_generic_conformance_bounds<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<GenericConformanceBound>> {
    let mut input = input.take_contextual("where")?;
    let mut bounds = Vec::new();

    loop {
        let (subject, rest) = input.take_identifier()?;
        let rest = rest.take_contextual("satisfies")?;
        let (carrier, rest) = rest.take_identifier()?;
        let (arguments, mut rest) = parse_optional_satisfies_type_arguments(syntax_trees, rest)?;
        let selected_conformance = if rest.at_punctuation(PunctuationKind::ColonColon) {
            rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let (name, next) = rest.take_identifier()?;
            rest = next;
            let application = if let Some((application, next)) =
                crate::parser::expression::try_parse_static_symbol_application(rest)?
            {
                rest = next;
                Some(application)
            } else {
                None
            };
            Some(psi_syntax_trees::expression::StaticMachineArgument {
                path: vec![name].into_boxed_slice(),
                application,
                const_literal: None,
                evidence_projection: None,
            })
        } else {
            None
        };
        bounds.push(GenericConformanceBound {
            binder: None,
            subject,
            carrier,
            arguments,
            selected_conformance,
        });

        if !rest.at_punctuation(PunctuationKind::Comma) {
            return Ok((bounds, rest));
        }
        input = rest.take_punctuation(PunctuationKind::Comma, ",")?;
    }
}

fn take_invokes_clause_terminator<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> Result<Input<'tokens, 'source>, crate::parse_error::ParseError> {
    let after_semicolon = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if continues_after_operational_clause(after_semicolon) {
        Ok(after_semicolon)
    } else {
        // On a bodyless machine this is also the item terminator.
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

fn take_operational_clause<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    name: &str,
) -> Result<Input<'tokens, 'source>, crate::parse_error::ParseError> {
    let after_name = input.take_contextual(name)?;
    let after_semicolon = after_name.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if continues_after_operational_clause(after_semicolon) {
        Ok(after_semicolon)
    } else {
        // On a bodyless machine this semicolon is both the operational-clause
        // terminator and the item terminator. Leave it for `parse_machine`.
        Ok(after_name)
    }
}

fn continues_after_operational_clause(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::LeftBrace)
        || input.at_punctuation(PunctuationKind::Arrow)
        || input.at_contextual("terminates")
        || input.at_contextual("decreases")
        || input.at_contextual("reaches")
        || input.at_contextual("effects")
        || input.at_contextual("invokes")
        || input.at_contextual("suspends")
        || input.at_contextual("blocks")
        || input.at_contextual("crashes")
        || input.at_contextual("boundary")
        || input.at_contextual("requires")
        || input.at_contextual("ensures")
        || input.at_contextual("where")
        || input.at_contextual("satisfies")
}

fn parse_crash_header<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, (CrashCause, usize)> {
    let input = input.take_contextual("crashes")?;
    let (cause, input) = input.take_identifier()?;
    let cause = match cause.as_str() {
        "Trap" => CrashCause::Trap,
        "Abort" => CrashCause::Abort,
        _ => {
            return Err(input.error_here(format!(
                "unknown crash cause `{}`; expected `Trap` or `Abort`",
                cause.as_str()
            )));
        }
    };
    Ok(((cause, 2), input))
}

fn starts_termination_clause_block(input: Input<'_, '_>) -> bool {
    if !input.at_punctuation(PunctuationKind::LeftBrace) || input.tokens.is_empty() {
        return false;
    }

    let rest = Input::new(input.source_id, &input.tokens[1..]);
    rest.tokens.first().is_some_and(|token| {
        matches!(token.punctuation(), Some(PunctuationKind::RightBrace))
            || matches!(token.lexeme.as_str(), "decreases" | "increases")
    })
}

/// Decision 23's ranking-witness body: `<subject>` or `(<s1>, <s2>, ..)`
/// followed by an optional `-> View::Path` and an optional `in <range>`
/// (the rank-range constraint; stored on the machine, refused downstream
/// until TPR3's cycle checker consumes ranges).
fn parse_ranked_subjects<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, RankedSubjects> {
    let (subjects, mut rest) = if input.at_punctuation(PunctuationKind::LeftParen) {
        // The tuple form `terminates by (index, limit) -> View`: the arrow's
        // left side is uniformly the ranked subjects, bound in order to the
        // named view's parameters.
        let mut tuple_input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let mut subjects = Vec::new();
        loop {
            let (subject, after_subject) =
                parse_expression_handle_without_struct_literals(syntax_trees, tuple_input)?;
            subjects.push(subject);
            if after_subject.at_punctuation(PunctuationKind::Comma) {
                tuple_input = after_subject.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }
            tuple_input = after_subject.take_punctuation(PunctuationKind::RightParen, ")")?;
            break;
        }
        (subjects, tuple_input)
    } else {
        // NO-MEMBERSHIP variant: the clause's own `in <range>` must not be
        // eaten as a membership expression on the subject.
        let (expression, rest) =
            parse_expression_handle_without_struct_literals_or_membership(syntax_trees, input)?;
        (vec![expression], rest)
    };
    let ranking_subjects = syntax_trees.expressions.insert_expression_handles(subjects);
    let mut ranking_view = HandleSpan::empty();
    let mut ranking_view_arguments = HandleSpan::empty();

    if rest.at_punctuation(PunctuationKind::Arrow) {
        rest = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
        let (order, next) = parse_path_handle_span(rest, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?;
        ranking_view = order;
        rest = next;

        // TPR3: an ARGUMENTED view (`-> Nat::IncreasingTo(limit)`) -- the
        // bound is part of the view, bound in order to its parameters.
        if rest.at_punctuation(PunctuationKind::LeftParen) {
            let mut argument_input = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let mut arguments = Vec::new();
            loop {
                let (argument, after_argument) =
                    parse_expression_handle_without_struct_literals(syntax_trees, argument_input)?;
                arguments.push(argument);
                if after_argument.at_punctuation(PunctuationKind::Comma) {
                    argument_input =
                        after_argument.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                argument_input =
                    after_argument.take_punctuation(PunctuationKind::RightParen, ")")?;
                break;
            }
            ranking_view_arguments = syntax_trees
                .expressions
                .insert_expression_handles(arguments);
            rest = argument_input;
        }
    }

    let mut ranking_range = psi_syntax_trees::expression::ExpressionHandle::invalid();
    if rest.at_contextual("in") {
        let range_input = rest.take_contextual("in")?;
        // `<start> ..(=) <end>`: ranges only parse structurally in index
        // position, so build the Range node here.
        let (start, after_start) = parse_expression_handle_without_struct_literals_or_membership(
            syntax_trees,
            range_input,
        )?;
        let end_inclusive = if after_start.at_punctuation(PunctuationKind::DotDotEqual) {
            true
        } else if after_start.at_punctuation(PunctuationKind::DotDot) {
            false
        } else {
            return Err(after_start.expected_one_of_here(&["`..`", "`..=`"]));
        };
        let after_separator = if end_inclusive {
            after_start.take_punctuation(PunctuationKind::DotDotEqual, "..=")?
        } else {
            after_start.take_punctuation(PunctuationKind::DotDot, "..")?
        };
        let (end, next) = parse_expression_handle_without_struct_literals_or_membership(
            syntax_trees,
            after_separator,
        )?;
        ranking_range =
            syntax_trees
                .expressions
                .insert(psi_syntax_trees::expression::ExpressionNode::Range(
                    psi_syntax_trees::expression::TableRangeExpression {
                        start,
                        end,
                        end_inclusive,
                    },
                ));
        rest = next;
    }

    Ok((
        (
            ranking_subjects,
            ranking_view,
            ranking_view_arguments,
            ranking_range,
        ),
        rest,
    ))
}

pub(super) fn parse_satisfies_traits<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<SatisfiesClause>> {
    if !input.at_contextual("satisfies") {
        return Ok((HandleSpan::empty(), input));
    }

    input = input.take_contextual("satisfies")?;
    let mut clause_start = Handle::invalid();
    let mut clause_count = 0u32;

    loop {
        let (trait_name, rest) = input.take_identifier()?;
        let (arguments, mut rest) = parse_optional_satisfies_type_arguments(syntax_trees, rest)?;

        // The single-requirement binding (rearrange settle 2026-07-18):
        // `satisfies Trait::requirement [as Alias]` conforms THIS machine to
        // that one requirement; the alias names the satisfier when signatures
        // collide or the same machine fills different slots (plural algebras).
        if !rest.at_punctuation(PunctuationKind::ColonColon) {
            return Err(rest.error_here(format!(
                "bare `satisfies {}` on a machine is retired; bind one exact requirement as `satisfies {}::requirement`",
                trait_name.as_str(),
                trait_name.as_str(),
            )));
        }
        let next = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (member, next) = next.take_identifier()?;
        let requirement = Some(member);
        rest = next;
        let mut alias = None;
        if rest.at_keyword(KeywordKind::As) {
            let next = rest.take_keyword(KeywordKind::As, "as")?;
            let (name, next) = next.take_identifier()?;
            alias = Some(name);
            rest = next;
        }
        // The external-leaf suffix constructs the compiler-known closed
        // `Binding` sum explicitly.
        let mut via = None;
        if rest.at_contextual("via") {
            let next = rest.take_contextual("via")?;
            let (binding, next) = crate::parser::item::parse_external_provider_binding(next)?;
            via = Some(binding);
            rest = next;
        }

        let handle = syntax_trees.items.append_satisfies_clause(SatisfiesClause {
            trait_name,
            arguments,
            requirement,
            alias,
            via,
        });
        if clause_count == 0 {
            clause_start = handle;
        }
        clause_count = clause_count
            .checked_add(1)
            .expect("machine satisfies span count overflow");
        input = rest;

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }

        break;
    }

    let satisfies = if clause_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(clause_start, clause_count)
    };
    Ok((satisfies, input))
}

pub(in crate::parser) fn parse_optional_satisfies_type_arguments<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> Result<
    (
        HandleSpan<psi_syntax_trees::types::TypeReferenceHandle>,
        Input<'tokens, 'source>,
    ),
    crate::parse_error::ParseError,
> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok((HandleSpan::empty(), input));
    }

    input = input.take_punctuation(PunctuationKind::Less, "<")?;
    let mut arguments = Vec::new();
    loop {
        let (argument, rest) = parse_satisfies_type_argument(syntax_trees, input)?;
        arguments.push(argument);
        input = rest;

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }

        let input = input.take_punctuation(PunctuationKind::Greater, ">")?;
        return Ok((
            syntax_trees
                .type_references
                .insert_type_reference_handles(arguments),
            input,
        ));
    }
}

pub(in crate::parser) fn parse_satisfies_type_argument<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, psi_syntax_trees::types::TypeReferenceHandle> {
    if input
        .tokens
        .first()
        .is_some_and(crate::parser::input::is_identifier_token_for_parser)
    {
        let (first, mut rest) = input.take_identifier()?;
        if rest.at_punctuation(PunctuationKind::ColonColon) {
            let mut members = vec![first];
            while rest.at_punctuation(PunctuationKind::ColonColon) {
                rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
                let (member, next) = rest.take_identifier()?;
                members.push(member);
                rest = next;
            }
            return Ok((
                syntax_trees
                    .type_references
                    .insert_named(crate::parser::machine::join_path_identifier(&members)),
                rest,
            ));
        }
    }
    parse_type_reference_handle(syntax_trees, input)
}

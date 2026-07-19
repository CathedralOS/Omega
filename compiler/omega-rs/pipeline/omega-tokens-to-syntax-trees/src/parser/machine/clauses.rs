use crate::parser::expression::{
    parse_expression_handle_without_struct_literals,
    parse_expression_handle_without_struct_literals_or_membership,
};
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::type_reference::parse_type_reference_handle;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{
    BoundaryLevel, CapabilityContract, CapabilityContractKind, SatisfiesClause,
};
use omega_tokens::{KeywordKind, PunctuationKind};

type MachineClauses = (
    bool,
    // TPR2: authored BARE `terminates;` (the public guarantee); the by-form
    // supplies only the witness and leaves this false.
    bool,
    HandleSpan<omega_syntax_trees::expression::ExpressionHandle>,
    HandleSpan<Identifier>,
    // TPR3: argumented-view arguments (`-> Nat::IncreasingTo(limit)`).
    HandleSpan<omega_syntax_trees::expression::ExpressionHandle>,
    omega_syntax_trees::expression::ExpressionHandle,
    HandleSpan<Identifier>,
    HandleSpan<CapabilityContract>,
    omega_syntax_trees::types::TypeReferenceHandle,
);

type RankedSubjects = (
    HandleSpan<omega_syntax_trees::expression::ExpressionHandle>,
    HandleSpan<Identifier>,
    // TPR3: an argumented view's arguments (`-> Nat::IncreasingTo(limit)`).
    HandleSpan<omega_syntax_trees::expression::ExpressionHandle>,
    omega_syntax_trees::expression::ExpressionHandle,
);

pub(super) fn parse_machine_clauses<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, MachineClauses> {
    let mut terminates = false;
    let mut terminates_guarantee = false;
    let mut decreases = HandleSpan::empty();
    let mut decrease_order = HandleSpan::empty();
    let mut decrease_view_arguments = HandleSpan::empty();
    let mut decrease_range = omega_syntax_trees::expression::ExpressionHandle::invalid();
    let mut effect_start = Handle::invalid();
    let mut effect_count = 0u32;
    let mut contract_start = Handle::invalid();
    let mut contract_count = 0u32;
    let mut return_type = omega_syntax_trees::types::TypeReferenceHandle::invalid();

    while !input.at_punctuation(PunctuationKind::LeftBrace)
        // CH10 bodyless machines end at `;` in body position -- the clause
        // loop stands down and leaves the semicolon for parse_machine.
        && !input.at_punctuation(PunctuationKind::Semicolon)
    {
        if input.at_contextual("terminates") {
            input = input.take_contextual("terminates")?;
            terminates = true;
            // Decision 23 (TPR1): bare `terminates;` authors the public
            // guarantee; `terminates by <subjects> [-> View] [in <range>];`
            // supplies the private ranking witness. The old block form is
            // RETIRED loudly below.
            if input.at_contextual("by") {
                let by_input = input.take_contextual("by")?;
                let ((clause_decreases, clause_order, clause_view_arguments, clause_range), rest) =
                    parse_ranked_subjects(syntax_trees, by_input)?;
                input = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
                decreases = clause_decreases;
                decrease_order = clause_order;
                decrease_view_arguments = clause_view_arguments;
                decrease_range = clause_range;
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
            input = input.take_contextual("effects")?;
            while !input.at_punctuation(PunctuationKind::LeftBrace)
                && !input.at_punctuation(PunctuationKind::Semicolon)
                && !input.at_contextual("requires")
                && !input.at_contextual("ensures")
                && !input.at_contextual("terminates")
                && !input.at_contextual("decreases")
                && !input.at_contextual("boundary")
                && !input.at_contextual("where")
                && !input.at_contextual("satisfies")
            {
                let (effect, rest) = input.take_identifier()?;
                let handle = syntax_trees.items.append_identifier_path_member(effect);
                if effect_count == 0 {
                    effect_start = handle;
                }
                effect_count = effect_count
                    .checked_add(1)
                    .expect("machine effect span count overflow");
                input = rest;

                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                }
            }
            continue;
        }

        if input.at_contextual("boundary") {
            let (boundary, rest) = parse_boundary_clause(input)?;
            input = rest;
            let handle = syntax_trees
                .items
                .append_capability_contract(CapabilityContract {
                    kind: CapabilityContractKind::Boundary(boundary),
                    facts: HandleSpan::empty(),
                    token_count: 2,
                });
            if contract_count == 0 {
                contract_start = handle;
            }
            contract_count = contract_count
                .checked_add(1)
                .expect("machine contract span count overflow");
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
                crate::parser::proof_fact::parse_proof_facts_until_with_machine_semicolon(
                    syntax_trees,
                    input,
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
                        || input.at_contextual("effects")
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
                    kind,
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

        // A `where` clause (generic machine requirements, ch12: `where machine
        // T::increment(&mut self)`). Its constraints are consumed up to the
        // body brace -- the same treatment the old skip gave it, but scoped so
        // ONLY a where clause is tolerated here.
        if input.at_contextual("where") {
            input = input.take_contextual("where")?;
            while !input.at_punctuation(PunctuationKind::LeftBrace) {
                let (_, rest) = input.expect_token()?;
                input = rest;
            }
            continue;
        }

        // Anything else between the machine header and its `{` is a mistake --
        // silently skipping it hid dropped return types and typo'd clauses.
        return Err(input.expected_one_of_here(&[
            "`terminates`",
            "`decreases`",
            "`effects`",
            "`boundary`",
            "`requires`",
            "`ensures`",
            "`->`",
            "`{`",
        ]));
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
        (
            terminates,
            terminates_guarantee,
            decreases,
            decrease_order,
            decrease_view_arguments,
            decrease_range,
            effects,
            contracts,
            return_type,
        ),
        input,
    ))
}

fn parse_boundary_clause<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, BoundaryLevel> {
    let input = input.take_contextual("boundary")?;
    if input.at_contextual("host") {
        let input = input.take_contextual("host")?;
        return Ok((BoundaryLevel::Host, input));
    }

    let (name, input) = input.take_identifier()?;
    Ok((BoundaryLevel::Named(name), input))
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
        let (expression, rest) = parse_expression_handle_without_struct_literals_or_membership(
            syntax_trees,
            input,
        )?;
        (vec![expression], rest)
    };
    let decreases = syntax_trees.expressions.insert_expression_handles(subjects);
    let mut decrease_order = HandleSpan::empty();
    let mut decrease_view_arguments = HandleSpan::empty();

    if rest.at_punctuation(PunctuationKind::Arrow) {
        rest = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
        let (order, next) = parse_path_handle_span(rest, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?;
        decrease_order = order;
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
                    argument_input = after_argument.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                argument_input =
                    after_argument.take_punctuation(PunctuationKind::RightParen, ")")?;
                break;
            }
            decrease_view_arguments = syntax_trees.expressions.insert_expression_handles(arguments);
            rest = argument_input;
        }
    }

    let mut decrease_range = omega_syntax_trees::expression::ExpressionHandle::invalid();
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
        decrease_range = syntax_trees.expressions.insert(
            omega_syntax_trees::expression::ExpressionNode::Range(
                omega_syntax_trees::expression::TableRangeExpression {
                    start,
                    end,
                    end_inclusive,
                },
            ),
        );
        rest = next;
    }

    Ok((
        (decreases, decrease_order, decrease_view_arguments, decrease_range),
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
        let mut rest = parse_optional_satisfies_type_arguments(syntax_trees, rest)?;

        // The single-requirement binding (rearrange settle 2026-07-18):
        // `satisfies Trait::requirement [as Alias]` conforms THIS machine to
        // that one requirement; the alias names the satisfier when signatures
        // collide or the same machine fills different slots (plural algebras).
        let mut requirement = None;
        if rest.at_punctuation(PunctuationKind::ColonColon) {
            let next = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let (member, next) = next.take_identifier()?;
            requirement = Some(member);
            rest = next;
        }
        let mut alias = None;
        if rest.at_keyword(KeywordKind::As) {
            let next = rest.take_keyword(KeywordKind::As, "as")?;
            let (name, next) = next.take_identifier()?;
            alias = Some(name);
            rest = next;
        }
        // PRV4 step 1: the external-leaf suffix. `via <Binding>` reuses the
        // provides grammar's closed binding sum verbatim.
        let mut via = None;
        if rest.at_contextual("via") {
            let next = rest.take_contextual("via")?;
            let (binding, next) =
                crate::parser::item::parse_external_provider_binding(next)?;
            via = Some(binding);
            rest = next;
        }

        let handle = syntax_trees.items.append_satisfies_clause(SatisfiesClause {
            trait_name,
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

fn parse_optional_satisfies_type_arguments<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> Result<Input<'tokens, 'source>, crate::parse_error::ParseError> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok(input);
    }

    input = input.take_punctuation(PunctuationKind::Less, "<")?;
    loop {
        let (_argument, rest) = parse_type_reference_handle(syntax_trees, input)?;
        input = rest;

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }

        return input.take_punctuation(PunctuationKind::Greater, ">");
    }
}

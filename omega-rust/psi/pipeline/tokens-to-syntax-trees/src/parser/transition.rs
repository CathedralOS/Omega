use crate::parse_error::ParseError;
use crate::parser::input::{Input, ParseResult, is_identifier_token_for_parser};
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{
    StatementHandle, StatementNode, TableTransition, TransitionTargetHandle, TransitionTargetNode,
};
use tokens::{KeywordKind, PunctuationKind};

mod guards;
mod targets;

use guards::{parse_transition_expression_list, parse_transition_guard_node};
pub(super) use targets::parse_transition_block_target_handle;
use targets::parse_transition_block_target_with_bindings;

pub(super) fn parse_transition_block_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let (mut subject, mut input) = if input.at_punctuation(PunctuationKind::LeftBrace) {
        (
            Vec::new(),
            input.take_punctuation(PunctuationKind::LeftBrace, "{")?,
        )
    } else {
        let (expressions, rest) = parse_expression_list_until_punctuation(
            syntax_trees,
            input,
            PunctuationKind::LeftBrace,
        )?;
        let input = rest.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        (expressions, input)
    };

    // A transition SUBJECT is evaluated once, before arm dispatch. Pure places
    // can be re-read directly; computed subjects (notably value-machine calls)
    // are captured into generated locals and every guard / pattern extraction
    // below reads that place. Besides making the general transition law
    // explicit, this gives record-pattern validation an inferred DECLARED type
    // to resolve: `transition self.make() { Point { x, y } -> ... }` must not
    // call `make` once for `x` and again for `y`, nor skip the field law merely
    // because the authored subject was not already a place.
    let mut subject_captures: Vec<(Identifier, ExpressionHandle)> = Vec::new();
    if transition_contains_destructure_pattern(input) {
        for subject_expression in &mut subject {
            if expression_is_place(syntax_trees, *subject_expression) {
                continue;
            }
            let captured = *subject_expression;
            let name =
                Identifier::generated(format!("__transition_subject#{}", captured.arena_index()));
            let path_start = syntax_trees
                .expressions
                .append_identifier_path_member(name.clone());
            let path = HandleSpan::from_parts(path_start, 1);
            *subject_expression = syntax_trees.expressions.insert(ExpressionNode::Name(path));
            subject_captures.push((name, captured));
        }
    }

    let mut start = Handle::invalid();
    let mut count = 0u32;
    let mut arm_statements: Vec<StatementHandle> = Vec::new();
    let mut arm_bool_tuples: Vec<Option<Vec<Option<bool>>>> = Vec::new();
    // Parsed arms are COLLECTED first and their Transition statements
    // appended after the loop, so exhaustiveness MARKER lets (below) can
    // precede every arm statement in the item list -- the returned span
    // must be one contiguous run.
    let mut parsed_arms: Vec<(
        syntax_trees::statement::TransitionGuardNode,
        TransitionTargetHandle,
        arena::HandleSpan<syntax_trees::statement::TableOutcomeProofSelector>,
        source::SourceSpan,
    )> = Vec::new();
    // (marker name, subject) per destructure arm. Computed transition subjects
    // have already become generated places above, so every arm has a declared
    // type carrier for the missing/unknown-field law. Deduplicated by name
    // (identical patterns share one marker; duplicate local names would
    // collide in symbol resolution).
    let mut pattern_markers: Vec<(String, ExpressionHandle)> = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (guard, bindings, bool_tuple, rest) =
            parse_transition_guard_node(syntax_trees, input, &subject)?;
        let source_span = rest
            .tokens
            .first()
            .map(|token| rest.source_span(token))
            .expect("recognized transition arrow has a source token");
        input = rest.take_punctuation(PunctuationKind::Arrow, "->")?;

        let (target, rest) = if input.at_punctuation(PunctuationKind::LeftBrace) {
            let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
            let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
            (
                syntax_trees
                    .statements
                    .insert_transition_target(TransitionTargetNode::Terminal),
                input,
            )
        } else {
            parse_transition_block_target_with_bindings(syntax_trees, input, &bindings)?
        };
        input = rest;

        // Exhaustiveness law (owner spec 2026-07-18): a `..`-free destructure
        // arm must spell every field of its record/case. The spelled set is
        // encoded into a marker let (`__arm_destructure#V=<variant>#<f1>...`;
        // `#`/`=` cannot appear in identifiers, so the split is unambiguous)
        // whose initializer is the subject place -- the typed-stage
        // validation resolves it against the data definition.
        for arm_binding in &bindings {
            let Some(spelling) = arm_binding.spelling.as_ref() else {
                continue;
            };
            if !expression_is_place(syntax_trees, arm_binding.subject) {
                continue;
            }
            let mut marker_name = String::from("__arm_destructure#V=");
            if let Some(variant) = spelling.variant.as_ref() {
                marker_name.push_str(variant.as_str());
            }
            for member in &spelling.members {
                marker_name.push('#');
                marker_name.push_str(member.as_str());
            }
            // Two tuple axes may spell the same pattern. Keep validation
            // carriers keyed by subject without exposing the generated id as
            // an authored field name; validation strips this sentinel.
            marker_name.push_str("#~subject=");
            marker_name.push_str(&arm_binding.subject.arena_index().to_string());
            // `..` opts out of the MISSING-field law but spelled fields must
            // still EXIST -- the trailing `#~rest` sentinel tells validation
            // to skip only the exhaustiveness half (`~` cannot appear in an
            // identifier).
            if spelling.has_rest {
                marker_name.push_str("#~rest");
            }
            if !pattern_markers
                .iter()
                .any(|(name, subject)| *name == marker_name && *subject == arm_binding.subject)
            {
                pattern_markers.push((marker_name, arm_binding.subject));
            }
        }

        let proof_selectors = bindings
            .first()
            .map(|bindings| {
                syntax_trees
                    .statements
                    .insert_outcome_proof_selectors(bindings.proof_selectors.iter().cloned())
            })
            .unwrap_or_else(HandleSpan::empty);
        parsed_arms.push((guard, target, proof_selectors, source_span));
        arm_bool_tuples.push(bool_tuple);
    }

    for (name, initial_value) in subject_captures {
        let capture = syntax_trees.statements.insert(StatementNode::LocalData(
            syntax_trees::statement::TableLocalData {
                name,
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                initial_value,
                is_mutable: false,
            },
        ));
        let handle = syntax_trees.items.append_statement_handle(capture);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("transition block statement span count overflow");
    }

    for (marker_name, subject_place) in pattern_markers {
        let marker = syntax_trees.statements.insert(StatementNode::LocalData(
            syntax_trees::statement::TableLocalData {
                name: syntax_trees::identifier::Identifier::generated(marker_name),
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                initial_value: subject_place,
                is_mutable: false,
            },
        ));
        let handle = syntax_trees.items.append_statement_handle(marker);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("transition block statement span count overflow");
    }

    for (guard, target, proof_selectors, source_span) in parsed_arms {
        let statement =
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: TransitionTargetHandle::invalid(),
                    guard,
                    proof_selectors,
                    exit: Default::default(),
                    source_span,
                }));
        let handle = syntax_trees.items.append_statement_handle(statement);
        arm_statements.push(statement);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("transition block statement span count overflow");
    }

    // BOOL-TUPLE exhaustiveness (ch4): when every arm is a bool-literal /
    // wildcard tuple and the matrix covers {true,false}^N, the LAST arm's
    // guard rewrites to the fall-through -- dispatch tests arms in order, so
    // an input reaching the last arm of a covering matrix must match it.
    // Non-bool arms or an uncovered matrix change nothing (the fall-through
    // checker keeps its refusal).
    if subject.len() >= 2
        && arm_bool_tuples.iter().all(|tuple| {
            tuple
                .as_ref()
                .is_some_and(|tuple| tuple.len() == subject.len())
        })
        && subject.len() <= 8
    {
        let matrices: Vec<&Vec<Option<bool>>> = arm_bool_tuples
            .iter()
            .map(|t| t.as_ref().unwrap())
            .collect();
        let covered = (0u32..(1u32 << subject.len())).all(|combo| {
            matrices.iter().any(|arm| {
                arm.iter().enumerate().all(|(bit, pattern)| match pattern {
                    None => true,
                    Some(value) => *value == ((combo >> bit) & 1 == 1),
                })
            })
        });
        if covered
            && let Some(last) = arm_statements.last()
            && let StatementNode::Transition(transition) =
                syntax_trees.statements.statement(*last).clone()
        {
            syntax_trees.statements.replace_statement(
                *last,
                StatementNode::Transition(TableTransition {
                    guard: syntax_trees::statement::TransitionGuardNode::Always,
                    ..transition
                }),
            );
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let statements = if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    };
    Ok((statements, input))
}

/// A pure re-readable place (Name / self / member chain) -- the same gate as
/// the LET destructure's value: the marker's initializer re-reads the subject,
/// which must not double-evaluate a call.
fn expression_is_place(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    match syntax_trees.expressions.expression(expression) {
        syntax_trees::expression::ExpressionNode::Name(_)
        | syntax_trees::expression::ExpressionNode::SelfValue => true,
        syntax_trees::expression::ExpressionNode::Member(member) => {
            expression_is_place(syntax_trees, member.receiver)
        }
        _ => false,
    }
}

/// A cheap structural lookahead for the one syntax that needs a computed
/// subject capture. It recognizes plain-record `Type { .. } [if ..] ->`
/// arms anywhere in the token stream. A case-pattern arm is included only when
/// its braces contain the outcome proof-lane separator; this preserves the
/// structural form of older proof-only recursive case machines while ensuring
/// a selected-witness producer call executes exactly once. Requiring
/// the pattern's closing brace to be followed immediately by `->` or `if`
/// distinguishes it from a struct-literal arm TARGET, whose closing brace is
/// followed by the next arm's pattern.
fn transition_contains_destructure_pattern(input: Input<'_, '_>) -> bool {
    // `Input` is the remainder of the SOURCE, not a slice already bounded to
    // this transition. Stop at this block's own closing brace or a later
    // machine/state containing a record arm would spuriously capture an
    // earlier scalar transition subject.
    let mut tokens = Vec::new();
    let mut outer_brace_depth = 0usize;
    for token in input.tokens.iter().filter(|token| !token.is_non_semantic()) {
        match token.punctuation() {
            Some(PunctuationKind::LeftBrace) => outer_brace_depth += 1,
            Some(PunctuationKind::RightBrace) if outer_brace_depth == 0 => break,
            Some(PunctuationKind::RightBrace) => outer_brace_depth -= 1,
            _ => {}
        }
        tokens.push(token);
    }

    for start in 0..tokens.len() {
        if !is_identifier_token_for_parser(tokens[start]) {
            continue;
        }
        if start > 0 && tokens[start - 1].punctuation() == Some(PunctuationKind::ColonColon) {
            continue;
        }
        let mut cursor = start + 1;
        let mut case_proof_lane = false;
        if tokens
            .get(cursor)
            .is_some_and(|token| token.punctuation() == Some(PunctuationKind::ColonColon))
        {
            cursor += 1;
            if !tokens
                .get(cursor)
                .is_some_and(|token| is_identifier_token_for_parser(token))
            {
                continue;
            }
            cursor += 1;
            case_proof_lane = true;
        }
        if !tokens
            .get(cursor)
            .is_some_and(|token| token.punctuation() == Some(PunctuationKind::LeftBrace))
        {
            continue;
        }

        let mut depth = 1usize;
        let mut has_proof_separator = false;
        cursor += 1;
        while cursor < tokens.len() && depth > 0 {
            match tokens[cursor].punctuation() {
                Some(PunctuationKind::LeftBrace) => depth += 1,
                Some(PunctuationKind::RightBrace) => depth -= 1,
                Some(PunctuationKind::Semicolon) if depth == 1 => {
                    has_proof_separator = true;
                }
                _ => {}
            }
            cursor += 1;
        }
        if depth != 0 {
            continue;
        }
        if tokens
            .get(cursor)
            .is_some_and(|token| token.punctuation() == Some(PunctuationKind::Arrow))
            && (!case_proof_lane || has_proof_separator)
        {
            return true;
        }
        // Tuple destructures have `, ... )` between a component's closing
        // brace and the arm arrow. Seeing the tuple close before that arrow
        // distinguishes the pattern side from a struct-literal target.
        if tokens.get(cursor).is_some_and(|token| {
            matches!(
                token.punctuation(),
                Some(PunctuationKind::Comma | PunctuationKind::RightParen)
            )
        }) {
            let mut nested_braces = 0usize;
            while cursor < tokens.len() {
                match tokens[cursor].punctuation() {
                    Some(PunctuationKind::LeftBrace) => nested_braces += 1,
                    Some(PunctuationKind::RightBrace) if nested_braces > 0 => nested_braces -= 1,
                    Some(PunctuationKind::RightParen) if nested_braces == 0 => {
                        let next = tokens[cursor + 1..]
                            .iter()
                            .find(|token| !token.is_non_semantic());
                        if next.is_some_and(|token| {
                            token.punctuation() == Some(PunctuationKind::Arrow)
                                || token.keyword() == Some(KeywordKind::If)
                        }) {
                            return true;
                        }
                    }
                    _ => {}
                }
                cursor += 1;
            }
            continue;
        }
        if !tokens
            .get(cursor)
            .is_some_and(|token| token.keyword() == Some(KeywordKind::If))
        {
            continue;
        }

        let mut paren = 0usize;
        let mut bracket = 0usize;
        let mut brace = 0usize;
        cursor += 1;
        while cursor < tokens.len() {
            match tokens[cursor].punctuation() {
                Some(PunctuationKind::LeftParen) => paren += 1,
                Some(PunctuationKind::RightParen) => paren = paren.saturating_sub(1),
                Some(PunctuationKind::LeftBracket) => bracket += 1,
                Some(PunctuationKind::RightBracket) => bracket = bracket.saturating_sub(1),
                Some(PunctuationKind::LeftBrace) => brace += 1,
                Some(PunctuationKind::RightBrace) if brace > 0 => brace -= 1,
                Some(PunctuationKind::Arrow) if paren == 0 && bracket == 0 && brace == 0 => {
                    return true;
                }
                _ => {}
            }
            cursor += 1;
        }
    }
    false
}

fn parse_expression_list_until_punctuation<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    delimiter: PunctuationKind,
) -> Result<(Vec<ExpressionHandle>, Input<'tokens, 'source>), ParseError> {
    let (expression_input, rest) =
        input.split_at_top_level_punctuation(delimiter, "expected transition block delimiter")?;
    let (expressions, rest_after_expression) =
        parse_transition_expression_list(syntax_trees, expression_input)?;

    if !rest_after_expression.tokens.is_empty() {
        return Err(rest_after_expression.error_here("expected transition subject expression"));
    }

    Ok((expressions, rest))
}

#[cfg(test)]
mod tests {
    use super::transition_contains_destructure_pattern;
    use crate::parser::input::Input;
    use source::SourceId;
    use source_files_to_tokens::Lexer;

    #[test]
    fn destructure_lookahead_finds_a_later_record_arm() {
        let tokens = Lexer::new("_ -> fallback() Point { x, y } if x > 0 -> done(x, y) }")
            .tokenize()
            .expect("tokenize transition arms")
            .into_tokens();
        assert!(transition_contains_destructure_pattern(Input::new(
            SourceId::default(),
            &tokens
        )));
    }

    #[test]
    fn destructure_lookahead_finds_tuple_record_components() {
        let tokens = Lexer::new(
            "(Pair { left as a, right as _ }, Pair { left as b, right as _ }) -> done(a, b) }",
        )
        .tokenize()
        .expect("tokenize tuple destructure arm")
        .into_tokens();
        assert!(transition_contains_destructure_pattern(Input::new(
            SourceId::default(),
            &tokens
        )));
    }

    #[test]
    fn destructure_lookahead_does_not_confuse_a_struct_literal_target() {
        let tokens =
            Lexer::new("_ -> Point { x: 1, y: 2 } _ -> fallback() } Point { x, y } -> later()")
                .tokenize()
                .expect("tokenize transition arms")
                .into_tokens();
        assert!(!transition_contains_destructure_pattern(Input::new(
            SourceId::default(),
            &tokens
        )));
    }

    #[test]
    fn destructure_lookahead_leaves_case_patterns_structural() {
        let tokens = Lexer::new("Nat::Zero -> base() Nat::Succ { prev } -> step(prev) }")
            .tokenize()
            .expect("tokenize transition case arms")
            .into_tokens();
        assert!(!transition_contains_destructure_pattern(Input::new(
            SourceId::default(),
            &tokens
        )));
    }
}

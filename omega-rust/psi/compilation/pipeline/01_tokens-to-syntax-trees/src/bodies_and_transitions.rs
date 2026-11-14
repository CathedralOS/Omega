/*
Machine state bodies, trait-default bodies, statement sequences, and transition
guards and targets (`grammar::bodies` and `grammar::bodies::transitions`).
*/

pub(crate) mod bodies {
    pub(crate) mod parse_body {
        use super::sequence::parse_statements;
        use crate::grammar::declarations::parse_declaration::{
            ParsedDeclaration, parse_item, starts_machine_declaration,
        };
        use crate::tokens::token_cursor::{Input, ParseResult};
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::identifier::Identifier;
        use syntax_trees::item::{Item, State, StateHandle, StateParameterHandle};
        use syntax_trees::types::TypeReferenceHandle;
        use tokens::{KeywordKind, PunctuationKind};

        pub(crate) struct ParsedMachineBody {
            pub states: HandleSpan<StateHandle>,
            pub nested_machines: HandleSpan<Item>,
            pub bodyless: bool,
        }

        pub(crate) fn parse_body<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            mut input: Input<'tokens, 'source>,
            entry_name: Option<Identifier>,
            machine_parameters: HandleSpan<StateParameterHandle>,
            machine_return_type: TypeReferenceHandle,
        ) -> ParseResult<'tokens, 'source, ParsedMachineBody> {
            if input.at_punctuation(PunctuationKind::Semicolon) {
                let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
                let entry_state = State {
                    name: entry_name.unwrap_or_else(|| Identifier::generated("entry")),
                    parameters: machine_parameters,
                    return_type: machine_return_type,
                    contracts: HandleSpan::empty(),
                    statements: HandleSpan::empty(),
                };
                let state = syntax_trees.items.insert_state(&entry_state);
                let state = syntax_trees.items.append_state_handle(state);
                return Ok((
                    ParsedMachineBody {
                        states: HandleSpan::from_parts(state, 1),
                        nested_machines: HandleSpan::empty(),
                        bodyless: true,
                    },
                    input,
                ));
            }

            input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
            let mut children = Vec::new();
            let implicit_entry_name = entry_name
                .clone()
                .unwrap_or_else(|| Identifier::generated("entry"));

            let (state, rest) = parse_implicit_entry_state(
                syntax_trees,
                input,
                implicit_entry_name,
                machine_parameters,
                machine_return_type,
            )?;
            let state = syntax_trees.items.insert_state(&state);
            let state = syntax_trees.items.append_state_handle(state);
            let states = HandleSpan::from_parts(state, 1);
            input = rest;

            while !input.at_punctuation(PunctuationKind::RightBrace) {
                if starts_machine_declaration(input) {
                    let declaration_start = input;
                    let (declaration, rest) = parse_item(syntax_trees, input)?;
                    let ParsedDeclaration::Item(Item::Machine(machine)) = declaration else {
                        return Err(
                            declaration_start.error_here("expected a nested machine declaration")
                        );
                    };
                    if machine.is_public {
                        return Err(declaration_start.error_here(
                    "nested machines are lexical declarations and cannot be package-exported",
                ));
                    }
                    children.push(Item::Machine(machine));
                    input = rest;
                    continue;
                }
                if starts_state_member(input) {
                    return Err(input.error_here(
                "nested `state` declarations are retired; declare a nested `machine` with its own signature and contract",
            ));
                }
                if !children.is_empty() {
                    return Err(input.error_here(
                        "executable statements cannot follow nested machine declarations",
                    ));
                }
                if starts_retired_entry_member(input)
                    || (input.at_keyword(KeywordKind::Public)
                        && input
                            .take_keyword(KeywordKind::Public, "public")?
                            .at_contextual("entry"))
                {
                    return Err(input.error_here(
                "explicit nested `entry` / `public entry` machine members are retired; put the callable signature on the `machine` head, put its entry statements directly in the machine body, and put visibility on the declaration as `public machine`",
            ));
                }

                if starts_retired_invariant_member(input) {
                    return Err(input.error_here(
                        "the `invariant` machine member is retired: state arrival facts use \
                 `requires`, result facts use `ensures`, and loop facts are derived from \
                 checked transitions",
                    ));
                }
                return Err(input.expected_one_of_here(&["nested `machine` declaration", "`}`"]));
            }

            input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
            let mut nested_machines = crate::storage::handle_spans::HandleSpanBuilder::new();
            for child in children {
                nested_machines.push(syntax_trees.items.append_item(child));
            }
            Ok((
                ParsedMachineBody {
                    states,
                    nested_machines: nested_machines.finish(),
                    bodyless: false,
                },
                input,
            ))
        }

        pub(crate) fn starts_machine_member(input: Input<'_, '_>) -> bool {
            input.at_punctuation(PunctuationKind::RightBrace)
                || input.at_keyword(KeywordKind::Public)
                || input.at_keyword(KeywordKind::Machine)
                || starts_retired_entry_member(input)
                || starts_state_member(input)
                || starts_retired_invariant_member(input)
                || starts_machine_declaration(input)
        }

        fn starts_retired_entry_member(input: Input<'_, '_>) -> bool {
            if !input.at_contextual("entry") {
                return false;
            }
            let after_name = input.restarted(input.tokens.get(1..).unwrap_or_default());
            if after_name
                .tokens
                .first()
                .is_some_and(crate::tokens::token_cursor::is_identifier_token_for_parser)
            {
                return true;
            }
            if !after_name.at_punctuation(PunctuationKind::LeftParen) {
                return false;
            }
            let mut nesting = 0u32;
            for (position, token) in after_name.tokens.iter().enumerate() {
                match token.kind {
                    tokens::TokenKind::Punctuation(PunctuationKind::LeftParen) => nesting += 1,
                    tokens::TokenKind::Punctuation(PunctuationKind::RightParen) => {
                        nesting = nesting.saturating_sub(1);
                        if nesting == 0 {
                            let tail = input.restarted(
                                after_name.tokens.get(position + 1..).unwrap_or_default(),
                            );
                            return tail.at_punctuation(PunctuationKind::LeftBrace)
                                || tail.at_punctuation(PunctuationKind::Arrow)
                                || [
                                    "requires", "ensures", "crashes", "reaches", "blocks",
                                    "suspends",
                                ]
                                .iter()
                                .any(|keyword| tail.at_contextual(keyword));
                        }
                    }
                    _ => {}
                }
            }
            false
        }

        fn starts_retired_invariant_member(input: Input<'_, '_>) -> bool {
            if !input.at_contextual("invariant") {
                return false;
            }

            let after_keyword = input.restarted(input.tokens.get(1..).unwrap_or_default());
            if !after_keyword
                .tokens
                .first()
                .is_some_and(crate::tokens::token_cursor::is_identifier_token_for_parser)
            {
                return false;
            }

            input
                .restarted(after_keyword.tokens.get(1..).unwrap_or_default())
                .at_punctuation(PunctuationKind::LeftBrace)
        }

        fn starts_state_member(input: Input<'_, '_>) -> bool {
            if !input.at_keyword(KeywordKind::State) {
                return false;
            }

            input
                .restarted(input.tokens.get(1..).unwrap_or_default())
                .tokens
                .first()
                .is_some_and(crate::tokens::token_cursor::is_identifier_token_for_parser)
        }

        fn parse_implicit_entry_state<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
            name: Identifier,
            parameters: HandleSpan<StateParameterHandle>,
            return_type: TypeReferenceHandle,
        ) -> ParseResult<'tokens, 'source, State> {
            let (statements, input) =
                parse_statements(syntax_trees, input, return_type.is_valid())?;
            Ok((
                State {
                    name,
                    parameters,
                    return_type,
                    contracts: HandleSpan::empty(),
                    statements,
                },
                input,
            ))
        }
    }
    pub(crate) mod sequence {
        use crate::grammar::bodies::statements::parse_statement::parse_statement_handles;
        use crate::storage::tables::{items_of, stmts_of};
        use crate::tokens::token_cursor::{Input, ParseResult};
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::statement::StatementHandle;
        use tokens::PunctuationKind;

        pub(crate) fn parse_statements<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            mut input: Input<'tokens, 'source>,
            requires_result: bool,
        ) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
            let mut statements = crate::storage::handle_spans::HandleSpanBuilder::new();
            while (requires_result && input.at_identifier_then_contextual("machine"))
                || !super::parse_body::starts_machine_member(input)
            {
                if input.at_punctuation(PunctuationKind::Arrow) {
                    return Err(input.error_here(
                "machine entry bodies must use the `transition` keyword; bare `->` transitions are not supported",
            ));
                }
                let (parsed, rest) = parse_statement_handles(syntax_trees, input)?;
                let completes_body = items_of(syntax_trees)
                    .statements(parsed)
                    .last()
                    .is_some_and(|statement| {
                        matches!(
                            stmts_of(syntax_trees).statement(*statement),
                            syntax_trees::statement::StatementNode::Transition(_)
                                | syntax_trees::statement::StatementNode::Expression(_)
                        )
                    })
                    && crate::grammar::declarations::parse_declaration::starts_machine_declaration(
                        rest,
                    );
                statements.push_span(parsed);
                input = rest;
                if completes_body {
                    break;
                }
            }
            Ok((statements.finish(), input))
        }
    }
    pub(crate) use crate::statements_and_assembly::statements;
    pub(crate) mod trait_default {
        use crate::diagnostics::parse_error::ParseError;
        use crate::grammar::bodies::statements::parse_statement::parse_statement_handles;
        use crate::tokens::token_cursor::Input;
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::statement::StatementHandle;
        use tokens::PunctuationKind;

        pub(crate) fn parse_trait_default_machine_body<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> Result<(HandleSpan<StatementHandle>, Input<'tokens, 'source>), ParseError> {
            let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
            let mut body = crate::storage::handle_spans::HandleSpanBuilder::new();
            while !input.at_punctuation(PunctuationKind::RightBrace) {
                let (statements, rest) = parse_statement_handles(syntax_trees, input)?;
                body.push_span(statements);
                input = rest;
            }
            let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
            Ok((body.finish(), input))
        }
    }
    pub(crate) mod transitions {
        pub(crate) mod guards {

            use crate::diagnostics::parse_error::ParseError;
            use crate::grammar::expressions::parse_expression::parse_expression_handle_without_struct_literals;
            use crate::storage::tables::{exprs, exprs_of};
            use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
            use syntax_trees::SyntaxTrees;
            use syntax_trees::expression::{
                BinaryOperator, ExpressionHandle, ExpressionNode, MatchFieldAction,
                TableBinaryExpression, UnaryOperator,
            };
            use syntax_trees::statement::TableOutcomeProofSelector;
            use syntax_trees::statement::TransitionGuardNode;
            use tokens::{KeywordKind, PunctuationKind};

            pub(crate) use crate::grammar::patterns::bindings::{
                DestructureBinding, rewrite_destructure_guard_expression,
            };

            pub(crate) struct DestructureBindings {
                pub(crate) subject: ExpressionHandle,
                pub(crate) fields: Vec<DestructureBinding>,
                pub(crate) pattern: Option<syntax_trees::statement::TransitionPattern>,
                pub(crate) proof_selectors: Vec<TableOutcomeProofSelector>,
            }

            pub(crate) fn parse_transition_guard_node<'tokens, 'source>(
                syntax_trees: &mut SyntaxTrees,
                input: Input<'tokens, 'source>,
                subject: &[ExpressionHandle],
            ) -> Result<
                (
                    TransitionGuardNode,
                    Vec<DestructureBindings>,
                    Option<Vec<Option<bool>>>,
                    Input<'tokens, 'source>,
                ),
                ParseError,
            > {
                let (pattern_input, rest) = input
                    .split_at_top_level_punctuation(PunctuationKind::Arrow, "expected `->`")?;

                if subject.len() == 1
                    && let Some((guard, bindings)) =
                        parse_destructure_pattern_arm(syntax_trees, pattern_input, subject[0])?
                {
                    return Ok((guard, vec![bindings], None, rest));
                }

                if subject.len() >= 2
                    && let Some((guard, bindings)) =
                        parse_tuple_destructure_pattern_arm(syntax_trees, pattern_input, subject)?
                {
                    return Ok((guard, bindings, None, rest));
                }

                let (patterns, pattern_rest) =
                    parse_transition_match_list(syntax_trees, pattern_input, true)?;
                if !pattern_rest.tokens.is_empty() {
                    return Err(pattern_rest.error_here("expected transition pattern"));
                }

                if subject.is_empty() {
                    if patterns.len() == 1 {
                        let guard = match patterns.into_iter().next().flatten() {
                            Some(expression) => TransitionGuardNode::When(expression),
                            None => TransitionGuardNode::Always,
                        };
                        return Ok((guard, Vec::new(), None, rest));
                    }
                    return Err(input
                        .error_here("anonymous transition blocks do not support tuple patterns"));
                }

                if patterns.len() == 1 && patterns[0].is_none() {
                    return Ok((TransitionGuardNode::Always, Vec::new(), None, rest));
                }

                if subject.len() != patterns.len() {
                    return Err(input.error_here(format!(
                        "transition pattern arity {} does not match subject arity {}",
                        patterns.len(),
                        subject.len()
                    )));
                }

                let bool_tuple: Option<Vec<Option<bool>>> = patterns
                    .iter()
                    .map(|pattern| match pattern {
                        None => Some(None),
                        Some(handle) => match exprs_of(syntax_trees).expression(*handle) {
                            ExpressionNode::Boolean(value) => Some(Some(*value)),
                            _ => None,
                        },
                    })
                    .collect();
                let mut bindings = Vec::new();
                for (left, right) in subject.iter().copied().zip(patterns) {
                    let Some(right) = right else {
                        continue;
                    };
                    retain_value_pattern(syntax_trees, left, right, &mut bindings);
                }

                Ok((TransitionGuardNode::Always, bindings, bool_tuple, rest))
            }

            fn arm_pattern_is_condition(
                syntax_trees: &SyntaxTrees,
                expression: ExpressionHandle,
            ) -> bool {
                match exprs_of(syntax_trees).expression(expression) {
                    ExpressionNode::Binary(binary) => matches!(
                        binary.operator,
                        BinaryOperator::And
                            | BinaryOperator::Or
                            | BinaryOperator::Equal
                            | BinaryOperator::NotEqual
                            | BinaryOperator::Greater
                            | BinaryOperator::GreaterOrEqual
                            | BinaryOperator::Less
                            | BinaryOperator::LessOrEqual
                    ),
                    ExpressionNode::Membership(_) => true,
                    ExpressionNode::Unary(unary) => unary.operator == UnaryOperator::LogicalNot,
                    _ => false,
                }
            }

            fn parse_tuple_destructure_pattern_arm<'tokens, 'source>(
                syntax_trees: &mut SyntaxTrees,
                input: Input<'tokens, 'source>,
                subjects: &[ExpressionHandle],
            ) -> Result<Option<(TransitionGuardNode, Vec<DestructureBindings>)>, ParseError>
            {
                if !input.at_punctuation(PunctuationKind::LeftParen) {
                    return Ok(None);
                }

                let (tuple_input, guard_input) =
                    match find_top_level_keyword(input, KeywordKind::If) {
                        Some(if_index) => {
                            let (tuple_tokens, guard_tokens_with_if) =
                                input.tokens.split_at(if_index);
                            (
                                input.restarted(tuple_tokens),
                                Some(
                                    input.restarted(
                                        guard_tokens_with_if
                                            .get(1..)
                                            .expect("if keyword split should include guard tokens"),
                                    ),
                                ),
                            )
                        }
                        None => (input, None),
                    };
                let components = split_tuple_pattern_components(tuple_input)?;
                if !components.iter().any(|component| {
                    component
                        .tokens
                        .iter()
                        .any(|token| token.punctuation() == Some(PunctuationKind::LeftBrace))
                }) {
                    return Ok(None);
                }
                if components.len() != subjects.len() {
                    return Err(input.error_here(format!(
                        "transition pattern arity {} does not match subject arity {}",
                        components.len(),
                        subjects.len()
                    )));
                }

                let mut combined = ExpressionHandle::invalid();
                let mut all_bindings = Vec::new();
                for (component, subject) in components.into_iter().zip(subjects.iter().copied()) {
                    if component.at_contextual("_") && component.tokens.len() == 1 {
                        continue;
                    }
                    if let Some((guard, bindings)) =
                        parse_destructure_pattern_arm(syntax_trees, component, subject)?
                    {
                        if !bindings.proof_selectors.is_empty() {
                            return Err(component.error_here(
                    "outcome proof selectors are admitted only on a single exact result-case arm",
                ));
                        }
                        if let TransitionGuardNode::When(expression) = guard {
                            combined = join_guard_conjunction(syntax_trees, combined, expression);
                        }
                        all_bindings.push(bindings);
                        continue;
                    }

                    let (pattern, rest) = parse_transition_match_component::<
                        Option<ExpressionHandle>,
                    >(syntax_trees, component, true)?;
                    if !rest.tokens.is_empty() {
                        return Err(rest.error_here("expected tuple transition pattern component"));
                    }
                    if let Some(pattern) = pattern {
                        retain_value_pattern(syntax_trees, subject, pattern, &mut all_bindings);
                    }
                }

                for (binding_index, bindings) in all_bindings.iter().enumerate() {
                    for field in &bindings.fields {
                        if all_bindings[..binding_index].iter().any(|earlier| {
                            earlier.fields.iter().any(|other| {
                                other.binding.as_str(syntax_trees)
                                    == field.binding.as_str(syntax_trees)
                            })
                        }) {
                            return Err(input.error_here(format!(
                    "tuple destructure binding `{}` is introduced by more than one subject; rename one binding with `as`",
                    field.binding.as_str(syntax_trees)
                )));
                        }
                    }
                }

                if let Some(guard_input) = guard_input {
                    let (guard, rest) =
                        parse_expression_handle_without_struct_literals(syntax_trees, guard_input)?;
                    if !rest.tokens.is_empty() {
                        return Err(rest.error_here("expected transition pattern guard"));
                    }
                    let guard = all_bindings.iter().fold(guard, |guard, bindings| {
                        rewrite_destructure_guard_expression(
                            syntax_trees,
                            guard,
                            bindings.subject,
                            &bindings.fields,
                        )
                    });
                    combined = join_guard_conjunction(syntax_trees, combined, guard);
                }

                Ok(Some((
                    if combined.is_valid() {
                        TransitionGuardNode::When(combined)
                    } else {
                        TransitionGuardNode::Always
                    },
                    all_bindings,
                )))
            }

            fn split_tuple_pattern_components<'tokens, 'source>(
                input: Input<'tokens, 'source>,
            ) -> Result<Vec<Input<'tokens, 'source>>, ParseError> {
                let tokens = input.tokens;
                if tokens.first().and_then(|token| token.punctuation())
                    != Some(PunctuationKind::LeftParen)
                {
                    return Err(input.error_here("expected `(` to open tuple transition pattern"));
                }

                let mut paren = 0usize;
                let mut bracket = 0usize;
                let mut brace = 0usize;
                let mut component_start = 1usize;
                let mut components = Vec::new();
                for (index, token) in tokens.iter().enumerate().skip(1) {
                    match token.punctuation() {
                        Some(PunctuationKind::LeftParen) => paren += 1,
                        Some(PunctuationKind::RightParen) if paren > 0 => paren -= 1,
                        Some(PunctuationKind::RightParen) if bracket == 0 && brace == 0 => {
                            if tokens[index + 1..]
                                .iter()
                                .any(|token| !token.is_non_semantic())
                            {
                                return Err(input.restarted(&tokens[index + 1..]).error_here(
                                    "expected `if` or `->` after tuple transition pattern",
                                ));
                            }
                            components.push(input.restarted(&tokens[component_start..index]));
                            return Ok(components);
                        }
                        Some(PunctuationKind::LeftBracket) => bracket += 1,
                        Some(PunctuationKind::RightBracket) => bracket = bracket.saturating_sub(1),
                        Some(PunctuationKind::LeftBrace) => brace += 1,
                        Some(PunctuationKind::RightBrace) => brace = brace.saturating_sub(1),
                        Some(PunctuationKind::Comma)
                            if paren == 0 && bracket == 0 && brace == 0 =>
                        {
                            components.push(input.restarted(&tokens[component_start..index]));
                            component_start = index + 1;
                        }
                        _ => {}
                    }
                }
                Err(input.error_here("expected `)` to close tuple transition pattern"))
            }

            fn retain_value_pattern(
                syntax_trees: &mut SyntaxTrees,
                subject: ExpressionHandle,
                value: ExpressionHandle,
                bindings: &mut Vec<DestructureBindings>,
            ) {
                let capture = exprs(syntax_trees).insert_transition_subject(subject);
                let (head, guard) = if matches!(
                    exprs_of(syntax_trees).expression(value),
                    ExpressionNode::Name { .. }
                ) {
                    (value, ExpressionHandle::invalid())
                } else if arm_pattern_is_condition(syntax_trees, value) {
                    (ExpressionHandle::invalid(), value)
                } else {
                    let equality =
                        exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: subject,
                            operator: BinaryOperator::Equal,
                            right: value,
                        }));
                    (ExpressionHandle::invalid(), equality)
                };
                bindings.push(DestructureBindings {
                    subject,
                    fields: Vec::new(),
                    pattern: Some(syntax_trees::statement::TransitionPattern {
                        subject,
                        capture,
                        is_structural: false,
                        pattern: syntax_trees::expression::TableDestructurePattern {
                            head,
                            guard,
                            has_rest: true,
                            ..Default::default()
                        },
                    }),
                    proof_selectors: Vec::new(),
                });
            }

            fn join_guard_conjunction(
                syntax_trees: &mut SyntaxTrees,
                left: ExpressionHandle,
                right: ExpressionHandle,
            ) -> ExpressionHandle {
                if !left.is_valid() {
                    return right;
                }
                exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    left,
                    operator: BinaryOperator::And,
                    right,
                }))
            }

            pub(crate) fn parse_transition_expression_list<'tokens, 'source>(
                syntax_trees: &mut SyntaxTrees,
                input: Input<'tokens, 'source>,
            ) -> ParseResult<'tokens, 'source, Vec<ExpressionHandle>> {
                parse_transition_match_list(syntax_trees, input, false)
            }

            fn parse_transition_match_list<'tokens, 'source, T>(
                syntax_trees: &mut SyntaxTrees,
                input: Input<'tokens, 'source>,
                allow_wildcard: bool,
            ) -> ParseResult<'tokens, 'source, Vec<T>>
            where
                T: FromTransitionMatchComponent,
            {
                if input.at_punctuation(PunctuationKind::LeftParen)
                    && (allow_wildcard || input.leading_paren_group_has_top_level_comma())
                {
                    let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
                    let (values, input) = parse_transition_match_list_after_open_paren(
                        syntax_trees,
                        input,
                        allow_wildcard,
                    )?;
                    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
                    Ok((values, input))
                } else {
                    let (value, input) =
                        parse_transition_match_component(syntax_trees, input, allow_wildcard)?;
                    Ok((vec![value], input))
                }
            }

            fn parse_transition_match_list_after_open_paren<'tokens, 'source, T>(
                syntax_trees: &mut SyntaxTrees,
                mut input: Input<'tokens, 'source>,
                allow_wildcard: bool,
            ) -> ParseResult<'tokens, 'source, Vec<T>>
            where
                T: FromTransitionMatchComponent,
            {
                let mut values = Vec::new();
                while !input.at_punctuation(PunctuationKind::RightParen) {
                    let (value, rest) =
                        parse_transition_match_component(syntax_trees, input, allow_wildcard)?;
                    values.push(value);
                    input = rest;
                    if input.at_punctuation(PunctuationKind::Comma) {
                        input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    } else {
                        break;
                    }
                }
                Ok((values, input))
            }

            fn parse_transition_match_component<'tokens, 'source, T>(
                syntax_trees: &mut SyntaxTrees,
                input: Input<'tokens, 'source>,
                allow_wildcard: bool,
            ) -> ParseResult<'tokens, 'source, T>
            where
                T: FromTransitionMatchComponent,
            {
                if allow_wildcard && input.at_contextual("_") {
                    let input = input.take_contextual("_")?;
                    return Ok((T::from_wildcard(), input));
                }

                let (expression, input) =
                    parse_expression_handle_without_struct_literals(syntax_trees, input)?;
                Ok((T::from_expression(expression), input))
            }

            fn parse_destructure_pattern_arm<'tokens, 'source>(
                syntax_trees: &mut SyntaxTrees,
                input: Input<'tokens, 'source>,
                subject: ExpressionHandle,
            ) -> Result<Option<(TransitionGuardNode, DestructureBindings)>, ParseError>
            {
                if !input.at_name_like() {
                    return Ok(None);
                }

                let (pattern_input, guard_input) =
                    match find_top_level_keyword(input, KeywordKind::If) {
                        Some(if_index) => {
                            let (pattern_tokens, guard_tokens_with_if) =
                                input.tokens.split_at(if_index);
                            (
                                input.restarted(pattern_tokens),
                                Some(
                                    input.restarted(
                                        guard_tokens_with_if
                                            .get(1..)
                                            .expect("if keyword split should include guard tokens"),
                                    ),
                                ),
                            )
                        }
                        None => (input, None),
                    };

                let (path, after_path) = parse_path_handle_span(pattern_input, |member| {
                    exprs(syntax_trees).append_identifier_path_member(member)
                })?;
                if !after_path.at_punctuation(PunctuationKind::LeftBrace) {
                    if guard_input.is_some() {
                        return Err(after_path.error_here(
                            "expected `{` to open a destructure pattern before `if` guard",
                        ));
                    }
                    return Ok(None);
                }

                let (parsed_fields, pattern_rest) =
                    crate::grammar::patterns::parse_destructure_fields(syntax_trees, after_path)?;
                let fields = parsed_fields
                    .fields
                    .iter()
                    .map(|field| {
                        let binding = match &field.action {
                            MatchFieldAction::Bind(binding) => Some(binding.clone()),
                            _ => None,
                        };
                        (field.member.clone(), binding, field.component)
                    })
                    .collect::<Vec<_>>();
                let has_rest = parsed_fields.has_rest;
                let proof_selectors = parsed_fields.proof_selectors;
                let head = exprs(syntax_trees).insert(ExpressionNode::Name {
                    members: path,
                    application: None,
                });
                let capture = exprs(syntax_trees).insert_transition_subject(subject);
                let saved = exprs(syntax_trees).insert(ExpressionNode::SavedSubject(
                    syntax_trees::expression::TableSavedSubject {
                        component: Default::default(),
                        capture,
                        pattern_head: head,
                    },
                ));
                let fields = fields
                    .into_iter()
                    .filter_map(|(member, binding, component)| {
                        binding.map(|binding| DestructureBinding {
                            component,
                            binding,
                            member,
                            case_variant: None,
                        })
                    })
                    .collect::<Vec<_>>();
                if !pattern_rest.tokens.is_empty() {
                    return Err(pattern_rest.error_here("expected data destructure pattern"));
                }

                let mut combined = ExpressionHandle::invalid();
                if let Some(guard_input) = guard_input {
                    let (guard, rest) =
                        parse_expression_handle_without_struct_literals(syntax_trees, guard_input)?;
                    if !rest.tokens.is_empty() {
                        return Err(rest.error_here("expected transition pattern guard"));
                    }
                    let guard =
                        rewrite_destructure_guard_expression(syntax_trees, guard, saved, &fields);
                    combined = join_guard_conjunction(syntax_trees, combined, guard);
                }

                let guard = TransitionGuardNode::Always;
                Ok(Some((
                    guard,
                    DestructureBindings {
                        subject: saved,
                        fields,
                        pattern: Some(syntax_trees::statement::TransitionPattern {
                            subject,
                            capture,
                            is_structural: true,
                            pattern: syntax_trees::expression::TableDestructurePattern {
                                application_type: Default::default(),
                                head,
                                fields: exprs(syntax_trees)
                                    .insert_match_fields(parsed_fields.fields),
                                has_rest,
                                guard: combined,
                                proof_selectors: syntax_trees
                                    .statements
                                    .insert_outcome_proof_selectors(
                                        proof_selectors.iter().cloned(),
                                    ),
                            },
                        }),
                        proof_selectors,
                    },
                )))
            }

            fn find_top_level_keyword(input: Input<'_, '_>, keyword: KeywordKind) -> Option<usize> {
                let mut paren_depth = 0usize;
                let mut bracket_depth = 0usize;
                let mut brace_depth = 0usize;

                for (index, token) in input.tokens.iter().enumerate() {
                    match token.punctuation() {
                        Some(PunctuationKind::LeftParen) => paren_depth += 1,
                        Some(PunctuationKind::RightParen) => {
                            paren_depth = paren_depth.saturating_sub(1)
                        }
                        Some(PunctuationKind::LeftBracket) => bracket_depth += 1,
                        Some(PunctuationKind::RightBracket) => {
                            bracket_depth = bracket_depth.saturating_sub(1)
                        }
                        Some(PunctuationKind::LeftBrace) => brace_depth += 1,
                        Some(PunctuationKind::RightBrace) => {
                            brace_depth = brace_depth.saturating_sub(1)
                        }
                        _ => {}
                    }

                    if token.keyword() == Some(keyword)
                        && paren_depth == 0
                        && bracket_depth == 0
                        && brace_depth == 0
                    {
                        return Some(index);
                    }
                }

                None
            }

            trait FromTransitionMatchComponent {
                fn from_expression(expression: ExpressionHandle) -> Self;
                fn from_wildcard() -> Self;
            }

            impl FromTransitionMatchComponent for ExpressionHandle {
                fn from_expression(expression: ExpressionHandle) -> Self {
                    expression
                }

                fn from_wildcard() -> Self {
                    ExpressionHandle::invalid()
                }
            }

            impl FromTransitionMatchComponent for Option<ExpressionHandle> {
                fn from_expression(expression: ExpressionHandle) -> Self {
                    Some(expression)
                }

                fn from_wildcard() -> Self {
                    None
                }
            }
        }
        pub(crate) mod parse_transition {
            use crate::diagnostics::parse_error::ParseError;
            use crate::storage::tables::{items, stmts, stmts_of};
            use crate::tokens::token_cursor::{Input, ParseResult};
            use arena::HandleSpan;
            use syntax_trees::SyntaxTrees;
            use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
            use syntax_trees::identifier::Identifier;
            use syntax_trees::statement::{
                StatementHandle, StatementNode, TableLocalData, TableTransition,
                TransitionTargetHandle, TransitionTargetNode,
            };
            use tokens::PunctuationKind;

            use super::guards::{parse_transition_expression_list, parse_transition_guard_node};
            use super::targets::parse_target::parse_transition_block_target_with_bindings;

            pub(crate) fn parse_transition_block_handles<'tokens, 'source>(
                syntax_trees: &mut SyntaxTrees,
                input: Input<'tokens, 'source>,
            ) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
                /*
                    Captured subjects are named by where the transition starts
                    in its file and their place in its subject list. The
                    parser splits files into pages by worker count, and each
                    page numbers its own expressions, so a name taken from an
                    expression's arena index changed with the host's thread
                    count and so did every checked record holding it.
                */
                let transition_start = input.tokens.first().map_or(0, |token| token.span.start);
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

                let mut subject_captures: Vec<(Identifier, ExpressionHandle)> = Vec::new();
                for (place, subject_expression) in subject.iter_mut().enumerate() {
                    if expression_is_closed_boolean(syntax_trees, *subject_expression) {
                        continue;
                    }
                    let captured = *subject_expression;
                    let name = Identifier::generated(format!(
                        "__transition_subject#{transition_start}.{place}"
                    ));
                    let path_start = syntax_trees
                        .expressions
                        .append_identifier_path_member(name.clone());
                    let path = HandleSpan::from_parts(path_start, 1);
                    *subject_expression = syntax_trees.expressions.insert(ExpressionNode::Name {
                        members: path,
                        application: None,
                    });
                    subject_captures.push((name, captured));
                }

                let mut statements = crate::storage::handle_spans::HandleSpanBuilder::new();
                let mut arm_statements: Vec<StatementHandle> = Vec::new();
                let mut arm_bool_tuples: Vec<Option<Vec<Option<bool>>>> = Vec::new();
                let mut parsed_arms: Vec<(
                    syntax_trees::statement::TransitionGuardNode,
                    TransitionTargetHandle,
                    arena::HandleSpan<syntax_trees::statement::TableOutcomeProofSelector>,
                    source::SourceSpan,
                    HandleSpan<syntax_trees::statement::TransitionPattern>,
                )> = Vec::new();
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
                            stmts(syntax_trees)
                                .insert_transition_target(TransitionTargetNode::Terminal),
                            input,
                        )
                    } else {
                        parse_transition_block_target_with_bindings(syntax_trees, input, &bindings)?
                    };
                    input = rest;

                    let patterns = stmts(syntax_trees).insert_transition_patterns(
                        bindings.iter().filter_map(|binding| binding.pattern),
                    );
                    let proof_selectors = bindings
                        .first()
                        .map(|bindings| {
                            stmts(syntax_trees).insert_outcome_proof_selectors(
                                bindings.proof_selectors.iter().cloned(),
                            )
                        })
                        .unwrap_or_else(HandleSpan::empty);
                    parsed_arms.push((guard, target, proof_selectors, source_span, patterns));
                    arm_bool_tuples.push(bool_tuple);
                }

                if parsed_arms.is_empty() {
                    return Err(input.error_here(
            "a transition block must carry at least one arm; an empty dispatch can never proceed",
        ));
                }

                for (name, initial_value) in subject_captures {
                    let capture =
                        stmts(syntax_trees).insert(StatementNode::LocalData(TableLocalData {
                            role: language_core::LocalBindingRole::DispatchSubject,
                            name,
                            type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                            initial_value,
                            is_mutable: false,
                            relevance: language_core::BindingRelevance::Relevant,
                            pattern_fields: Default::default(),
                        }));
                    let handle = items(syntax_trees).append_statement_handle(capture);
                    statements.push(handle);
                }

                for (guard, target, proof_selectors, source_span, patterns) in parsed_arms {
                    let statement =
                        stmts(syntax_trees).insert(StatementNode::Transition(TableTransition {
                            patterns,
                            target,
                            continuation: TransitionTargetHandle::invalid(),
                            guard,
                            proof_selectors,
                            exit: Default::default(),
                            ends_dispatch: false,
                            source_span,
                        }));
                    let handle = items(syntax_trees).append_statement_handle(statement);
                    arm_statements.push(statement);
                    statements.push(handle);
                }

                let last = *arm_statements
                    .last()
                    .expect("empty dispatch rejected above");
                if let StatementNode::Transition(mut transition) =
                    stmts_of(syntax_trees).statement(last).clone()
                {
                    transition.ends_dispatch = true;
                    stmts(syntax_trees)
                        .replace_statement(last, StatementNode::Transition(transition));
                }

                let opposite_boolean_pair = subject.len() == 1
                    && matches!(arm_bool_tuples.as_slice(), [Some(first), Some(second)]
            if matches!((first.as_slice(), second.as_slice()),
                ([Some(left)], [Some(right)]) if left != right));

                if (subject.len() >= 2 || opposite_boolean_pair)
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
                            stmts_of(syntax_trees).statement(*last).clone()
                    {
                        stmts(syntax_trees).replace_statement(
                            *last,
                            StatementNode::Transition(TableTransition {
                                guard: syntax_trees::statement::TransitionGuardNode::Always,
                                patterns: HandleSpan::empty(),
                                ..transition
                            }),
                        );
                    }
                }

                let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
                Ok((statements.finish(), input))
            }

            fn expression_is_closed_boolean(
                syntax_trees: &SyntaxTrees,
                expression: ExpressionHandle,
            ) -> bool {
                match syntax_trees.expressions.expression(expression) {
                    ExpressionNode::Boolean(_) => true,
                    ExpressionNode::Binary(binary) => {
                        binary.operator == syntax_trees::expression::BinaryOperator::Equal
                            && matches!(
                                syntax_trees.expressions.expression(binary.left),
                                ExpressionNode::Boolean(_)
                            )
                            && matches!(
                                syntax_trees.expressions.expression(binary.right),
                                ExpressionNode::Boolean(_)
                            )
                    }
                    _ => false,
                }
            }

            fn parse_expression_list_until_punctuation<'tokens, 'source>(
                syntax_trees: &mut SyntaxTrees,
                input: Input<'tokens, 'source>,
                delimiter: PunctuationKind,
            ) -> Result<(Vec<ExpressionHandle>, Input<'tokens, 'source>), ParseError> {
                let (expression_input, rest) = input.split_at_top_level_punctuation(
                    delimiter,
                    "expected transition block delimiter",
                )?;
                let (expressions, rest_after_expression) =
                    parse_transition_expression_list(syntax_trees, expression_input)?;

                if !rest_after_expression.tokens.is_empty() {
                    return Err(
                        rest_after_expression.error_here("expected transition subject expression")
                    );
                }

                Ok((expressions, rest))
            }

            #[cfg(test)]
            mod tests {}
        }
        pub(crate) mod targets {
            pub(crate) mod parse_target {
                use crate::diagnostics::parse_error::ParseError;
                use crate::grammar::bodies::transitions::guards::{
                    DestructureBindings, rewrite_destructure_guard_expression,
                };
                use crate::grammar::bodies::{
                    append_identifier_path_member_to_statement_table,
                    copy_expression_handles_to_statement_table,
                    copy_expression_to_statement_identifier_path,
                };
                use crate::grammar::expressions::context::ExpressionContext;
                use crate::grammar::expressions::parse_expression::{
                    has_unary_expression_prefix, parse_expression_handle,
                };
                use crate::grammar::expressions::parse_postfix::parse_postfix_expression_handle;
                use crate::storage::tables::{exprs, exprs_of, stmts};
                use crate::tokens::token_cursor::{Input, ParseResult};
                use arena::HandleSpan;
                use syntax_trees::SyntaxTrees;
                use syntax_trees::expression::{
                    ExpressionHandle, ExpressionNode, TableCallExpression,
                };
                use syntax_trees::statement::{TransitionTargetHandle, TransitionTargetNode};
                use tokens::KeywordKind;

                pub(crate) fn parse_transition_block_target_handle<'tokens, 'source>(
                    syntax_trees: &mut SyntaxTrees,
                    input: Input<'tokens, 'source>,
                ) -> ParseResult<'tokens, 'source, TransitionTargetHandle> {
                    parse_transition_block_target_with_bindings(syntax_trees, input, &[])
                }

                pub(crate) fn parse_transition_block_target_with_bindings<'tokens, 'source>(
                    syntax_trees: &mut SyntaxTrees,
                    input: Input<'tokens, 'source>,
                    bindings: &[DestructureBindings],
                ) -> ParseResult<'tokens, 'source, TransitionTargetHandle> {
                    // An acknowledgement changes the call envelope, not
                    // whether a static path is a terminal machine target.
                    // Keep the original parse mode for prefixes and classify
                    // only the complete call afterwards.
                    let acknowledged =
                        input.at_contextual("suspend") || input.at_contextual("block");
                    let mut acknowledged_path = input;
                    if acknowledged_path.at_contextual("suspend") {
                        acknowledged_path = acknowledged_path.take_contextual("suspend")?;
                    }
                    if acknowledged_path.at_contextual("block") {
                        acknowledged_path = acknowledged_path.take_contextual("block")?;
                    }
                    let acknowledged_target = acknowledged && acknowledged_path.at_name_like();
                    let target_shaped = !has_unary_expression_prefix(input)?
                        && !input.at_keyword(KeywordKind::True)
                        && !input.at_keyword(KeywordKind::False)
                        && !input.at_keyword(KeywordKind::SelfValue)
                        && input.at_name_like();
                    let (expression, rest) = if target_shaped {
                        parse_postfix_expression_handle(
                            syntax_trees,
                            input,
                            ExpressionContext::Default,
                        )?
                    } else {
                        parse_expression_handle(syntax_trees, input)?
                    };

                    let expression = bindings.iter().fold(expression, |expression, bindings| {
                        if bindings.fields.is_empty() {
                            expression
                        } else {
                            rewrite_destructure_guard_expression(
                                syntax_trees,
                                expression,
                                bindings.subject,
                                &bindings.fields,
                            )
                        }
                    });

                    Ok((
                        classify_transition_target_handle(
                            syntax_trees,
                            expression,
                            target_shaped || acknowledged_target,
                        )?,
                        rest,
                    ))
                }

                fn classify_transition_target_handle(
                    syntax_trees: &mut SyntaxTrees,
                    expression: ExpressionHandle,
                    target_shaped: bool,
                ) -> Result<TransitionTargetHandle, ParseError> {
                    let node = exprs_of(syntax_trees).expression(expression).clone();
                    let target = match node {
                        ExpressionNode::Call(call)
                            if target_shaped
                                && call.target_is_static
                                && call.machine_arguments.is_empty() =>
                        {
                            classify_call_target_handle(syntax_trees, call)?
                        }
                        _ => TransitionTargetNode::Value(expression),
                    };

                    Ok(stmts(syntax_trees).insert_transition_target(target))
                }

                fn classify_call_target_handle(
                    syntax_trees: &mut SyntaxTrees,
                    call: TableCallExpression,
                ) -> Result<TransitionTargetNode, ParseError> {
                    let receiver_depth = if call.receiver.is_valid() {
                        expression_handle_identifier_depth(syntax_trees, call.receiver)
                    } else {
                        Some(0)
                    };
                    let Some(receiver_depth) = receiver_depth else {
                        return Ok(TransitionTargetNode::Value(
                            exprs(syntax_trees).insert(ExpressionNode::Call(call)),
                        ));
                    };
                    if receiver_depth > 1 {
                        return Ok(TransitionTargetNode::Value(
                            exprs(syntax_trees).insert(ExpressionNode::Call(call)),
                        ));
                    }
                    let path = if call.receiver.is_valid() {
                        let receiver = copy_expression_to_statement_identifier_path(
                            syntax_trees,
                            call.receiver,
                        )
                        .ok_or_else(|| {
                            ParseError::new("call target must be a path or member access")
                        })?;
                        append_identifier_path_member_to_statement_table(
                            syntax_trees,
                            receiver,
                            call.target.clone(),
                        )
                    } else {
                        let handle = syntax_trees
                            .statements
                            .append_identifier_path_member(call.target.clone());
                        HandleSpan::from_parts(handle, 1)
                    };

                    let arguments =
                        copy_expression_handles_to_statement_table(syntax_trees, call.arguments);
                    Ok(TransitionTargetNode::Named {
                        operational_acknowledgement: call.operational_acknowledgement,
                        path,
                        arguments,
                        evidence_arguments: call.evidence_arguments,
                        source_span: call.target.source_span(),
                    })
                }

                fn expression_handle_identifier_depth(
                    syntax_trees: &SyntaxTrees,
                    expression: ExpressionHandle,
                ) -> Option<usize> {
                    match exprs_of(syntax_trees).expression(expression) {
                        ExpressionNode::Name {
                            members: path,
                            application: path_application,
                        } if path_application.is_none() => Some(
                            exprs_of(syntax_trees)
                                .identifier_path_members(path.to_owned())
                                .len(),
                        ),
                        ExpressionNode::Member(member) => {
                            let depth =
                                expression_handle_identifier_depth(syntax_trees, member.receiver)?;
                            Some(depth + 1)
                        }
                        _ => None,
                    }
                }
            }
        }
    }

    use crate::storage::tables::stmts;
    pub(crate) fn copy_identifier_path_to_statement_table(
        syntax_trees: &mut syntax_trees::SyntaxTrees,
        path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
    ) -> arena::HandleSpan<syntax_trees::identifier::Identifier> {
        let mut copy = crate::storage::handle_spans::HandleSpanBuilder::new();

        let member_count = syntax_trees.expressions.identifier_path_members(path).len();

        for index in 0..member_count {
            let member = syntax_trees.expressions.identifier_path_members(path)[index].clone();
            copy.push(stmts(syntax_trees).append_identifier_path_member(member));
        }

        copy.finish()
    }

    pub(crate) fn append_identifier_path_member_to_statement_table(
        syntax_trees: &mut syntax_trees::SyntaxTrees,
        path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
        member: syntax_trees::identifier::Identifier,
    ) -> arena::HandleSpan<syntax_trees::identifier::Identifier> {
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(member);

        if path.is_empty() {
            arena::HandleSpan::from_parts(handle, 1)
        } else {
            arena::HandleSpan::from_parts(
                path.start(),
                path.count()
                    .checked_add(1)
                    .expect("handle span count overflow"),
            )
        }
    }

    pub(crate) fn copy_expression_handles_to_statement_table(
        syntax_trees: &mut syntax_trees::SyntaxTrees,
        arguments: arena::HandleSpan<syntax_trees::expression::ExpressionHandle>,
    ) -> arena::HandleSpan<syntax_trees::expression::ExpressionHandle> {
        let mut copy = crate::storage::handle_spans::HandleSpanBuilder::new();

        let arguments = syntax_trees
            .tables
            .expressions
            .expression_handles(arguments)
            .to_vec();

        for argument in arguments {
            copy.push(
                syntax_trees
                    .tables
                    .statements
                    .append_expression_handle(argument),
            );
        }

        copy.finish()
    }

    pub(crate) fn copy_expression_to_statement_identifier_path(
        syntax_trees: &mut syntax_trees::SyntaxTrees,
        expression: syntax_trees::expression::ExpressionHandle,
    ) -> Option<arena::HandleSpan<syntax_trees::identifier::Identifier>> {
        use syntax_trees::expression::ExpressionNode;
        match syntax_trees.expressions.expression(expression).clone() {
            ExpressionNode::Name {
                members: path,
                application: path_application,
            } if path_application.is_none() => Some(copy_identifier_path_to_statement_table(
                syntax_trees,
                path.to_owned(),
            )),
            ExpressionNode::Member(member) => {
                let receiver =
                    copy_expression_to_statement_identifier_path(syntax_trees, member.receiver)?;
                Some(append_identifier_path_member_to_statement_table(
                    syntax_trees,
                    receiver,
                    member.member,
                ))
            }
            _ => None,
        }
    }
}

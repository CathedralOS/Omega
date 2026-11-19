//! `match` and `transition`, which share one grammar, and the patterns their arms use.

use crate::errors::parse_error_kind::ParseErrorKind;
use crate::grammar::contexts::expression_context::ExpressionContext;
use crate::parsing::closed_node::ClosedNode;
use crate::parsing::parse_failed::Parsed;
use crate::parsing::syntax_parser::SyntaxParser;
use crate::tree::node_kind::NodeKind;
use omega_lexer::Punctuation;

impl SyntaxParser<'_> {
    /*
        :DispatchKeyword
        `match` and `transition` are ordinary words, so we need a rule for when
        one of them begins a dispatch and when it is just a name. It begins a
        dispatch when the token after it is `{` (no subject) or could start a
        subject. Otherwise it is a name: `match = 3;`, `match.field`,
        `match + 1`, `(match)`.

        The cost is that a value named `match` cannot be called as `match(x)`,
        indexed as `match[0]` or subtracted from as `match - 1`, because `(`,
        `[` and `-` can start a subject; `(match)(x)` keeps it a name. The
        `machine` keyword test takes a word away the same way, which GRAMMAR.md
        lists under "When a word is a keyword".
    */
    pub(crate) fn at_dispatch_keyword(
        &self,
        keyword: &str,
    ) -> bool {
        if !self.at_keyword(keyword) {
            return false;
        }

        if self.nth_at(1, Punctuation::LeftBrace) {
            return true;
        }

        self.nth_can_start_expression(1) && !self.nth_at_keyword(1, "as") && !self.nth_at_keyword(1, "in")
    }

    /// `( 'match' | 'transition' ) subject? '{' ( arm % ',' )? '}'`, closed as `kind`.
    pub(crate) fn parse_dispatch(
        &mut self,
        kind: NodeKind,
    ) -> Parsed<ClosedNode> {
        let dispatch = self.open();

        self.bump();

        if !self.at(Punctuation::LeftBrace) {
            // No record literal at the top of a subject: the `{` after it opens the arms.
            self.parse_expression(ExpressionContext::BEFORE_BRACE)?;
        }

        self.expect(Punctuation::LeftBrace, "`{`")?;

        while !self.at(Punctuation::RightBrace) {
            self.parse_arm()?;

            if self.eat(Punctuation::Comma) {
                continue;
            }

            if self.at(Punctuation::RightBrace) {
                break;
            }

            // Arms are separated by commas, and a newline is not one (decision 2).
            let kind = if self.at_end() {
                ParseErrorKind::UnexpectedEnd
            } else {
                ParseErrorKind::MissingArmComma
            };

            return Err(self.error_of_kind(kind, "`,` or `}`"));
        }

        self.expect(Punctuation::RightBrace, "`}`")?;

        Ok(self.close(dispatch, kind))
    }

    /// `pattern '->' expression`
    fn parse_arm(&mut self) -> Parsed {
        let arm = self.open();

        self.parse_pattern()?;
        self.expect(Punctuation::Arrow, "`->`")?;
        self.parse_expression(ExpressionContext::FULL)?;
        self.close(arm, NodeKind::Arm);

        Ok(())
    }

    /*
        A pattern.

            pattern = '_' | '(' ( pattern % ',' )? ')'
                    | path '{' ( field_pattern % ',' )? '..'? '}'
                    | expression

        The last alternative is what makes this interesting. With no subject,
        an arm's pattern is a condition, so `(a + b) > c -> ...` is a legal arm
        and so is `(a, b) -> ...`. Both start with `(`. We tell them apart by
        finding the matching `)` and looking at the one token after it: a tuple
        pattern is followed by something that ends a pattern (`->`, `,`, `)`,
        `=`, `:`, `;`), and a parenthesized condition is followed by the rest of
        the condition. See :ParenthesizedPattern.

        A pattern that is a single bare word becomes a BindingPattern. Whether
        that word binds a new name or names a constant or a case is a question
        about what the word refers to, which the resolver answers.
    */
    pub(crate) fn parse_pattern(&mut self) -> Parsed {
        self.enter_nesting()?;
        self.parse_pattern_unguarded()?;
        self.leave_nesting();

        Ok(())
    }

    fn parse_pattern_unguarded(&mut self) -> Parsed {
        if self.at_keyword("_") {
            let wildcard_pattern = self.open();

            self.bump();
            self.close(wildcard_pattern, NodeKind::WildcardPattern);

            return Ok(());
        }

        if self.at(Punctuation::LeftParenthesis) && self.parenthesis_closes_a_pattern() {
            return self.parse_tuple_pattern();
        }

        // @Speed: the commonest pattern by far is one name, as in `let total = ...`. A word followed
        // by a token that ends a pattern is that, and needs none of the expression grammar. The
        // general path below reaches the same node for it. Experiment 4.
        if self.at_word() && self.nth_ends_pattern(1) {
            let binding_pattern = self.open();

            self.bump();
            self.close(binding_pattern, NodeKind::BindingPattern);

            return Ok(());
        }

        let expression = self.parse_expression(ExpressionContext::BEFORE_BRACE)?;
        let is_path = self.has_kind(expression, NodeKind::PathExpression);

        if is_path && self.at(Punctuation::LeftBrace) {
            return self.parse_record_pattern(expression);
        }

        if is_path && self.is_single_token(expression) {
            self.set_kind(expression, NodeKind::BindingPattern);

            return Ok(());
        }

        let value_pattern = self.open_before(expression);

        self.close(value_pattern, NodeKind::ValuePattern);

        Ok(())
    }

    /// :ParenthesizedPattern
    /// With the cursor on `(`: true if the token after its matching `)` is one that ends a pattern.
    ///
    /// The scan costs the length of the group. Nested groups are each scanned again from their own
    /// `(`, so the worst case is the length times the nesting depth, which :NestingLimit bounds.
    fn parenthesis_closes_a_pattern(&self) -> bool {
        let mut bracket_depth = 0usize;
        let mut ahead = 0;

        loop {
            match self.nth_punctuation(ahead) {
                Some(Punctuation::LeftParenthesis | Punctuation::LeftBracket | Punctuation::LeftBrace) => bracket_depth += 1,
                Some(Punctuation::RightParenthesis | Punctuation::RightBracket | Punctuation::RightBrace) => {
                    bracket_depth = bracket_depth.saturating_sub(1);

                    if bracket_depth == 0 {
                        break;
                    }
                }
                Some(_) => {}
                None if !self.nth_exists(ahead) => return false,
                None => {}
            }

            ahead += 1;
        }

        self.nth_ends_pattern(ahead + 1)
    }

    /// True if the token `ahead` places on is one that can only come after a complete pattern.
    fn nth_ends_pattern(
        &self,
        ahead: usize,
    ) -> bool {
        matches!(
            self.nth_punctuation(ahead),
            Some(Punctuation::Arrow | Punctuation::Comma | Punctuation::RightParenthesis | Punctuation::Equal | Punctuation::Colon | Punctuation::Semicolon)
        )
    }

    /// `'(' ( pattern % ',' )? ')'`
    ///
    /// As with expressions, one pattern in parentheses with no comma is that pattern, grouped;
    /// `()`, `(a,)` and `(a, b)` are tuples.
    fn parse_tuple_pattern(&mut self) -> Parsed {
        let tuple_pattern = self.open();
        let mut pattern_count = 0usize;
        let mut has_comma = false;

        self.bump();

        while !self.at(Punctuation::RightParenthesis) {
            self.parse_pattern()?;
            pattern_count += 1;

            if !self.eat(Punctuation::Comma) {
                break;
            }

            has_comma = true;
        }

        self.expect(Punctuation::RightParenthesis, "`,` or `)`")?;

        let kind = if pattern_count == 1 && !has_comma {
            NodeKind::ParenPattern
        } else {
            NodeKind::TuplePattern
        };

        self.close(tuple_pattern, kind);

        Ok(())
    }

    /// `path '{' ( field_pattern % ',' )? '..'? '}'`, with `path` already parsed.
    ///
    /// `field` binds the field, `field as name` renames it, `field as _` waives it, and
    /// `field: value` is projected equality against an ordinary expression.
    fn parse_record_pattern(
        &mut self,
        path_expression: ClosedNode,
    ) -> Parsed {
        let record_pattern = self.open_before(path_expression);

        self.bump();

        while !self.at(Punctuation::RightBrace) {
            if self.at(Punctuation::DotDot) {
                let rest_pattern = self.open();

                self.bump();
                self.close(rest_pattern, NodeKind::RestPattern);

                // `..` stands for every field not written, so it comes last.
                self.expect(Punctuation::RightBrace, "`}` after `..`")?;
                self.close(record_pattern, NodeKind::RecordPattern);

                return Ok(());
            }

            let field_pattern = self.open();

            self.expect_word("a field name, `..` or `}`")?;

            if self.eat_keyword("as") {
                self.expect_word("a name or `_`")?;
            } else if self.eat(Punctuation::Colon) {
                self.parse_expression(ExpressionContext::FULL)?;
            }

            self.close(field_pattern, NodeKind::FieldPattern);

            if !self.eat(Punctuation::Comma) {
                break;
            }
        }

        self.expect(Punctuation::RightBrace, "`,` or `}`")?;
        self.close(record_pattern, NodeKind::RecordPattern);

        Ok(())
    }
}

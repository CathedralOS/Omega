//! Expressions: binary operators by precedence, casts, prefixes, postfixes and primaries.

use crate::expression_context::ExpressionContext;
use crate::grammar::types::TypePosition;
use crate::node_kind::NodeKind;
use crate::parser::{ClosedNode, Parsed, Parser};
use omega_lexer::Punctuation;

/// A binary operator at the cursor: how tightly it binds, and how many tokens spell it.
struct BinaryOperator {
    binding_level: u8,
    token_count: usize,
}

impl Parser<'_> {
    pub(crate) fn parse_expression(
        &mut self,
        context: ExpressionContext,
    ) -> Parsed<ClosedNode> {
        self.parse_binary(context, 1)
    }

    /*
        Binary operators, by precedence climbing.

        We parse one operand, then look at the operator after it. If that
        operator binds at least as tightly as `minimum_level`, it belongs to us:
        we wrap what we have as its left side and parse its right side, asking
        for operators that bind strictly tighter (`binding_level + 1`), which is
        what makes every operator group to the left. Otherwise the operator
        belongs to a caller further up, and we return.

        The levels are V1's table (decision 6), loosest first:

            1  ||
            2  &&
            3  == !=
            4  < <= > >=
            5  in
            6  |
            7  ^
            8  &
            9  << >>           and their policy forms
            10 + -             and their policy forms
            11 * / %           and their policy forms

        Note that bitwise operators bind tighter than comparisons, unlike C.
    */
    fn parse_binary(
        &mut self,
        context: ExpressionContext,
        minimum_level: u8,
    ) -> Parsed<ClosedNode> {
        let mut left_side = self.parse_cast(context)?;

        while let Some(binary_operator) = self.peek_binary_operator(context) {
            if binary_operator.binding_level < minimum_level {
                break;
            }

            let binary = self.open_before(left_side);

            for _ in 0..binary_operator.token_count {
                self.bump();
            }

            self.parse_binary(context, binary_operator.binding_level + 1)?;
            left_side = self.close(binary, NodeKind::Binary);
        }

        Ok(left_side)
    }

    fn peek_binary_operator(
        &self,
        context: ExpressionContext,
    ) -> Option<BinaryOperator> {
        // @Speed: punctuation is looked for first. The only operator that is a word is `in`, and
        // what follows an operand is nearly always punctuation, so `in` is only compared when
        // the token is not punctuation at all. Experiment 3.
        let Some(punctuation) = self.nth_punctuation(0) else {
            let is_membership = context.get_allows_membership() && self.at_keyword("in");

            return is_membership.then_some(BinaryOperator {
                binding_level: 5,
                token_count: 1,
            });
        };

        let binding_level = match punctuation {
            Punctuation::PipePipe => 1,
            Punctuation::AmpersandAmpersand => 2,
            Punctuation::EqualEqual | Punctuation::ExclamationEqual => 3,
            Punctuation::Less | Punctuation::LessEqual | Punctuation::GreaterEqual => 4,
            Punctuation::Greater => {
                if context.get_stops_at_greater() {
                    return None;
                }

                if let Some(token_count) = self.get_right_shift_token_count() {
                    return Some(BinaryOperator { binding_level: 9, token_count });
                }

                4
            }
            Punctuation::Pipe => 6,
            Punctuation::Caret => 7,
            Punctuation::Ampersand => 8,
            Punctuation::LessLess | Punctuation::LessLessWrapping | Punctuation::LessLessSaturating | Punctuation::LessLessTrapping => 9,
            Punctuation::Plus
            | Punctuation::PlusWrapping
            | Punctuation::PlusSaturating
            | Punctuation::PlusTrapping
            | Punctuation::Minus
            | Punctuation::MinusWrapping
            | Punctuation::MinusSaturating
            | Punctuation::MinusTrapping => 10,
            Punctuation::Asterisk
            | Punctuation::AsteriskWrapping
            | Punctuation::AsteriskSaturating
            | Punctuation::AsteriskTrapping
            | Punctuation::Slash
            | Punctuation::SlashWrapping
            | Punctuation::SlashSaturating
            | Punctuation::SlashTrapping
            | Punctuation::Percent
            | Punctuation::PercentWrapping
            | Punctuation::PercentSaturating
            | Punctuation::PercentTrapping => 11,
            _ => return None,
        };

        Some(BinaryOperator { binding_level, token_count: 1 })
    }

    /// With the cursor on `>`: the number of tokens in a right shift that starts here, if one does.
    ///
    /// The lexer has no `>>` token, so that `Vec<Vec<u8>>` can close one `>` at a time. A right
    /// shift is two `>` with no byte between them, and its policy form adds a `%`, `|` or `!` that
    /// touches the second.
    fn get_right_shift_token_count(&self) -> Option<usize> {
        if !(self.nth_at(1, Punctuation::Greater) && self.nth_touches_next(0)) {
            return None;
        }

        let has_policy_suffix = matches!(
            self.nth_punctuation(2),
            Some(Punctuation::Percent | Punctuation::Pipe | Punctuation::Exclamation)
        ) && self.nth_touches_next(1);

        Some(if has_policy_suffix { 3 } else { 2 })
    }

    /// `prefix ( 'as' type? cast_entry* )*`
    ///
    /// After `as`, an optional carrier type and then every `=D`, `+D` or `-D` belong to the cast,
    /// so a `+` or `-` directly after a cast begins an entry and is never arithmetic.
    fn parse_cast(
        &mut self,
        context: ExpressionContext,
    ) -> Parsed<ClosedNode> {
        let mut operand = self.parse_prefix(context)?;

        while self.at_keyword("as") {
            let cast = self.open_before(operand);

            self.bump();

            if !self.at_cast_entry() {
                self.parse_type(TypePosition::Cast)?;
            }

            while self.at_cast_entry() {
                let cast_entry = self.open();

                self.bump();
                self.parse_path_type()?;
                self.close(cast_entry, NodeKind::CastEntry);
            }

            operand = self.close(cast, NodeKind::Cast);
        }

        Ok(operand)
    }

    fn at_cast_entry(&self) -> bool {
        matches!(self.nth_punctuation(0), Some(Punctuation::Plus | Punctuation::Minus | Punctuation::Equal))
    }

    /// `( '&' 'mut'? | '!' | '~' | '-' | 'move' | 'suspend' | 'block' )* postfix`
    fn parse_prefix(
        &mut self,
        context: ExpressionContext,
    ) -> Parsed<ClosedNode> {
        self.enter_nesting()?;

        let parsed_prefix = self.parse_prefix_unguarded(context)?;

        self.leave_nesting();

        Ok(parsed_prefix)
    }

    fn parse_prefix_unguarded(
        &mut self,
        context: ExpressionContext,
    ) -> Parsed<ClosedNode> {
        match self.nth_punctuation(0) {
            Some(Punctuation::Ampersand) => {
                let borrow = self.open();

                self.bump();

                // `mut` is the marker only if an operand follows it; otherwise it is the name being borrowed.
                if self.at_keyword("mut") && self.nth_can_start_expression(1) {
                    self.bump();
                }

                self.parse_prefix(context)?;

                return Ok(self.close(borrow, NodeKind::Borrow));
            }
            Some(Punctuation::Exclamation | Punctuation::Tilde | Punctuation::Minus) => {
                let unary = self.open();

                self.bump();
                self.parse_prefix(context)?;

                return Ok(self.close(unary, NodeKind::Unary));
            }
            _ => {}
        }

        // :PrefixWords
        // `move`, `suspend` and `block` are prefixes only directly before another word, so each is
        // still a name everywhere else: `block(x)`, `block.field`, `block + 1`. The word after must
        // not be `as` or `in`, which would make this one the operand of a cast or a membership test.
        //
        // @Speed: the word after is tested first. After most names comes punctuation, so this one
        // token read spares nearly every operand three spelling comparisons. Experiment 3.
        if self.nth_at_word(1) {
            let prefix_word_kind = if self.at_keyword("move") {
                Some(NodeKind::Move)
            } else if self.at_keyword("suspend") || self.at_keyword("block") {
                Some(NodeKind::CallMarker)
            } else {
                None
            };

            if let Some(kind) = prefix_word_kind
                && !self.nth_at_keyword(1, "as")
                && !self.nth_at_keyword(1, "in")
            {
                let prefixed = self.open();

                self.bump();
                self.parse_prefix(context)?;

                return Ok(self.close(prefixed, kind));
            }
        }

        self.parse_postfix(context)
    }

    /// True if the token `ahead` places on could be the first token of an expression.
    pub(crate) fn nth_can_start_expression(
        &self,
        ahead: usize,
    ) -> bool {
        if self.nth_at_word(ahead) || self.nth_at_literal(ahead) {
            return true;
        }

        matches!(
            self.nth_punctuation(ahead),
            Some(
                Punctuation::LeftParenthesis
                    | Punctuation::LeftBracket
                    | Punctuation::Ampersand
                    | Punctuation::Exclamation
                    | Punctuation::Tilde
                    | Punctuation::Minus
            )
        )
    }

    /// `primary ( '(' args ')' | '.' word expr_args? | '[' index ']' )*`
    fn parse_postfix(
        &mut self,
        context: ExpressionContext,
    ) -> Parsed<ClosedNode> {
        let mut operand = self.parse_primary(context)?;

        loop {
            if self.at(Punctuation::LeftParenthesis) {
                let call = self.open_before(operand);

                self.parse_args()?;
                operand = self.close(call, NodeKind::Call);
            } else if self.at(Punctuation::Dot) {
                let field_access = self.open_before(operand);

                self.bump();
                self.expect_word("a field name")?;

                if self.at(Punctuation::ColonColon) && self.nth_at(1, Punctuation::Less) {
                    self.parse_expression_args()?;
                }

                operand = self.close(field_access, NodeKind::FieldAccess);
            } else if self.at(Punctuation::LeftBracket) {
                let index = self.open_before(operand);

                self.parse_index_brackets()?;
                operand = self.close(index, NodeKind::Index);
            } else {
                break;
            }
        }

        Ok(operand)
    }

    /// `'(' ( expression % ',' )? ')'`
    fn parse_args(&mut self) -> Parsed {
        let args = self.open();

        self.bump();

        while !self.at(Punctuation::RightParenthesis) {
            self.parse_expression(ExpressionContext::FULL)?;

            if !self.eat(Punctuation::Comma) {
                break;
            }
        }

        self.expect(Punctuation::RightParenthesis, "`,` or `)`")?;
        self.close(args, NodeKind::Args);

        Ok(())
    }

    /// `'[' ( expression | expression? ( '..' | '..=' ) expression? ) ']'`
    fn parse_index_brackets(&mut self) -> Parsed {
        self.bump();

        if self.at_range_operator() {
            let range = self.open();

            self.parse_range_tail()?;
            self.close(range, NodeKind::Range);
        } else {
            let first_bound = self.parse_expression(ExpressionContext::FULL)?;

            if self.at_range_operator() {
                let range = self.open_before(first_bound);

                self.parse_range_tail()?;
                self.close(range, NodeKind::Range);
            }
        }

        self.expect(Punctuation::RightBracket, "`]`")
    }

    fn at_range_operator(&self) -> bool {
        self.at(Punctuation::DotDot) || self.at(Punctuation::DotDotEqual)
    }

    /// The range operator and the bound after it, if there is one. The cursor is on the operator.
    fn parse_range_tail(&mut self) -> Parsed {
        self.bump();

        if !self.at(Punctuation::RightBracket) {
            self.parse_expression(ExpressionContext::FULL)?;
        }

        Ok(())
    }

    fn parse_primary(
        &mut self,
        context: ExpressionContext,
    ) -> Parsed<ClosedNode> {
        if self.at_literal() {
            let literal = self.open();

            self.bump();

            return Ok(self.close(literal, NodeKind::Literal));
        }

        if self.at(Punctuation::LeftParenthesis) {
            return self.parse_parenthesized();
        }

        if self.at(Punctuation::LeftBracket) {
            return self.parse_array_literal();
        }

        if self.at_dispatch_keyword("match") {
            return self.parse_dispatch(NodeKind::Match);
        }

        if self.at_word() {
            let path_expression = self.parse_path_expression()?;

            if context.get_allows_record_literal() && self.at(Punctuation::LeftBrace) {
                return self.parse_record_literal(path_expression);
            }

            return Ok(path_expression);
        }

        Err(self.error("an expression"))
    }

    /// `word expr_args? ( '::' word expr_args? )*`
    ///
    /// Generic arguments in an expression are written `::<...>`. A bare `<` after a name is always
    /// less-than (decision 7), so nothing here needs to guess.
    pub(crate) fn parse_path_expression(&mut self) -> Parsed<ClosedNode> {
        let path_expression = self.open();

        self.expect_word("a name")?;

        loop {
            if self.at(Punctuation::ColonColon) && self.nth_at(1, Punctuation::Less) {
                self.parse_expression_args()?;
            }

            if !(self.at(Punctuation::ColonColon) && self.nth_at_word(1)) {
                break;
            }

            self.bump();
            self.bump();
        }

        if self.at(Punctuation::ColonColon) {
            self.bump();

            return Err(self.error("a name or `<` after `::`"));
        }

        Ok(self.close(path_expression, NodeKind::PathExpression))
    }

    /// `path '{' ( field_init % ',' )? '..'? '}'`, with `path` already parsed.
    fn parse_record_literal(
        &mut self,
        path_expression: ClosedNode,
    ) -> Parsed<ClosedNode> {
        let record_literal = self.open_before(path_expression);

        self.bump();

        while !self.at(Punctuation::RightBrace) {
            if self.at(Punctuation::DotDot) {
                let omission = self.open();

                self.bump();
                self.close(omission, NodeKind::Omission);

                // `..` stands for every field not written, so it comes last.
                self.expect(Punctuation::RightBrace, "`}` after `..`")?;

                return Ok(self.close(record_literal, NodeKind::RecordLiteral));
            }

            let field_init = self.open();

            self.expect_word("a field name, `..` or `}`")?;

            if self.eat(Punctuation::Colon) {
                self.parse_expression(ExpressionContext::FULL)?;
            }

            self.close(field_init, NodeKind::FieldInit);

            if !self.eat(Punctuation::Comma) {
                break;
            }
        }

        self.expect(Punctuation::RightBrace, "`,` or `}`")?;

        Ok(self.close(record_literal, NodeKind::RecordLiteral))
    }

    /// `'(' ')'`, `'(' expression ')'`, `'(' expression ',' ... ')'` or `'(' path 'satisfies' type ')'`
    fn parse_parenthesized(&mut self) -> Parsed<ClosedNode> {
        let parenthesized = self.open();

        self.bump();

        if self.eat(Punctuation::RightParenthesis) {
            return Ok(self.close(parenthesized, NodeKind::Tuple));
        }

        if self.at_selection() {
            self.parse_path()?;
            self.bump();
            self.parse_type(TypePosition::Plain)?;
            self.expect(Punctuation::RightParenthesis, "`)`")?;

            return Ok(self.close(parenthesized, NodeKind::Selection));
        }

        self.parse_expression(ExpressionContext::FULL)?;

        let mut kind = NodeKind::Paren;

        while self.eat(Punctuation::Comma) {
            kind = NodeKind::Tuple;

            if self.at(Punctuation::RightParenthesis) {
                break;
            }

            self.parse_expression(ExpressionContext::FULL)?;
        }

        self.expect(Punctuation::RightParenthesis, "`,` or `)`")?;

        Ok(self.close(parenthesized, kind))
    }

    /// With the cursor just inside `(`: true if what follows is `path 'satisfies'`.
    fn at_selection(&self) -> bool {
        if !self.at_word() {
            return false;
        }

        let mut ahead = 1;

        while self.nth_at(ahead, Punctuation::ColonColon) && self.nth_at_word(ahead + 1) {
            ahead += 2;
        }

        self.nth_at_keyword(ahead, "satisfies")
    }

    /// `'[' ( expression % ',' )? ']'`
    fn parse_array_literal(&mut self) -> Parsed<ClosedNode> {
        let array_literal = self.open();

        self.bump();

        while !self.at(Punctuation::RightBracket) {
            self.parse_expression(ExpressionContext::FULL)?;

            if !self.eat(Punctuation::Comma) {
                break;
            }
        }

        self.expect(Punctuation::RightBracket, "`,` or `]`")?;

        Ok(self.close(array_literal, NodeKind::ArrayLiteral))
    }
}

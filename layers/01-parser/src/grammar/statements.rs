//! Bodies and the statements inside them.

use crate::expression_context::ExpressionContext;
use crate::grammar::types::TypePosition;
use crate::node_kind::NodeKind;
use crate::parse_error::ParseError;
use crate::parse_error_kind::ParseErrorKind;
use crate::parser::{Parsed, Parser};
use omega_lexer::Punctuation;

/// How a statement ended, which decides what may come after it in a body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StatementShape {
    /// Ended with `;`, or is a form that needs none. More statements may follow.
    Terminated,

    /// An expression with no `;`: the body's result. Only nested machines may follow.
    Tail,

    /// A `transition`. Only nested machines may follow.
    Transition,
}

/// Where an expression with no `;` after it is accepted as a result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TailRule {
    /// Only as the last executable thing in a body. This is the language's rule.
    LastInBody,

    /// Anywhere. Only fragments use this: the spec lists bare expressions one per line to show
    /// forms, and each ends where the next token cannot continue it.
    Anywhere,
}

impl Parser<'_> {
    /*
        `'{' statement* tail? machine* '}'`

        A body is its executable part followed by its nested machines, in that
        order: "Once declarations begin, executable statements cannot resume."
        A `transition` is the last executable thing in a body, and so is an
        expression with no `;` after it. We give each of those mistakes its own
        error, because "expected `}`" tells the writer nothing.
    */
    pub(crate) fn parse_body(&mut self) -> Parsed {
        self.enter_nesting()?;

        let body = self.open();

        self.expect(Punctuation::LeftBrace, "`{`")?;

        let mut last_shape = StatementShape::Terminated;

        while !self.at(Punctuation::RightBrace) && !self.at_machine_declaration() {
            if self.at_end() {
                return Err(self.error("`}`"));
            }

            if last_shape == StatementShape::Transition {
                return Err(self.error_of_kind(ParseErrorKind::TransitionNotLast, "`}`"));
            }

            last_shape = self.parse_statement(TailRule::LastInBody)?;
        }

        while !self.at(Punctuation::RightBrace) {
            if self.at_end() {
                return Err(self.error("`}`"));
            }

            if !self.at_machine_declaration() {
                return Err(self.error_of_kind(ParseErrorKind::StatementAfterMachine, "a machine or `}`"));
            }

            self.parse_item()?;
        }

        self.bump();
        self.close(body, NodeKind::Body);
        self.leave_nesting();

        Ok(())
    }

    /// One statement. There is no `if`, `while`, `loop`, `for` or `return` (decision 1).
    pub(crate) fn parse_statement(
        &mut self,
        tail_rule: TailRule,
    ) -> Parsed<StatementShape> {
        // `let` begins a statement when a pattern follows it; `let = 3;` assigns to a name.
        if self.at_keyword("let") && (self.nth_at_word(1) || self.nth_at(1, Punctuation::LeftParenthesis)) {
            self.parse_let()?;

            return Ok(StatementShape::Terminated);
        }

        if self.at_dispatch_keyword("transition") {
            self.parse_dispatch(NodeKind::Transition)?;

            return Ok(StatementShape::Transition);
        }

        if self.at_asm_block() {
            self.parse_asm_block()?;

            return Ok(StatementShape::Terminated);
        }

        if self.at_keyword("_") && self.nth_at(1, Punctuation::Equal) {
            let discard = self.open();

            self.bump();
            self.bump();
            self.parse_expression(ExpressionContext::FULL)?;
            self.expect(Punctuation::Semicolon, "`;`")?;
            self.close(discard, NodeKind::Discard);

            return Ok(StatementShape::Terminated);
        }

        /*
            :IfStatement
            `if` is not reserved, so `if(ready);` is a call to a machine named
            `if` and must parse. But someone who writes `if ready { ... }` has
            made a different mistake from "expected `;`", and deserves to be told
            Omega has no `if`. So we parse the statement as any other, and when
            a statement that began with the word `if` fails, we report that
            instead, at the `if`. In a fragment, where a bare `if` followed by
            more source would be accepted as a result, that counts as failing.
        */
        let if_span = self.at_keyword("if").then(|| self.get_span(0));
        let statement_result = self.parse_expression_statement(tail_rule);

        let Some(if_span) = if_span else {
            return statement_result;
        };

        let is_if_statement = match &statement_result {
            Err(error) => matches!(error.get_kind(), ParseErrorKind::UnexpectedToken | ParseErrorKind::UnexpectedEnd),
            Ok(StatementShape::Tail) => tail_rule == TailRule::Anywhere && !self.at_end(),
            Ok(_) => false,
        };

        if is_if_statement {
            return Err(ParseError::new(ParseErrorKind::IfStatement, "`transition`", if_span));
        }

        statement_result
    }

    /// `'let' 'mut'? pattern ( ':' type_in )? ( '=' expression )? ';'`
    fn parse_let(&mut self) -> Parsed {
        let let_statement = self.open();

        self.bump();

        // `mut` is the marker only if a pattern follows it; `let mut = 3;` binds a name spelled `mut`.
        if self.at_keyword("mut") && (self.nth_at_word(1) || self.nth_at(1, Punctuation::LeftParenthesis)) {
            self.bump();
        }

        self.parse_pattern()?;

        if self.eat(Punctuation::Colon) {
            self.parse_type_in(TypePosition::Plain)?;
        }

        if self.eat(Punctuation::Equal) {
            self.parse_expression(ExpressionContext::FULL)?;
        }

        self.expect(Punctuation::Semicolon, "`;`")?;
        self.close(let_statement, NodeKind::Let);

        Ok(())
    }

    /// `place '=' expression ';'`, `expression ';'`, or an expression that is the body's result.
    fn parse_expression_statement(
        &mut self,
        tail_rule: TailRule,
    ) -> Parsed<StatementShape> {
        // @Speed: the statement is opened before its expression, and given its kind once we see how
        // it ends. Wrapping the finished expression instead would shift all of its events to make
        // room for the opening one. Experiment 4.
        let statement = self.open();

        self.parse_expression(ExpressionContext::FULL)?;

        if self.at(Punctuation::Equal) {
            self.bump();
            self.parse_expression(ExpressionContext::FULL)?;
            self.expect(Punctuation::Semicolon, "`;`")?;
            self.close(statement, NodeKind::Assign);

            return Ok(StatementShape::Terminated);
        }

        if self.at(Punctuation::Semicolon) {
            self.bump();
            self.close(statement, NodeKind::ExpressionStatement);

            return Ok(StatementShape::Terminated);
        }

        let is_last_in_body = self.at(Punctuation::RightBrace) || self.at_end() || self.at_machine_declaration();

        if is_last_in_body || tail_rule == TailRule::Anywhere {
            self.close(statement, NodeKind::Tail);

            return Ok(StatementShape::Tail);
        }

        Err(self.error("`;`"))
    }
}

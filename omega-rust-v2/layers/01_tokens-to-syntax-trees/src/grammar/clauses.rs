//! Header clauses: `requires`, `ensures`, `reaches` and the rest.

use crate::NodeKind;
use crate::grammar::contexts::expression_context::ExpressionContext;
use crate::grammar::contexts::type_position::TypePosition;
use crate::parsing::parse_failed::Parsed;
use crate::parsing::syntax_parser::SyntaxParser;
use omega_tokens::Punctuation;

/// The suffix grammar and resulting syntax node for a header keyword.
enum Clause {
    Predicate(NodeKind),
    Type(NodeKind),
    Flag(NodeKind),
    Crashes,
    Terminates,
    Registers,
    Clobbers,
}

fn clause(word: &[u8]) -> Option<Clause> {
    Some(match word {
        b"requires" => Clause::Predicate(NodeKind::Requires),
        b"ensures" => Clause::Predicate(NodeKind::Ensures),
        b"reaches" => Clause::Type(NodeKind::Reaches),
        b"crashes" => Clause::Crashes,
        b"blocks" => Clause::Flag(NodeKind::Blocks),
        b"suspends" => Clause::Flag(NodeKind::Suspends),
        b"terminates" => Clause::Terminates,
        b"invokes" => Clause::Type(NodeKind::Invokes),
        b"satisfies" => Clause::Type(NodeKind::Satisfies),
        b"registers" => Clause::Registers,
        b"clobbers" => Clause::Clobbers,
        _ => return None,
    })
}

impl SyntaxParser<'_> {
    pub(crate) fn nth_at_clause_keyword(
        &self,
        ahead: usize,
    ) -> bool {
        self.nth_word(ahead).and_then(clause).is_some()
    }

    /// Can this word start the next header clause rather than name an operand?
    pub(super) fn nth_can_start_clause(
        &self,
        ahead: usize,
    ) -> bool {
        match self.nth_word(ahead).and_then(clause) {
            Some(Clause::Predicate(_)) => self.nth_can_start_expression(ahead + 1),
            Some(Clause::Type(_)) => self.nth_can_start_type(ahead + 1) || self.nth_at(ahead + 1, Punctuation::LeftParenthesis),
            Some(Clause::Crashes | Clause::Registers | Clause::Clobbers) => self.nth_at_word(ahead + 1),
            Some(Clause::Flag(_) | Clause::Terminates) => true,
            None => false,
        }
    }

    /*
        `clause*`

        Nothing separates one clause from the next or from the body. A clause's
        predicate is an expression, and an expression ends by itself at the
        first token that cannot continue it. A word after a complete expression
        cannot continue it, so when we get back here we are looking at a single
        token and ask whether it is a clause word. That is why `requires
        ensures_stupid()` and `requires a < reaches` both work: in the first the
        lexer hands us one word that is not `ensures`, and in the second
        `reaches` is consumed as the operand `<` was waiting for, long before
        this loop sees it.
    */
    pub(crate) fn parse_clauses(&mut self) -> Parsed {
        while let Some(clause) = self.nth_word(0).and_then(clause) {
            if matches!(clause, Clause::Predicate(NodeKind::Requires)) && self.nth_at_keyword(1, "machine") && self.nth_at_word(2) {
                self.parse_hypothesis()?;
            } else {
                self.parse_clause(clause)?;
            }
        }

        Ok(())
    }

    pub(super) fn parse_data_requirements(&mut self) -> Parsed {
        while self.at_keyword("requires") {
            self.parse_clause(Clause::Predicate(NodeKind::Requires))?;
        }
        Ok(())
    }

    /// One clause. The cursor is on its clause word.
    fn parse_clause(
        &mut self,
        header: Clause,
    ) -> Parsed {
        let clause = self.open();

        self.bump();

        let kind = match header {
            Clause::Predicate(kind) => {
                self.parse_expression(ExpressionContext::HEADER)?;
                kind
            }
            Clause::Crashes => {
                // The cause names `Trap`, `Abort` or `none`, so it is a path type and not a full
                // type: a `(` after it opens the guard, as in `crashes Trap (value == 0)`, instead
                // of making the cause a family application that swallows the guard.
                self.parse_path_type()?;
                let at_boundary = self.at_end()
                    || self.nth_at_clause_keyword(0)
                    || [
                        Punctuation::LeftBrace,
                        Punctuation::RightBrace,
                        Punctuation::Semicolon,
                        Punctuation::Comma,
                        Punctuation::RightParenthesis,
                        Punctuation::RightBracket,
                        Punctuation::Greater,
                        Punctuation::Equal,
                    ]
                    .into_iter()
                    .any(|punctuation| self.at(punctuation));
                if !at_boundary {
                    self.parse_expression(ExpressionContext::HEADER)?;
                }
                NodeKind::Crashes
            }
            Clause::Type(kind) => {
                // `none` is an ordinary name here; what it means is not this layer's question.
                self.parse_type(TypePosition::Plain)?;
                kind
            }
            Clause::Flag(kind) => {
                if self.at_keyword("true") || self.at_keyword("false") {
                    self.bump();
                }
                kind
            }
            Clause::Terminates => {
                // `terminates ( 'by' measure ( '->' order )? ( 'in' bound )? )?`
                if self.eat_keyword("by") {
                    self.parse_expression(ExpressionContext::MEASURE)?;

                    if self.eat(Punctuation::Arrow) {
                        self.parse_expression(ExpressionContext::MEASURE)?;
                    }

                    if self.eat_keyword("in") {
                        self.parse_bound(ExpressionContext::HEADER)?;
                    }
                }
                NodeKind::Terminates
            }
            Clause::Registers => {
                self.expect_word("a register name")?;
                self.expect(Punctuation::Equal, "`=`")?;
                self.parse_expression(ExpressionContext::HEADER)?;
                NodeKind::Registers
            }
            Clause::Clobbers => {
                self.expect_word("`none` or a register name")?;

                // Leave a comma immediately before the body to the header's separator diagnostic.
                while self.at(Punctuation::Comma) && !self.nth_at(1, Punctuation::LeftBrace) {
                    self.bump();
                    self.expect_word("a register name")?;
                }
                NodeKind::Clobbers
            }
        };

        self.close(clause, kind);

        Ok(())
    }

    /// `expression | expression ( '..' | '..=' ) expression`, the bound of `terminates by ... in`.
    fn parse_bound(
        &mut self,
        context: ExpressionContext,
    ) -> Parsed {
        let lower_bound = self.parse_expression(context)?;

        if self.at(Punctuation::DotDot) || self.at(Punctuation::DotDotEqual) {
            let range = self.open_before(lower_bound);

            self.bump();
            self.parse_expression(context)?;
            self.close(range, NodeKind::Range);
        }

        Ok(())
    }

    /// `'requires' 'machine' word '(' params ')' result? clause* ';'`
    ///
    /// A hypothesis assumes a machine with this signature exists. It is the one clause that ends
    /// in a semicolon: it carries clauses of its own, and without the `;` there would be no telling
    /// where they stop and the enclosing header's clauses resume.
    pub(crate) fn parse_hypothesis(&mut self) -> Parsed {
        self.enter_nesting()?;
        let hypothesis = self.open();

        self.bump();
        self.bump();
        self.expect_word("a name")?;
        self.parse_params()?;

        if self.at(Punctuation::Arrow) {
            self.parse_result()?;
        }

        self.parse_clauses()?;
        self.expect(Punctuation::Semicolon, "a clause or `;`")?;
        self.close(hypothesis, NodeKind::Hypothesis);
        self.leave_nesting();

        Ok(())
    }
}

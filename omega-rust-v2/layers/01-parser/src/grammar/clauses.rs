//! Header clauses: `requires`, `ensures`, `reaches` and the rest.

use crate::grammar::contexts::expression_context::ExpressionContext;
use crate::grammar::contexts::type_position::TypePosition;
use crate::parsing::parse_failed::Parsed;
use crate::parsing::syntax_parser::SyntaxParser;
use crate::tree::node_kind::NodeKind;
use omega_lexer::Punctuation;

/// The clause a word introduces, if it is a clause word.
fn get_clause_kind(word: &[u8]) -> Option<NodeKind> {
    Some(match word {
        b"requires" => NodeKind::Requires,
        b"ensures" => NodeKind::Ensures,
        b"reaches" => NodeKind::Reaches,
        b"crashes" => NodeKind::Crashes,
        b"blocks" => NodeKind::Blocks,
        b"suspends" => NodeKind::Suspends,
        b"terminates" => NodeKind::Terminates,
        b"invokes" => NodeKind::Invokes,
        b"satisfies" => NodeKind::Satisfies,
        b"registers" => NodeKind::Registers,
        b"clobbers" => NodeKind::Clobbers,
        _ => return None,
    })
}

impl SyntaxParser<'_> {
    pub(crate) fn nth_at_clause_keyword(
        &self,
        ahead: usize,
    ) -> bool {
        self.nth_word(ahead).and_then(get_clause_kind).is_some()
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

        `type_position` matters for one case. The clauses of a machine type
        written directly inside `<...>` end at the `>` that closes the list, so
        their predicates must not read that `>` as greater-than.
    */
    pub(crate) fn parse_clauses(
        &mut self,
        type_position: TypePosition,
    ) -> Parsed {
        while let Some(kind) = self.nth_word(0).and_then(get_clause_kind) {
            if kind == NodeKind::Requires && self.nth_at_keyword(1, "machine") && self.nth_at_word(2) {
                self.parse_hypothesis(type_position)?;
            } else {
                self.parse_clause(kind, type_position)?;
            }
        }

        Ok(())
    }

    /// One clause. The cursor is on its clause word.
    pub(crate) fn parse_clause(
        &mut self,
        kind: NodeKind,
        type_position: TypePosition,
    ) -> Parsed {
        let stops_at_greater = type_position == TypePosition::InsideAngles;
        let predicate_context = ExpressionContext::BEFORE_BRACE.with_stops_at_greater(stops_at_greater);
        let measure_context = ExpressionContext::MEASURE.with_stops_at_greater(stops_at_greater);
        let clause = self.open();

        self.bump();

        match kind {
            NodeKind::Requires | NodeKind::Ensures => {
                self.parse_expression(predicate_context)?;
            }
            NodeKind::Crashes => {
                self.parse_type(type_position)?;
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
                    self.parse_expression(predicate_context)?;
                }
            }
            NodeKind::Reaches | NodeKind::Invokes | NodeKind::Satisfies => {
                // `none` is an ordinary name here; what it means is not this layer's question.
                self.parse_type(type_position)?;
            }
            NodeKind::Blocks | NodeKind::Suspends => {
                if self.at_keyword("true") || self.at_keyword("false") {
                    self.bump();
                }
            }
            NodeKind::Terminates => {
                // `terminates ( 'by' measure ( '->' order )? ( 'in' bound )? )?`
                if self.eat_keyword("by") {
                    self.parse_expression(measure_context)?;

                    if self.eat(Punctuation::Arrow) {
                        self.parse_expression(measure_context)?;
                    }

                    if self.eat_keyword("in") {
                        self.parse_bound(predicate_context)?;
                    }
                }
            }
            NodeKind::Registers => {
                self.expect_word("a register name")?;
                self.expect(Punctuation::Equal, "`=`")?;
                self.parse_expression(predicate_context)?;
            }
            NodeKind::Clobbers => {
                self.expect_word("`none` or a register name")?;

                // A comma continues the list only if a name follows; a comma before `{` is caught by the caller.
                while self.at(Punctuation::Comma) && self.nth_at_word(1) {
                    self.bump();
                    self.bump();
                }
            }
            _ => {}
        }

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
    pub(crate) fn parse_hypothesis(
        &mut self,
        type_position: TypePosition,
    ) -> Parsed {
        let hypothesis = self.open();

        self.bump();
        self.bump();
        self.expect_word("a name")?;
        self.parse_params()?;

        if self.at(Punctuation::Arrow) {
            self.parse_result(type_position)?;
        }

        self.parse_clauses(type_position)?;
        self.expect(Punctuation::Semicolon, "a clause or `;`")?;
        self.close(hypothesis, NodeKind::Hypothesis);

        Ok(())
    }
}

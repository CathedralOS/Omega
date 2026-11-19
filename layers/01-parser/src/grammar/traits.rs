//! Traits and the conformances that satisfy them.

use crate::grammar::contexts::body_rule::BodyRule;
use crate::grammar::contexts::type_position::TypePosition;
use crate::parsing::parse_failed::Parsed;
use crate::parsing::syntax_parser::SyntaxParser;
use crate::tree::node_kind::NodeKind;
use omega_lexer::Punctuation;

impl SyntaxParser<'_> {
    /// `visibility? 'boundary'? 'trait' word generic_params? ( ':' type )?
    ///  ( '{' requirement* '}' | '=' type '{' restriction* '}' )`
    pub(crate) fn parse_trait(&mut self) -> Parsed {
        let trait_item = self.open();

        self.eat_visibility();
        self.eat_keyword("boundary");
        self.bump();
        self.expect_word("a name")?;

        if self.at(Punctuation::Less) {
            self.parse_generic_params()?;
        }

        if self.eat(Punctuation::Colon) {
            self.parse_type(TypePosition::Plain)?;
        }

        // A restricted alias names the trait it narrows: `trait LocalLogger = Logger { machine * reaches none; }`
        let is_restricted_alias = self.eat(Punctuation::Equal);

        if is_restricted_alias {
            self.parse_type(TypePosition::Plain)?;
        }

        self.expect(Punctuation::LeftBrace, "`{`")?;

        while !self.at(Punctuation::RightBrace) {
            if !self.at_keyword("machine") {
                return Err(self.error("`machine` or `}`"));
            }

            if is_restricted_alias {
                self.parse_restriction()?;
            } else {
                self.parse_machine(BodyRule::Refused)?;
            }
        }

        self.bump();
        self.close(trait_item, NodeKind::Trait);

        Ok(())
    }

    /// `'machine' '*' clause* ';'`, a restriction on every machine of the aliased trait.
    fn parse_restriction(&mut self) -> Parsed {
        let restriction = self.open();

        self.bump();
        self.expect(Punctuation::Asterisk, "`*`")?;
        self.parse_clauses(TypePosition::Plain)?;
        self.expect(Punctuation::Semicolon, "a clause or `;`")?;
        self.close(restriction, NodeKind::Restriction);

        Ok(())
    }

    /// A conformance supplies members with inline machines or named selection rows.
    pub(crate) fn parse_conformance(&mut self) -> Parsed {
        let conformance = self.open();

        self.eat_visibility();
        self.expect_word("a name")?;

        if self.at(Punctuation::Less) {
            self.parse_generic_params()?;
        }

        self.expect_keyword("satisfies", "`satisfies`")?;
        self.parse_type(TypePosition::Plain)?;

        // The only clauses a conformance takes are hypotheses, `requires machine ...;`.
        while self.at_keyword("requires") && self.nth_at_keyword(1, "machine") {
            self.parse_hypothesis(TypePosition::Plain)?;
        }

        self.refuse_separator_before_body()?;
        self.expect(Punctuation::LeftBrace, "`{`")?;

        while !self.at(Punctuation::RightBrace) {
            if self.at_machine_declaration() {
                self.parse_item()?;
            } else if self.at_word() && self.nth_at(1, Punctuation::ColonColon) {
                let row = self.open();
                let requirement = self.open();
                self.bump();
                self.bump();
                self.expect_word("a trait member name")?;
                self.close(requirement, NodeKind::Path);
                self.expect(Punctuation::Equal, "`=`")?;
                self.parse_path()?;
                self.expect(Punctuation::Semicolon, "`;`")?;
                self.close(row, NodeKind::SelectionRow);
            } else {
                return Err(self.error("a machine, selection row or `}`"));
            }
        }

        self.bump();
        self.close(conformance, NodeKind::Conformance);

        Ok(())
    }
}

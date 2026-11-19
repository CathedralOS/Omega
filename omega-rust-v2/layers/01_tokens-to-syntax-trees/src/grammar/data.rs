//! `data` declarations: records, sums, aliases, and the attributes they carry.

use crate::grammar::contexts::expression_context::ExpressionContext;
use crate::grammar::contexts::type_position::TypePosition;
use crate::parsing::parse_failed::Parsed;
use crate::parsing::syntax_parser::SyntaxParser;
use crate::tree::node_kind::NodeKind;
use omega_lexer::Punctuation;

impl SyntaxParser<'_> {
    /// `visibility? 'data' word generic_params? attributes? dropped_by?
    ///  ( '=' type ';' | requires* '{' data_member* '}' )`
    pub(crate) fn parse_data(&mut self) -> Parsed {
        self.enter_nesting()?;

        let data = self.open();

        self.eat_visibility();
        self.bump();
        self.expect_word("a name")?;

        if self.at(Punctuation::Less) {
            self.parse_generic_params()?;
        }

        if self.at(Punctuation::LeftBracket) {
            self.parse_attributes()?;
        }

        if self.at_dropped_by() {
            self.parse_dropped_by()?;
        }

        if self.eat(Punctuation::Equal) {
            self.parse_type(TypePosition::Plain)?;
            self.expect(Punctuation::Semicolon, "`;`")?;
        } else {
            while self.at_keyword("requires") {
                self.parse_clause(NodeKind::Requires, TypePosition::Plain)?;
            }

            self.refuse_separator_before_body()?;
            self.expect(Punctuation::LeftBrace, "`{` or `=`")?;

            while !self.at(Punctuation::RightBrace) {
                self.parse_data_member()?;
            }

            self.bump();
        }

        self.close(data, NodeKind::Data);
        self.leave_nesting();

        Ok(())
    }

    /// A field, a case, or a nested `data`.
    ///
    /// `case` and `data` introduce their forms only when a name follows, so a field may be called
    /// `case` or `data`: in `data: Payload;` the word is followed by `:`.
    fn parse_data_member(&mut self) -> Parsed {
        if self.at_keyword("data") && self.nth_at_word(1) {
            return self.parse_data();
        }

        let member = self.open();

        if self.at(Punctuation::Hash) {
            self.parse_stable_number()?;
        }

        if self.at_keyword("case") && self.nth_at_word(1) {
            self.bump();
            self.bump();

            if self.eat(Punctuation::LeftParenthesis) {
                while !self.at(Punctuation::RightParenthesis) {
                    self.parse_case_field()?;

                    if !self.eat(Punctuation::Comma) {
                        break;
                    }
                }

                self.expect(Punctuation::RightParenthesis, "`,` or `)`")?;
            }

            self.expect(Punctuation::Semicolon, "`;`")?;
            self.close(member, NodeKind::Case);

            return Ok(());
        }

        self.expect_word("a field, a `case`, a nested `data` or `}`")?;

        if self.at(Punctuation::LeftBracket) {
            self.parse_attributes()?;
        }

        self.expect(Punctuation::Colon, "`:`")?;

        // A field's type may be declared in place: `op: data Operation { ... };`
        if self.at_keyword("data") && self.nth_at_word(1) {
            self.parse_data()?;
        } else {
            self.parse_type_in(TypePosition::Plain)?;
        }

        self.expect(Punctuation::Semicolon, "`;`")?;
        self.close(member, NodeKind::Field);

        Ok(())
    }

    /// `stable_number? word ':' type`
    fn parse_case_field(&mut self) -> Parsed {
        let case_field = self.open();

        if self.at(Punctuation::Hash) {
            self.parse_stable_number()?;
        }

        self.expect_word("a field name")?;
        self.expect(Punctuation::Colon, "`:`")?;
        self.parse_type_in(TypePosition::Plain)?;
        self.close(case_field, NodeKind::CaseField);

        Ok(())
    }

    /// `'#' integer`
    fn parse_stable_number(&mut self) -> Parsed {
        let stable_number = self.open();

        self.bump();

        if !self.at_integer() {
            return Err(self.error("a stable number"));
        }

        self.bump();
        self.close(stable_number, NodeKind::StableNumber);

        Ok(())
    }

    /// `'[' ( attribute % ',' ) ']'` where `attribute = word ( '(' ( word ':' word ) % ',' ')' )?`
    pub(crate) fn parse_attributes(&mut self) -> Parsed {
        let attributes = self.open();

        self.bump();

        loop {
            let attribute = self.open();

            self.expect_word("an attribute")?;

            if self.eat(Punctuation::LeftParenthesis) {
                loop {
                    self.expect_word("a setting name")?;
                    self.expect(Punctuation::Colon, "`:`")?;
                    self.expect_word("a setting value")?;

                    if !self.eat(Punctuation::Comma) || self.at(Punctuation::RightParenthesis) {
                        break;
                    }
                }

                self.expect(Punctuation::RightParenthesis, "`,` or `)`")?;
            }

            self.close(attribute, NodeKind::Attribute);

            if !self.eat(Punctuation::Comma) || self.at(Punctuation::RightBracket) {
                break;
            }
        }

        self.expect(Punctuation::RightBracket, "`,` or `]`")?;
        self.close(attributes, NodeKind::Attributes);

        Ok(())
    }

    pub(crate) fn at_dropped_by(&self) -> bool {
        self.at_keyword("dropped") && self.nth_at_keyword(1, "by")
    }

    /// `'dropped' 'by' path type_args?`
    pub(crate) fn parse_dropped_by(&mut self) -> Parsed {
        let dropped_by = self.open();

        self.bump();
        self.bump();
        self.parse_path_type()?;
        self.close(dropped_by, NodeKind::DroppedBy);

        Ok(())
    }

    /// `visibility? 'domain' generic_params? path type_args?
    ///  ( '=' type_domains ';' | requires? ( 'access' type )? dropped_by? ';' )`
    pub(crate) fn parse_domain(&mut self) -> Parsed {
        let domain = self.open();

        self.eat_visibility();
        self.bump();

        if self.at(Punctuation::Less) {
            self.parse_generic_params()?;
        }

        self.parse_path_type()?;

        if self.eat(Punctuation::Equal) {
            self.parse_type_domains()?;
        } else {
            if self.at_keyword("requires") {
                // The predicate ends at `access`, `dropped` or `;`, none of which can continue an expression.
                let requires = self.open();

                self.bump();
                self.parse_expression(ExpressionContext::FULL)?;
                self.close(requires, NodeKind::Requires);
            }

            if self.at_keyword("access") {
                let access = self.open();

                self.bump();
                self.parse_type(TypePosition::Plain)?;
                self.close(access, NodeKind::Access);
            }

            if self.at_dropped_by() {
                self.parse_dropped_by()?;
            }
        }

        self.expect(Punctuation::Semicolon, "`;`")?;
        self.close(domain, NodeKind::Domain);

        Ok(())
    }
}

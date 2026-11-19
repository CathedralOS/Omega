//! Types: references, slices, arrays, machine types, function arrows and paths.

use crate::grammar::contexts::expression_context::ExpressionContext;
use crate::grammar::contexts::type_position::TypePosition;
use crate::parsing::closed_node::ClosedNode;
use crate::parsing::parse_failed::Parsed;
use crate::parsing::syntax_parser::SyntaxParser;
use crate::tree::node_kind::NodeKind;
use omega_lexer::Punctuation;

impl SyntaxParser<'_> {
    /// `type`, or `type -> type` except after `as`. The arrow groups to the right.
    pub(crate) fn parse_type(
        &mut self,
        type_position: TypePosition,
    ) -> Parsed<ClosedNode> {
        self.enter_nesting()?;

        let mut parsed_type = self.parse_type_operand(type_position)?;

        if type_position != TypePosition::Cast && self.at(Punctuation::Arrow) {
            let function_type = self.open_before(parsed_type);

            self.bump();
            self.parse_type(type_position)?;
            parsed_type = self.close(function_type, NodeKind::FunctionType);
        }

        self.leave_nesting();

        Ok(parsed_type)
    }

    /// `type ( 'in' domain ( '&' domain )* )?`, as in `i32 in Km & Positive`.
    pub(crate) fn parse_type_in(
        &mut self,
        type_position: TypePosition,
    ) -> Parsed {
        let parsed_type = self.parse_type(type_position)?;

        if self.at_keyword("in") {
            let qualified_type = self.open_before(parsed_type);

            self.bump();
            self.parse_type_domains()?;
            self.close(qualified_type, NodeKind::QualifiedType);
        }

        Ok(())
    }

    /// `path_type ( '&' path_type )*`
    pub(crate) fn parse_type_domains(&mut self) -> Parsed {
        self.parse_path_type()?;

        while self.eat(Punctuation::Ampersand) {
            self.parse_path_type()?;
        }

        Ok(())
    }

    /// A type with no function arrow at its top: `@A -> B` is a function from `@A`.
    fn parse_type_operand(
        &mut self,
        type_position: TypePosition,
    ) -> Parsed<ClosedNode> {
        if self.at(Punctuation::At) {
            let reference_type = self.open();

            self.bump();

            if self.at(Punctuation::Apostrophe) {
                self.parse_lifetime()?;
            }

            // An access word is a marker only if a type follows it; otherwise it names the referenced type.
            if (self.at_keyword("mut") || self.at_keyword("write")) && self.nth_can_start_type(1) {
                self.bump();
            }

            self.enter_nesting()?;
            self.parse_type_operand(type_position)?;
            self.leave_nesting();

            return Ok(self.close(reference_type, NodeKind::ReferenceType));
        }

        if self.at(Punctuation::LeftBracket) {
            let bracketed_type = self.open();

            self.bump();
            self.parse_type(TypePosition::Plain)?;

            let kind = if self.eat(Punctuation::Semicolon) {
                self.parse_expression(ExpressionContext::FULL)?;
                NodeKind::ArrayType
            } else {
                NodeKind::SliceType
            };

            self.expect(Punctuation::RightBracket, "`]`")?;

            return Ok(self.close(bracketed_type, kind));
        }

        if self.at_machine_type() {
            return self.parse_machine_type(type_position);
        }

        if self.at_word() {
            return self.parse_path_type();
        }

        Err(self.error("a type"))
    }

    fn nth_can_start_type(
        &self,
        ahead: usize,
    ) -> bool {
        self.nth_at_word(ahead) || self.nth_at(ahead, Punctuation::At) || self.nth_at(ahead, Punctuation::LeftBracket)
    }

    /// `machine` is a machine type when a parameter list or generic parameters follow; alone it is a name.
    fn at_machine_type(&self) -> bool {
        self.at_keyword("machine") && (self.nth_at(1, Punctuation::LeftParenthesis) || self.nth_at(1, Punctuation::Less))
    }

    /// `'machine' generic_params? '(' params ')' result? clause*`
    fn parse_machine_type(
        &mut self,
        type_position: TypePosition,
    ) -> Parsed<ClosedNode> {
        let machine_type = self.open();

        self.bump();

        if self.at(Punctuation::Less) {
            self.parse_generic_params()?;
        }

        self.parse_params()?;

        if self.at(Punctuation::Arrow) {
            self.parse_result(type_position)?;
        }

        self.parse_clauses(type_position)?;

        Ok(self.close(machine_type, NodeKind::MachineType))
    }

    /// `word type_args? ( '::' word type_args? )*`
    ///
    /// In a type, `<` after a name always opens generic arguments. That holds after `as` too, so
    /// `x as u32 < y` is refused and is written `(x as u32) < y`.
    pub(crate) fn parse_path_type(&mut self) -> Parsed<ClosedNode> {
        let path_type = self.open();

        self.expect_word("a type")?;

        loop {
            if self.at(Punctuation::Less) {
                self.parse_type_args()?;
            }

            if !(self.at(Punctuation::ColonColon) && self.nth_at_word(1)) {
                break;
            }

            self.bump();
            self.bump();
        }

        Ok(self.close(path_type, NodeKind::PathType))
    }

    /// `''' word`
    pub(crate) fn parse_lifetime(&mut self) -> Parsed {
        let lifetime = self.open();

        self.bump();
        self.expect_word("a lifetime name")?;
        self.close(lifetime, NodeKind::Lifetime);

        Ok(())
    }
}

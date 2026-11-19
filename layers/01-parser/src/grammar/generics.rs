//! Generic parameters on declarations, and generic arguments in both of their spellings.

use crate::grammar::contexts::expression_context::ExpressionContext;
use crate::grammar::contexts::type_position::TypePosition;
use crate::parsing::parse_failed::Parsed;
use crate::parsing::syntax_parser::SyntaxParser;
use crate::tree::node_kind::NodeKind;
use omega_lexer::Punctuation;

impl SyntaxParser<'_> {
    /// `'<' ( generic_param % ',' ) '>'`
    pub(crate) fn parse_generic_params(&mut self) -> Parsed {
        let generic_params = self.open();

        self.bump();

        loop {
            self.parse_generic_param()?;

            if !self.eat(Punctuation::Comma) || self.at(Punctuation::Greater) {
                break;
            }
        }

        self.expect(Punctuation::Greater, "`,` or `>`")?;
        self.close(generic_params, NodeKind::GenericParams);

        Ok(())
    }

    /// One parameter. `const`, `machine` and `some` introduce their forms only when a name follows,
    /// so each of them is still usable as the name of an ordinary type parameter.
    pub(crate) fn parse_generic_param(&mut self) -> Parsed {
        let generic_param = self.open();

        let kind = if self.at(Punctuation::Apostrophe) {
            self.bump();
            self.expect_word("a lifetime name")?;
            NodeKind::LifetimeParam
        } else if self.at_keyword("const") && self.nth_at_word(1) {
            self.bump();
            self.bump();
            self.expect(Punctuation::Colon, "`:`")?;
            self.parse_type(TypePosition::InsideAngles)?;
            NodeKind::ConstParam
        } else if self.at_keyword("machine") && self.nth_at_word(1) {
            self.bump();
            self.bump();
            NodeKind::MachineParam
        } else if self.at_keyword("some") && self.nth_at_word(1) {
            self.bump();
            self.bump();
            NodeKind::SomeParam
        } else {
            self.expect_word("a generic parameter")?;

            if self.at(Punctuation::LeftBracket) {
                self.parse_attributes()?;
            }

            if self.eat(Punctuation::Colon) {
                self.parse_type(TypePosition::InsideAngles)?;
                NodeKind::KindedParam
            } else {
                NodeKind::TypeParam
            }
        };

        self.close(generic_param, kind);

        Ok(())
    }

    /// `'<' ( generic_arg % ',' ) '>'`, the spelling used in types and declarations.
    pub(crate) fn parse_type_args(&mut self) -> Parsed {
        let type_args = self.open();

        self.bump();
        self.parse_generic_arg_list()?;
        self.close(type_args, NodeKind::TypeArgs);

        Ok(())
    }

    /// `'::' '<' ( generic_arg % ',' ) '>'`, the spelling used in expressions.
    pub(crate) fn parse_expression_args(&mut self) -> Parsed {
        let expression_args = self.open();

        self.bump();
        self.bump();
        self.parse_generic_arg_list()?;
        self.close(expression_args, NodeKind::ExpressionArgs);

        Ok(())
    }

    fn parse_generic_arg_list(&mut self) -> Parsed {
        loop {
            self.parse_generic_arg()?;

            if !self.eat(Punctuation::Comma) || self.at(Punctuation::Greater) {
                break;
            }
        }

        self.expect(Punctuation::Greater, "`,` or `>`")
    }

    fn parse_generic_arg(&mut self) -> Parsed {
        if self.at(Punctuation::Apostrophe) {
            return self.parse_lifetime();
        }

        if self.at(Punctuation::Greater) || self.at(Punctuation::Comma) {
            return Err(self.error("a generic argument"));
        }

        if self.generic_arg_is_type() {
            self.parse_type(TypePosition::InsideAngles)?;
        } else {
            self.parse_expression(ExpressionContext::INSIDE_ANGLES)?;
        }

        Ok(())
    }

    /*
        :GenericArgumentKind
        A generic argument is a type (`Outcome<u32, Never>`) or a value
        (`ByteBuffer<64>`, `Buffer<N * 2>`, `Routes<[open_file(), open_dir()]>`),
        and the two can start alike. We decide by looking ahead, without trying
        one and falling back to the other:

          - Skip any `[` the argument opens with. They belong to a slice or
            array type, or to an array literal, and what follows them settles
            which.
          - `@` or `machine(` there begins a type.
          - A name there, with any `::name` parts, begins a type if the token
            after it continues a type or ends the argument: `<`, `->`, and
            `,` or `>` at the top, `]` or `;` inside brackets. So `[u8]` and
            `[u8; 4]` are types, and `[a, b]` and `[f(x)]` are array literals.
          - Everything else is an expression: a literal, a parenthesis, or a
            name followed by an operator, a call or `::<`.

        So `Buffer<N>` holds the type-shaped argument `N`, which may turn out to
        name a value; telling which is the resolver's job. Two shapes land on
        the wrong side and must be written with parentheses: a value that
        begins with a generic type, `Buffer<(Other::<T>::SIZE + 1)>`, and an
        array literal of one bare name, `Routes<[(open_file)]>`. Inside the
        parentheses it is an expression, so its own generic arguments are
        written `::<`.
    */
    fn generic_arg_is_type(&self) -> bool {
        let mut ahead = 0;

        while self.nth_at(ahead, Punctuation::LeftBracket) {
            ahead += 1;
        }

        let is_inside_brackets = ahead > 0;

        if self.nth_at(ahead, Punctuation::At) {
            return true;
        }

        if !self.nth_at_word(ahead) {
            return false;
        }

        if self.nth_at_keyword(ahead, "machine") && self.nth_at(ahead + 1, Punctuation::LeftParenthesis) {
            return true;
        }

        ahead += 1;

        while self.nth_at(ahead, Punctuation::ColonColon) && self.nth_at_word(ahead + 1) {
            ahead += 2;
        }

        match self.nth_punctuation(ahead) {
            Some(Punctuation::Less | Punctuation::Arrow) => true,
            Some(Punctuation::Comma | Punctuation::Greater) => !is_inside_brackets,
            Some(Punctuation::RightBracket | Punctuation::Semicolon) => is_inside_brackets,
            _ => false,
        }
    }
}

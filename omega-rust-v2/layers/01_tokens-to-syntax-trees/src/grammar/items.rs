//! Declarations: which one starts at the cursor, and `use`, `const`, machines and mathematical `let`.

use crate::errors::parse_error_kind::ParseErrorKind;
use crate::grammar::contexts::body_rule::BodyRule;
use crate::grammar::contexts::expression_context::ExpressionContext;
use crate::grammar::contexts::type_position::TypePosition;
use crate::parsing::parse_failed::Parsed;
use crate::parsing::syntax_parser::SyntaxParser;
use crate::tree::node_kind::NodeKind;
use omega_lexer::Punctuation;

/// The declarations that begin with a word of their own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ItemKind {
    Use,
    Const,
    Data,
    Domain,
    Trait,
    Machine,
    AsmMachine,
    MathLet,
    Conformance,
}

impl SyntaxParser<'_> {
    pub(crate) fn parse_item(&mut self) -> Parsed {
        match self.peek_item_kind() {
            Some(ItemKind::Use) => self.parse_use(),
            Some(ItemKind::Const) => self.parse_const(),
            Some(ItemKind::Data) => self.parse_data(),
            Some(ItemKind::Domain) => self.parse_domain(),
            Some(ItemKind::Trait) => self.parse_trait(),
            Some(ItemKind::Machine) => self.parse_machine(BodyRule::Optional),
            Some(ItemKind::AsmMachine) => self.parse_asm_machine(),
            Some(ItemKind::MathLet) => self.parse_math_let(),
            Some(ItemKind::Conformance) => self.parse_conformance(),
            None => Err(self.error("a declaration")),
        }
    }

    pub(crate) fn at_item(&self) -> bool {
        self.peek_item_kind().is_some()
    }

    /// True at a machine or `asm machine` declaration, the only declarations a body may hold.
    pub(crate) fn at_machine_declaration(&self) -> bool {
        matches!(self.peek_keyword_item_kind(), Some(ItemKind::Machine | ItemKind::AsmMachine))
    }

    fn peek_item_kind(&self) -> Option<ItemKind> {
        if let Some(item_kind) = self.peek_keyword_item_kind() {
            return Some(item_kind);
        }

        let name_offset = usize::from(self.at_visibility());

        if self.nth_at_word(name_offset) && self.conformance_follows(name_offset + 1) {
            return Some(ItemKind::Conformance);
        }

        None
    }

    /*
        :DeclarationWords
        No word is reserved, so `data`, `machine`, `use` and the rest introduce
        a declaration only when what follows them fits one. Each test below
        looks one or two tokens past the word:

            use NAME            const NAME          data NAME
            domain NAME|<       trait NAME          machine NAME|operator
            asm machine NAME    let NAME ( | <

        Anywhere those tests fail the word is an ordinary name, so `data = 3;`
        and `machine.run()` are statements. Two words in a row are never a
        legal expression, which is why testing for a following name is safe.
    */
    fn peek_keyword_item_kind(&self) -> Option<ItemKind> {
        let mut keyword_offset = usize::from(self.at_visibility());

        if self.nth_at_keyword(keyword_offset, "boundary") {
            let qualified_offset = keyword_offset + 1;

            if self.nth_at_keyword(qualified_offset, "trait") && self.nth_at_word(qualified_offset + 1) {
                return Some(ItemKind::Trait);
            }

            if self.nth_at_keyword(qualified_offset, "let") && self.math_let_follows(qualified_offset + 1) {
                // A mathematical `let` takes no visibility; see below.
                return (!self.at_visibility()).then_some(ItemKind::MathLet);
            }

            if self.nth_at_keyword(qualified_offset, "machine") && self.machine_name_follows(qualified_offset + 1) {
                keyword_offset = qualified_offset;
            }
        }

        let name_offset = keyword_offset + 1;
        let is_name_next = self.nth_at_word(name_offset);
        let item_kind = self.peek_item_kind_at(keyword_offset, name_offset, is_name_next)?;

        // `use` and the mathematical `let` take no visibility, so `public use a;` is not a declaration.
        let takes_visibility = !matches!(item_kind, ItemKind::Use | ItemKind::MathLet);

        if self.at_visibility() && !takes_visibility {
            return None;
        }

        Some(item_kind)
    }

    fn peek_item_kind_at(
        &self,
        keyword_offset: usize,
        name_offset: usize,
        is_name_next: bool,
    ) -> Option<ItemKind> {
        match self.nth_word(keyword_offset)? {
            b"use" if is_name_next => Some(ItemKind::Use),
            b"const" if is_name_next => Some(ItemKind::Const),
            b"data" if is_name_next => Some(ItemKind::Data),
            b"domain" if is_name_next || self.nth_at(name_offset, Punctuation::Less) => Some(ItemKind::Domain),
            b"trait" if is_name_next => Some(ItemKind::Trait),
            b"machine" if self.machine_name_follows(name_offset) => Some(ItemKind::Machine),
            b"asm" if self.nth_at_keyword(name_offset, "machine") && self.nth_at_word(name_offset + 1) => Some(ItemKind::AsmMachine),
            b"let" if self.math_let_follows(name_offset) => Some(ItemKind::MathLet),
            _ => None,
        }
    }

    /// `public` is a visibility when a declaration follows it.
    ///
    /// The one exception keeps `public` usable as a type's name: `public satisfies T { }` declares
    /// a conformance for a type named `public`, and is not a visible declaration of `satisfies`.
    pub(crate) fn at_visibility(&self) -> bool {
        if !self.at_keyword("public") || !self.nth_at_word(1) {
            return false;
        }

        let names_a_conformance = self.nth_at_keyword(1, "satisfies") && !self.nth_at_keyword(2, "satisfies") && !self.nth_at(2, Punctuation::Less);

        !names_a_conformance
    }

    pub(crate) fn eat_visibility(&mut self) {
        if self.at_visibility() {
            self.bump();
        }
    }

    /// A mathematical `let` is `let NAME (` or `let NAME <`; a `let` statement is never either.
    fn math_let_follows(
        &self,
        name_offset: usize,
    ) -> bool {
        self.nth_at_word(name_offset) && (self.nth_at(name_offset + 1, Punctuation::LeftParenthesis) || self.nth_at(name_offset + 1, Punctuation::Less))
    }

    /// After `machine`: a name, or an operator token and then a name.
    fn machine_name_follows(
        &self,
        ahead: usize,
    ) -> bool {
        if self.nth_at_word(ahead) {
            return true;
        }

        match self.nth_punctuation(ahead) {
            Some(Punctuation::LeftBracket) => {
                let is_index_operator = self.nth_at(ahead + 1, Punctuation::RightBracket) && self.nth_at_word(ahead + 2);
                let is_view_operator =
                    self.nth_at(ahead + 1, Punctuation::DotDot) && self.nth_at(ahead + 2, Punctuation::RightBracket) && self.nth_at_word(ahead + 3);

                is_index_operator || is_view_operator
            }
            Some(punctuation) => is_operator_token(punctuation) && self.nth_at_word(ahead + 1),
            None => false,
        }
    }

    /// With `ahead` just past a name: true if `satisfies` follows, directly or after `<...>`.
    ///
    /// The scan over `<...>` stops at the first `;`, `{` or `}`, so it never runs past the end of
    /// the statement or declaration it started in.
    fn conformance_follows(
        &self,
        mut ahead: usize,
    ) -> bool {
        if self.nth_at(ahead, Punctuation::Less) {
            let mut angle_depth = 0usize;

            loop {
                match self.nth_punctuation(ahead) {
                    Some(Punctuation::Less) => angle_depth += 1,
                    Some(Punctuation::Greater) => {
                        angle_depth = angle_depth.saturating_sub(1);

                        if angle_depth == 0 {
                            break;
                        }
                    }
                    Some(Punctuation::Semicolon | Punctuation::LeftBrace | Punctuation::RightBrace) => return false,
                    Some(_) => {}
                    None if !self.nth_exists(ahead) => return false,
                    None => {}
                }

                ahead += 1;
            }

            ahead += 1;
        }

        self.nth_at_keyword(ahead, "satisfies")
    }

    /// `word ( '::' word )*`
    pub(crate) fn parse_path(&mut self) -> Parsed {
        let path = self.open();

        self.expect_word("a name")?;

        while self.at(Punctuation::ColonColon) && self.nth_at_word(1) {
            self.bump();
            self.bump();
        }

        self.close(path, NodeKind::Path);

        Ok(())
    }

    /// `'use' path ';'`
    fn parse_use(&mut self) -> Parsed {
        let use_item = self.open();

        self.bump();
        self.parse_path()?;
        self.expect(Punctuation::Semicolon, "`;`")?;
        self.close(use_item, NodeKind::Use);

        Ok(())
    }

    /// `visibility? 'const' path ( ':' type )? '=' expression ';'`
    fn parse_const(&mut self) -> Parsed {
        let const_item = self.open();

        self.eat_visibility();
        self.bump();
        self.parse_path()?;

        if self.eat(Punctuation::Colon) {
            self.parse_type(TypePosition::Plain)?;
        }

        self.expect(Punctuation::Equal, "`=`")?;
        self.parse_expression(ExpressionContext::FULL)?;
        self.expect(Punctuation::Semicolon, "`;`")?;
        self.close(const_item, NodeKind::Const);

        Ok(())
    }

    /// `visibility? 'boundary'? 'machine' operator_token? path generic_params? '(' params ')'
    ///  result? clause* ( body | ';' )`
    pub(crate) fn parse_machine(
        &mut self,
        body_rule: BodyRule,
    ) -> Parsed {
        let machine = self.open();

        self.eat_visibility();

        if self.at_keyword("boundary") && self.nth_at_keyword(1, "machine") {
            self.bump();
        }

        self.expect_keyword("machine", "`machine`")?;
        self.parse_operator_token_if_present()?;
        self.parse_path()?;

        if self.at(Punctuation::Less) {
            self.parse_generic_params()?;
        }

        self.parse_params()?;

        if self.at(Punctuation::Arrow) {
            self.parse_result(TypePosition::Plain)?;
        }

        self.parse_clauses(TypePosition::Plain)?;

        match body_rule {
            BodyRule::Optional => self.parse_body_or_semicolon()?,
            BodyRule::Refused => self.expect(Punctuation::Semicolon, "a clause or `;`")?,
        }

        self.close(machine, NodeKind::Machine);

        Ok(())
    }

    /// `'+' | '-' | ... | '[' ']' | '[' '..' ']'`, present when the machine declares an operator.
    fn parse_operator_token_if_present(&mut self) -> Parsed {
        if self.at_word() {
            return Ok(());
        }

        let operator_token = self.open();

        if self.eat(Punctuation::LeftBracket) {
            self.eat(Punctuation::DotDot);
            self.expect(Punctuation::RightBracket, "`]`")?;
        } else if self.nth_punctuation(0).is_some_and(is_operator_token) {
            self.bump();
        } else {
            return Err(self.error("a machine name or an operator"));
        }

        self.close(operator_token, NodeKind::OperatorToken);

        Ok(())
    }

    /// `'(' ( param % ',' )? ')'` where `param = word ':' type_in`
    pub(crate) fn parse_params(&mut self) -> Parsed {
        let params = self.open();

        self.expect(Punctuation::LeftParenthesis, "`(`")?;

        while !self.at(Punctuation::RightParenthesis) {
            let param = self.open();

            self.expect_word("a parameter name or `)`")?;
            self.expect(Punctuation::Colon, "`:`")?;
            self.parse_type_in(TypePosition::Plain)?;
            self.close(param, NodeKind::Param);

            if !self.eat(Punctuation::Comma) {
                break;
            }
        }

        self.expect(Punctuation::RightParenthesis, "`,` or `)`")?;
        self.close(params, NodeKind::Params);

        Ok(())
    }

    /// `'->' type_in`
    pub(crate) fn parse_result(
        &mut self,
        type_position: TypePosition,
    ) -> Parsed {
        let result = self.open();

        self.bump();
        self.parse_type_in(type_position)?;
        self.close(result, NodeKind::Result);

        Ok(())
    }

    /// What ends a header: a body, or `;` for a declaration that has none.
    ///
    /// Nothing may sit between the last clause and the body (decision 4). A `;` or `,` there gets
    /// its own error, since "expected `{`" pointing at a `;` reads like a riddle.
    pub(crate) fn parse_body_or_semicolon(&mut self) -> Parsed {
        self.refuse_separator_before_body()?;

        if self.eat(Punctuation::Semicolon) {
            return Ok(());
        }

        if self.at(Punctuation::LeftBrace) {
            return self.parse_body();
        }

        Err(self.error("a clause, `{` or `;`"))
    }

    pub(crate) fn refuse_separator_before_body(&mut self) -> Parsed {
        let is_separator = self.at(Punctuation::Semicolon) || self.at(Punctuation::Comma);

        if is_separator && self.nth_at(1, Punctuation::LeftBrace) {
            return Err(self.error_of_kind(ParseErrorKind::SeparatorBeforeBody, "`{`"));
        }

        Ok(())
    }

    /// `'boundary'? 'let' word generic_params? '(' params ')' ':' type ( '=' expression )? ';'`
    fn parse_math_let(&mut self) -> Parsed {
        let math_let = self.open();

        self.eat_keyword("boundary");
        self.bump();
        self.expect_word("a name")?;

        if self.at(Punctuation::Less) {
            self.parse_generic_params()?;
        }

        self.parse_params()?;
        self.expect(Punctuation::Colon, "`:`")?;
        self.parse_type(TypePosition::Plain)?;

        if self.eat(Punctuation::Equal) {
            self.parse_expression(ExpressionContext::FULL)?;
        }

        self.expect(Punctuation::Semicolon, "`;`")?;
        self.close(math_let, NodeKind::MathLet);

        Ok(())
    }
}

/// The operators a machine may declare: `machine + add(...)`. `[]` and `[..]` are handled apart.
fn is_operator_token(punctuation: Punctuation) -> bool {
    matches!(
        punctuation,
        Punctuation::Plus
            | Punctuation::Minus
            | Punctuation::Asterisk
            | Punctuation::Slash
            | Punctuation::Percent
            | Punctuation::Less
            | Punctuation::Greater
            | Punctuation::LessEqual
            | Punctuation::GreaterEqual
            | Punctuation::EqualEqual
            | Punctuation::ExclamationEqual
    )
}

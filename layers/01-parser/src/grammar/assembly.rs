//! Assembly: `asm machine` declarations and `asm { ... }` blocks.

use crate::expression_context::ExpressionContext;
use crate::grammar::types::TypePosition;
use crate::node_kind::NodeKind;
use crate::parser::{Parsed, Parser};
use omega_lexer::Punctuation;

impl Parser<'_> {
    /// `visibility? 'asm' 'machine' path generic_params? ( '(' params ')' )? result? clause*
    ///  '{' asm_line* asm_machine* '}'`
    pub(crate) fn parse_asm_machine(&mut self) -> Parsed {
        self.enter_nesting()?;

        let asm_machine = self.open();

        self.eat_visibility();
        self.bump();
        self.bump();
        self.parse_path()?;

        if self.at(Punctuation::Less) {
            self.parse_generic_params()?;
        }

        if self.at(Punctuation::LeftParenthesis) {
            self.parse_params()?;
        }

        if self.at(Punctuation::Arrow) {
            self.parse_result(TypePosition::Plain)?;
        }

        self.parse_clauses(TypePosition::Plain)?;
        self.refuse_separator_before_body()?;
        self.expect(Punctuation::LeftBrace, "a clause or `{`")?;
        self.parse_asm_lines()?;

        // Nested assembly machines follow the instructions of their parent, as nested machines follow statements.
        while self.at_asm_machine() {
            self.parse_asm_machine()?;
        }

        self.expect(Punctuation::RightBrace, "`;` or `}`")?;
        self.close(asm_machine, NodeKind::AsmMachine);
        self.leave_nesting();

        Ok(())
    }

    fn at_asm_machine(&self) -> bool {
        self.at_keyword("asm") && self.nth_at_keyword(1, "machine") && self.nth_at_word(2)
    }

    /// `asm` begins a block when `{` or a clause word follows it; otherwise it is a name.
    pub(crate) fn at_asm_block(&self) -> bool {
        self.at_keyword("asm") && (self.nth_at(1, Punctuation::LeftBrace) || self.nth_at_clause_keyword(1))
    }

    /// `'asm' clause* '{' asm_line* '}'`
    pub(crate) fn parse_asm_block(&mut self) -> Parsed {
        let asm_block = self.open();

        self.bump();
        self.parse_clauses(TypePosition::Plain)?;
        self.refuse_separator_before_body()?;
        self.expect(Punctuation::LeftBrace, "a clause or `{`")?;
        self.parse_asm_lines()?;
        self.expect(Punctuation::RightBrace, "`;` or `}`")?;
        self.close(asm_block, NodeKind::AsmBlock);

        Ok(())
    }

    /// `asm_line*`, where "instructions use `;` separators" and the last may omit its own.
    fn parse_asm_lines(&mut self) -> Parsed {
        while !self.at(Punctuation::RightBrace) && !self.at_asm_machine() {
            if self.at_keyword("transition") && self.nth_at(1, Punctuation::Arrow) {
                let asm_transition = self.open();

                self.bump();
                self.bump();
                self.expect_word("the name of the next block")?;
                self.close(asm_transition, NodeKind::AsmTransition);
            } else {
                self.parse_instruction()?;
            }

            if !self.eat(Punctuation::Semicolon) {
                break;
            }
        }

        Ok(())
    }

    /// `word ( '.' word )? ( expression % ',' )?`
    ///
    /// Operands are ordinary expressions. Whether the mnemonic exists and takes those operands is
    /// a question for the layer that knows the target, not for this one.
    fn parse_instruction(&mut self) -> Parsed {
        // "Every assembly-bodied declaration spells `asm machine`, including a declaration nested
        // inside another assembly machine." Without this the word `machine` would be read as a
        // mnemonic and the refusal would arrive later, at a token that explains nothing.
        if self.at_machine_declaration() {
            return Err(self.error("`asm machine`; a machine nested in assembly is itself assembly"));
        }

        let instruction = self.open();

        self.expect_word("an instruction or `}`")?;

        if self.at(Punctuation::Dot) && self.nth_at_word(1) {
            self.bump();
            self.bump();
        }

        // The last instruction may omit its `;`, so what follows it can be the `}` or a nested machine.
        let has_operands = !self.at(Punctuation::Semicolon) && !self.at(Punctuation::RightBrace) && !self.at_asm_machine();

        if has_operands {
            loop {
                self.parse_expression(ExpressionContext::FULL)?;

                if !self.eat(Punctuation::Comma) {
                    break;
                }
            }
        }

        self.close(instruction, NodeKind::Instruction);

        Ok(())
    }
}

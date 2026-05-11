use crate::parse_error::ParseError;
use crate::syntax::{SyntaxFile, SyntaxKind, SyntaxNodeHandle, SyntaxTable};
use omega_core::source::FileId;
use omega_source_files_to_tokens::Token;

pub(super) fn parse_syntax_file_impl(
    file_id: FileId,
    tokens: &[Token<'_>],
) -> Result<SyntaxFile, ParseError> {
    let mut parser = SyntaxParser::new(file_id, tokens);
    parser.parse_file()
}

struct SyntaxParser<'tokens, 'source> {
    file_id: FileId,
    tokens: &'tokens [Token<'source>],
    index: usize,
    syntax: SyntaxTable,
    file_tokens: omega_core::arena::HandleSpan<crate::syntax::SyntaxToken>,
}

impl<'tokens, 'source> SyntaxParser<'tokens, 'source> {
    fn new(file_id: FileId, tokens: &'tokens [Token<'source>]) -> Self {
        let mut syntax = SyntaxTable::new();
        let file_tokens = syntax.insert_tokens(tokens);

        Self {
            file_id,
            tokens,
            index: 0,
            syntax,
            file_tokens,
        }
    }

    fn parse_file(&mut self) -> Result<SyntaxFile, ParseError> {
        let start = self.index;
        let mut children = Vec::new();

        while !self.is_at_end() {
            children.push(self.parse_item()?);
        }

        let root = self.insert_node(SyntaxKind::File, start, self.index, children);
        Ok(SyntaxFile {
            file_id: self.file_id,
            root,
            file_tokens: self.file_tokens,
            syntax: std::mem::take(&mut self.syntax),
        })
    }

    fn parse_item(&mut self) -> Result<SyntaxNodeHandle, ParseError> {
        if self.check("use") {
            return self.parse_simple_statement_like_item(SyntaxKind::UseItem);
        }
        if self.check("trust") {
            return self.parse_braced_or_statement_item(SyntaxKind::TrustItem);
        }
        if self.check("target") {
            return self.parse_opaque_braced_item(SyntaxKind::TargetItem);
        }
        if self.check("capability") {
            return self.parse_opaque_braced_item(SyntaxKind::CapabilityItem);
        }
        if self.check("invariant") {
            return self.parse_braced_or_statement_item(SyntaxKind::InvariantItem);
        }
        if self.check("library") {
            return self.parse_opaque_braced_item(SyntaxKind::LibraryItem);
        }
        if self.check("enum") {
            return self.parse_opaque_braced_item(SyntaxKind::EnumItem);
        }
        if self.check("data") {
            return self.parse_opaque_braced_item(SyntaxKind::DataItem);
        }
        if self.check("platform") {
            return self.parse_opaque_braced_item(SyntaxKind::PlatformItem);
        }
        if self.check("machine") {
            return self.parse_machine();
        }

        Err(self.error_here("expected top-level item"))
    }

    fn parse_machine(&mut self) -> Result<SyntaxNodeHandle, ParseError> {
        let start = self.index;
        self.expect("machine")?;

        while !self.check("{") {
            self.advance_or_error("unterminated machine header")?;
        }

        self.expect("{")?;
        let mut children = Vec::new();

        while !self.consume("}") {
            children.push(self.parse_machine_item()?);
        }

        Ok(self.insert_node(SyntaxKind::MachineItem, start, self.index, children))
    }

    fn parse_machine_item(&mut self) -> Result<SyntaxNodeHandle, ParseError> {
        if self.check("contains") {
            return self.parse_simple_statement_like_item(SyntaxKind::MachineContains);
        }
        if self.check("owns") {
            return self.parse_simple_statement_like_item(SyntaxKind::MachineOwns);
        }
        if self.check("invariant") {
            return self.parse_braced_or_statement_item(SyntaxKind::MachineInvariant);
        }

        let start = self.index;
        let kind = if self.consume("pub") {
            if self.consume("entry") {
                SyntaxKind::CallableEntry
            } else {
                return Err(self.error_here("expected `entry` after `pub`"));
            }
        } else if self.consume("entry") {
            SyntaxKind::CallableEntry
        } else if self.consume("state") {
            SyntaxKind::CallableState
        } else if self.consume("fn") {
            SyntaxKind::CallableFn
        } else {
            return Err(self.error_here("expected machine item"));
        };

        self.parse_callable_body(kind, start)
    }

    fn parse_callable_body(
        &mut self,
        kind: SyntaxKind,
        start: usize,
    ) -> Result<SyntaxNodeHandle, ParseError> {
        while !self.check("{") {
            self.advance_or_error("unterminated callable header")?;
        }

        self.expect("{")?;
        let mut children = Vec::new();

        while !self.consume("}") {
            children.push(self.parse_statement()?);
        }

        Ok(self.insert_node(kind, start, self.index, children))
    }

    fn parse_statement(&mut self) -> Result<SyntaxNodeHandle, ParseError> {
        if self.check("let") {
            return self.parse_simple_statement_like_item(SyntaxKind::StatementLet);
        }
        if self.check("->") {
            return self.parse_transition_statement();
        }
        if self.check("transition") || self.check("match") {
            return self.parse_transition_block();
        }

        self.parse_opaque_statement()
    }

    fn parse_transition_statement(&mut self) -> Result<SyntaxNodeHandle, ParseError> {
        let start = self.index;
        self.expect("->")?;

        while !self.is_at_end() && !self.check(";") && !self.check("}") {
            self.advance();
        }

        let _ = self.consume(";");
        Ok(self.insert_node(SyntaxKind::StatementTransition, start, self.index, []))
    }

    fn parse_transition_block(&mut self) -> Result<SyntaxNodeHandle, ParseError> {
        let start = self.index;
        self.advance();

        while !self.check("{") {
            self.advance_or_error("unterminated transition block")?;
        }

        self.expect("{")?;
        let mut depth = 1usize;

        while depth > 0 {
            let token = self.advance_or_error("unterminated transition block")?;

            match token.lexeme.as_str() {
                "{" => depth += 1,
                "}" => depth -= 1,
                _ => {}
            }
        }

        Ok(self.insert_node(SyntaxKind::StatementTransitionBlock, start, self.index, []))
    }

    fn parse_opaque_statement(&mut self) -> Result<SyntaxNodeHandle, ParseError> {
        let start = self.index;
        let mut paren_depth = 0usize;
        let mut bracket_depth = 0usize;
        let mut brace_depth = 0usize;

        while !self.is_at_end() {
            let Some(token) = self.peek() else {
                break;
            };

            if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
                if token.lexeme.as_str() == ";" {
                    self.advance();
                    break;
                }

                if token.lexeme.as_str() == "}" {
                    break;
                }
            }

            let token = self.advance_or_error("unterminated statement")?;
            match token.lexeme.as_str() {
                "(" => paren_depth += 1,
                ")" => paren_depth = paren_depth.saturating_sub(1),
                "[" => bracket_depth += 1,
                "]" => bracket_depth = bracket_depth.saturating_sub(1),
                "{" => brace_depth += 1,
                "}" => {
                    if brace_depth == 0 {
                        self.index = self.index.saturating_sub(1);
                        break;
                    }
                    brace_depth -= 1;
                }
                _ => {}
            }
        }

        Ok(self.insert_node(SyntaxKind::StatementOpaque, start, self.index, []))
    }

    fn parse_simple_statement_like_item(
        &mut self,
        kind: SyntaxKind,
    ) -> Result<SyntaxNodeHandle, ParseError> {
        let start = self.index;

        while !self.is_at_end() && !self.consume(";") {
            self.advance();
        }

        Ok(self.insert_node(kind, start, self.index, []))
    }

    fn parse_braced_or_statement_item(
        &mut self,
        kind: SyntaxKind,
    ) -> Result<SyntaxNodeHandle, ParseError> {
        let start = self.index;

        while !self.is_at_end() {
            if self.check("{") {
                self.skip_balanced_braces()?;
                return Ok(self.insert_node(kind, start, self.index, []));
            }

            if self.consume(";") {
                return Ok(self.insert_node(kind, start, self.index, []));
            }

            self.advance();
        }

        Ok(self.insert_node(kind, start, self.index, []))
    }

    fn parse_opaque_braced_item(
        &mut self,
        kind: SyntaxKind,
    ) -> Result<SyntaxNodeHandle, ParseError> {
        let start = self.index;

        while !self.check("{") {
            self.advance_or_error("unterminated item header")?;
        }

        self.skip_balanced_braces()?;
        Ok(self.insert_node(kind, start, self.index, []))
    }

    fn insert_node(
        &mut self,
        kind: SyntaxKind,
        start_index: usize,
        end_index: usize,
        children: impl IntoIterator<Item = SyntaxNodeHandle>,
    ) -> SyntaxNodeHandle {
        let tokens = self
            .syntax
            .token_span(self.file_tokens, start_index, end_index);
        self.syntax.insert_node(kind, tokens, children)
    }

    fn skip_balanced_braces(&mut self) -> Result<(), ParseError> {
        self.expect("{")?;
        let mut depth = 1usize;

        while depth > 0 {
            let token = self.advance_or_error("unterminated block")?;

            match token.lexeme.as_str() {
                "{" => depth += 1,
                "}" => depth -= 1,
                _ => {}
            }
        }

        Ok(())
    }

    fn consume(&mut self, lexeme: &str) -> bool {
        if self.check(lexeme) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, lexeme: &str) -> Result<(), ParseError> {
        if self.consume(lexeme) {
            Ok(())
        } else {
            Err(self.error_here(format!("expected `{lexeme}`")))
        }
    }

    fn check(&self, lexeme: &str) -> bool {
        self.peek()
            .is_some_and(|token| token.lexeme.as_str() == lexeme)
    }

    fn advance(&mut self) -> Option<&Token<'source>> {
        let token = self.tokens.get(self.index)?;
        self.index += 1;
        Some(token)
    }

    fn advance_or_error(
        &mut self,
        message: impl Into<String>,
    ) -> Result<&Token<'source>, ParseError> {
        if self.is_at_end() {
            Err(self.error_here(message))
        } else {
            Ok(self.advance().expect("checked above"))
        }
    }

    fn peek(&self) -> Option<&Token<'source>> {
        self.tokens.get(self.index)
    }

    fn is_at_end(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn error_here(&self, message: impl Into<String>) -> ParseError {
        if let Some(token) = self.peek() {
            ParseError::at_span(message, token.span)
        } else {
            ParseError::new(message)
        }
    }
}

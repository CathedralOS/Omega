// Parser: tokens -> AST. Platform-independent. Enforces the Alpha subset (an
// unsupported construct is a hard error, not a silent acceptance).
//
// Grammar handled so far:
//   program := item*
//   item    := ("boundary" | "data") <balanced braces, skipped>
//            | "machine" <header> "{" stmt* "}"
//   stmt    := "let" IDENT ":" TYPE "=" expr ";"?
//            | path "(" expr ")" ";"?            (path's last segment = exit_process)
//   expr    := add ; add := mul (("+"|"-") mul)* ; mul := primary (("*"|"/") primary)*
//   primary := INT | IDENT(local) | "(" expr ")"

use crate::ast::{BinaryOp, Expr, Pattern, Program, Statement, TransitionArm};
use crate::lex::{token_text, Token, TokenKind};

pub struct Parser<'a> {
    tokens: &'a [Token],
    source: &'a [u8],
    position: usize,
    expressions: Vec<Expr>,
    local_names: Vec<Vec<u8>>,
    strings: Vec<Vec<u8>>,
    state_names: Vec<Vec<u8>>, // pre-scanned state names of the current machine
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token], source: &'a [u8]) -> Parser<'a> {
        Parser {
            tokens,
            source,
            position: 0,
            expressions: Vec::new(),
            local_names: Vec::new(),
            strings: Vec::new(),
            state_names: Vec::new(),
        }
    }

    fn find_state_index(&self, name: &[u8]) -> Option<usize> {
        self.state_names.iter().position(|state_name| state_name.as_slice() == name)
    }

    fn current_kind(&self) -> TokenKind {
        self.tokens[self.position].kind
    }
    fn current_text(&self) -> &[u8] {
        token_text(&self.tokens[self.position], self.source)
    }
    fn bump(&mut self) -> Token {
        let token = self.tokens[self.position];
        self.position += 1;
        token
    }
    fn expect(&mut self, kind: TokenKind) -> Result<Token, String> {
        if self.current_kind() == kind {
            Ok(self.bump())
        } else {
            Err(format!(
                "alpha-onramp: parse error: expected {:?}, found {:?}",
                kind,
                self.current_kind()
            ))
        }
    }
    fn add_expression(&mut self, expression: Expr) -> usize {
        self.expressions.push(expression);
        self.expressions.len() - 1
    }
    fn find_local_index(&self, name: &[u8]) -> Option<usize> {
        self.local_names.iter().position(|local_name| local_name.as_slice() == name)
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut machine: Option<(Vec<Statement>, Vec<Vec<Statement>>)> = None;
        while self.current_kind() != TokenKind::Eof {
            if self.current_kind() != TokenKind::Ident {
                return Err(format!(
                    "alpha-onramp: parse error: expected a top-level item, found {:?}",
                    self.current_kind()
                ));
            }
            match self.current_text() {
                b"boundary" | b"data" => self.skip_braced_item()?,
                b"machine" => machine = Some(self.parse_machine_body()?),
                other => {
                    return Err(format!(
                        "alpha-onramp: unsupported top-level keyword '{}' (Alpha subset)",
                        String::from_utf8_lossy(other)
                    ))
                }
            }
        }
        let (entry, states) =
            machine.ok_or_else(|| "alpha-onramp: no machine to compile".to_string())?;
        let local_count = self.local_names.len();
        Ok(Program {
            entry,
            states,
            expressions: std::mem::take(&mut self.expressions),
            local_count,
            strings: std::mem::take(&mut self.strings),
        })
    }

    fn skip_braced_item(&mut self) -> Result<(), String> {
        while self.current_kind() != TokenKind::LBrace {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: expected '{' in item".into());
            }
            self.bump();
        }
        let mut depth = 0i32;
        loop {
            match self.current_kind() {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth -= 1;
                    self.bump();
                    if depth == 0 {
                        return Ok(());
                    }
                    continue;
                }
                TokenKind::Eof => return Err("alpha-onramp: parse error: unbalanced braces".into()),
                _ => {}
            }
            self.bump();
        }
    }

    fn parse_machine_body(&mut self) -> Result<(Vec<Statement>, Vec<Vec<Statement>>), String> {
        self.local_names.clear(); // per-machine locals
        self.state_names.clear();
        while self.current_kind() != TokenKind::LBrace {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: expected machine body '{'".into());
            }
            self.bump();
        }
        self.bump(); // '{'
        self.prescan_state_names()?; // so transitions can name states declared later

        // entry statements come first, then state declarations
        let mut entry = Vec::new();
        while self.current_kind() != TokenKind::RBrace
            && !(self.current_kind() == TokenKind::Ident && self.current_text() == b"state")
        {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated machine body".into());
            }
            entry.push(self.parse_statement()?);
        }
        let mut states = Vec::new();
        while self.current_kind() == TokenKind::Ident && self.current_text() == b"state" {
            states.push(self.parse_state_decl()?);
        }
        if self.current_kind() != TokenKind::RBrace {
            return Err("alpha-onramp: parse error: expected '}' or 'state' in machine body".into());
        }
        self.bump(); // '}'
        Ok((entry, states))
    }

    // Scan from the current position (just inside the body '{') to the matching
    // '}', recording every top-level `state <name>` so transitions can refer to a
    // state declared later in the body.
    fn prescan_state_names(&mut self) -> Result<(), String> {
        let mut scan_position = self.position;
        let mut depth = 1i32;
        while depth > 0 {
            match self.tokens[scan_position].kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => depth -= 1,
                TokenKind::Eof => return Err("alpha-onramp: parse error: unterminated machine body".into()),
                TokenKind::Ident
                    if depth == 1 && token_text(&self.tokens[scan_position], self.source) == b"state" =>
                {
                    if self.tokens[scan_position + 1].kind == TokenKind::Ident {
                        self.state_names
                            .push(token_text(&self.tokens[scan_position + 1], self.source).to_vec());
                    }
                }
                _ => {}
            }
            scan_position += 1;
        }
        Ok(())
    }

    fn parse_state_decl(&mut self) -> Result<Vec<Statement>, String> {
        self.bump(); // "state"
        self.expect(TokenKind::Ident)?; // name (already pre-scanned)
        self.expect(TokenKind::LParen)?;
        while self.current_kind() != TokenKind::RParen {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated state parameter list".into());
            }
            self.bump(); // skip params (e.g. &mut self)
        }
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::LBrace)?;
        let mut statements = Vec::new();
        while self.current_kind() != TokenKind::RBrace {
            if self.current_kind() == TokenKind::Eof {
                return Err("alpha-onramp: parse error: unterminated state body".into());
            }
            statements.push(self.parse_statement()?);
        }
        self.bump(); // '}'
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        if self.current_kind() == TokenKind::Ident && self.current_text() == b"let" {
            self.bump(); // let
            let name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
            self.expect(TokenKind::Colon)?;
            self.expect(TokenKind::Ident)?; // type (ignored at slice 2: everything is i32)
            self.expect(TokenKind::Eq)?;
            let init = self.parse_expression()?;
            if self.current_kind() == TokenKind::Semi {
                self.bump();
            }
            if self.find_local_index(&name).is_some() {
                return Err(format!(
                    "alpha-onramp: local '{}' already declared",
                    String::from_utf8_lossy(&name)
                ));
            }
            let local_index = self.local_names.len();
            self.local_names.push(name);
            return Ok(Statement::Let(local_index, init));
        }
        if self.current_kind() == TokenKind::Ident && self.current_text() == b"transition" {
            self.bump(); // transition
            let subject = self.parse_expression()?;
            self.expect(TokenKind::LBrace)?;
            let mut arms = Vec::new();
            while self.current_kind() != TokenKind::RBrace {
                if self.current_kind() == TokenKind::Eof {
                    return Err("alpha-onramp: parse error: unterminated transition".into());
                }
                let pattern = match self.current_kind() {
                    TokenKind::Int => {
                        let token = self.bump();
                        let text = std::str::from_utf8(token_text(&token, self.source)).unwrap();
                        let value: i64 = text
                            .parse()
                            .map_err(|_| format!("alpha-onramp: bad integer pattern '{}'", text))?;
                        Pattern::Int(value as i32)
                    }
                    TokenKind::Ident => {
                        let token = self.bump();
                        match token_text(&token, self.source) {
                            b"_" => Pattern::Wild,
                            b"true" => Pattern::Int(1),
                            b"false" => Pattern::Int(0),
                            other => {
                                return Err(format!(
                                    "alpha-onramp: unsupported transition pattern '{}'",
                                    String::from_utf8_lossy(other)
                                ))
                            }
                        }
                    }
                    other_kind => {
                        return Err(format!(
                            "alpha-onramp: parse error: expected a transition pattern, found {:?}",
                            other_kind
                        ))
                    }
                };
                self.expect(TokenKind::Arrow)?;
                let target_name = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
                self.expect(TokenKind::LParen)?;
                while self.current_kind() != TokenKind::RParen {
                    if self.current_kind() == TokenKind::Eof {
                        return Err("alpha-onramp: parse error: unterminated transition target".into());
                    }
                    self.bump(); // skip target args (slice 4b: parameterless states)
                }
                self.expect(TokenKind::RParen)?;
                let target = self.find_state_index(&target_name).ok_or_else(|| {
                    format!(
                        "alpha-onramp: transition to unknown state '{}'",
                        String::from_utf8_lossy(&target_name)
                    )
                })?;
                arms.push(TransitionArm { pattern, target });
            }
            self.bump(); // '}'
            if self.current_kind() == TokenKind::Semi {
                self.bump();
            }
            return Ok(Statement::Transition(subject, arms));
        }
        // call statement: path "(" arg ")" — last path segment selects the boundary op
        let mut last_segment = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
        while self.current_kind() == TokenKind::Dot {
            self.bump();
            last_segment = token_text(&self.expect(TokenKind::Ident)?, self.source).to_vec();
        }
        self.expect(TokenKind::LParen)?;
        let statement = match last_segment.as_slice() {
            b"exit_process" => Statement::Exit(self.parse_expression()?),
            b"write_line" => {
                let string_token = self.expect(TokenKind::Str)?;
                let mut bytes = token_text(&string_token, self.source).to_vec();
                bytes.push(b'\n'); // write_line appends a newline
                let string_index = self.strings.len();
                self.strings.push(bytes);
                Statement::WriteLine(string_index)
            }
            other => {
                return Err(format!(
                    "alpha-onramp: unsupported call '{}' (supported: exit_process, write_line)",
                    String::from_utf8_lossy(other)
                ))
            }
        };
        self.expect(TokenKind::RParen)?;
        if self.current_kind() == TokenKind::Semi {
            self.bump();
        }
        Ok(statement)
    }

    fn parse_expression(&mut self) -> Result<usize, String> {
        self.parse_binary(1)
    }

    fn parse_binary(&mut self, min_precedence: u8) -> Result<usize, String> {
        let mut lhs = self.parse_primary()?;
        loop {
            let (op, precedence) = match self.current_kind() {
                TokenKind::Lt => (BinaryOp::Lt, 1u8),
                TokenKind::Gt => (BinaryOp::Gt, 1),
                TokenKind::Le => (BinaryOp::Le, 1),
                TokenKind::Ge => (BinaryOp::Ge, 1),
                TokenKind::EqEq => (BinaryOp::EqEq, 1),
                TokenKind::Ne => (BinaryOp::Ne, 1),
                TokenKind::Plus => (BinaryOp::Add, 2),
                TokenKind::Minus => (BinaryOp::Sub, 2),
                TokenKind::Star => (BinaryOp::Mul, 3),
                TokenKind::Slash => (BinaryOp::Div, 3),
                _ => break,
            };
            if precedence < min_precedence {
                break;
            }
            self.bump(); // operator
            let rhs = self.parse_binary(precedence + 1)?; // left-associative
            lhs = self.add_expression(Expr::Binary(op, lhs, rhs));
        }
        Ok(lhs)
    }

    fn parse_primary(&mut self) -> Result<usize, String> {
        match self.current_kind() {
            TokenKind::Int => {
                let token = self.bump();
                let text = std::str::from_utf8(token_text(&token, self.source)).unwrap();
                let value: i64 = text
                    .parse()
                    .map_err(|_| format!("alpha-onramp: bad integer literal '{}'", text))?;
                if value > i32::MAX as i64 {
                    return Err(format!("alpha-onramp: integer literal {} out of i32 range", value));
                }
                Ok(self.add_expression(Expr::Int(value as i32)))
            }
            TokenKind::Ident => {
                let token = self.bump();
                let name = token_text(&token, self.source).to_vec();
                match self.find_local_index(&name) {
                    Some(local_index) => Ok(self.add_expression(Expr::Local(local_index))),
                    None => Err(format!(
                        "alpha-onramp: unknown identifier '{}'",
                        String::from_utf8_lossy(&name)
                    )),
                }
            }
            TokenKind::LParen => {
                self.bump();
                let expression_node = self.parse_expression()?;
                self.expect(TokenKind::RParen)?;
                Ok(expression_node)
            }
            other_kind => Err(format!(
                "alpha-onramp: parse error: expected an expression, found {:?}",
                other_kind
            )),
        }
    }
}

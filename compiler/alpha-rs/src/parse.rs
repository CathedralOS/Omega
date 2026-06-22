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

use crate::ast::{Arm, BinOp, ExprKind, Main, Pat, Stmt};
use crate::lex::{tok_text, Token, TokKind};

pub struct Parser<'a> {
    toks: &'a [Token],
    src: &'a [u8],
    pos: usize,
    exprs: Vec<ExprKind>,
    local_names: Vec<Vec<u8>>,
    strings: Vec<Vec<u8>>,
    state_names: Vec<Vec<u8>>, // pre-scanned state names of the current machine
}

impl<'a> Parser<'a> {
    pub fn new(toks: &'a [Token], src: &'a [u8]) -> Parser<'a> {
        Parser {
            toks,
            src,
            pos: 0,
            exprs: Vec::new(),
            local_names: Vec::new(),
            strings: Vec::new(),
            state_names: Vec::new(),
        }
    }

    fn state_index(&self, name: &[u8]) -> Option<usize> {
        self.state_names.iter().position(|n| n.as_slice() == name)
    }

    fn kind(&self) -> TokKind {
        self.toks[self.pos].kind
    }
    fn cur_text(&self) -> &[u8] {
        tok_text(&self.toks[self.pos], self.src)
    }
    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos];
        self.pos += 1;
        t
    }
    fn expect(&mut self, k: TokKind) -> Result<Token, String> {
        if self.kind() == k {
            Ok(self.bump())
        } else {
            Err(format!(
                "alpha-onramp: parse error: expected {:?}, found {:?}",
                k,
                self.kind()
            ))
        }
    }
    fn add_expr(&mut self, e: ExprKind) -> usize {
        self.exprs.push(e);
        self.exprs.len() - 1
    }
    fn local_index(&self, name: &[u8]) -> Option<usize> {
        self.local_names.iter().position(|n| n.as_slice() == name)
    }

    pub fn parse_program(&mut self) -> Result<Main, String> {
        let mut machine: Option<(Vec<Stmt>, Vec<Vec<Stmt>>)> = None;
        while self.kind() != TokKind::Eof {
            if self.kind() != TokKind::Ident {
                return Err(format!(
                    "alpha-onramp: parse error: expected a top-level item, found {:?}",
                    self.kind()
                ));
            }
            match self.cur_text() {
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
        let n_locals = self.local_names.len();
        Ok(Main {
            entry,
            states,
            exprs: std::mem::take(&mut self.exprs),
            n_locals,
            strings: std::mem::take(&mut self.strings),
        })
    }

    fn skip_braced_item(&mut self) -> Result<(), String> {
        while self.kind() != TokKind::LBrace {
            if self.kind() == TokKind::Eof {
                return Err("alpha-onramp: parse error: expected '{' in item".into());
            }
            self.bump();
        }
        let mut depth = 0i32;
        loop {
            match self.kind() {
                TokKind::LBrace => depth += 1,
                TokKind::RBrace => {
                    depth -= 1;
                    self.bump();
                    if depth == 0 {
                        return Ok(());
                    }
                    continue;
                }
                TokKind::Eof => return Err("alpha-onramp: parse error: unbalanced braces".into()),
                _ => {}
            }
            self.bump();
        }
    }

    fn parse_machine_body(&mut self) -> Result<(Vec<Stmt>, Vec<Vec<Stmt>>), String> {
        self.local_names.clear(); // per-machine locals
        self.state_names.clear();
        while self.kind() != TokKind::LBrace {
            if self.kind() == TokKind::Eof {
                return Err("alpha-onramp: parse error: expected machine body '{'".into());
            }
            self.bump();
        }
        self.bump(); // '{'
        self.prescan_state_names()?; // so transitions can name states declared later

        // entry statements come first, then state declarations
        let mut entry = Vec::new();
        while self.kind() != TokKind::RBrace
            && !(self.kind() == TokKind::Ident && self.cur_text() == b"state")
        {
            if self.kind() == TokKind::Eof {
                return Err("alpha-onramp: parse error: unterminated machine body".into());
            }
            entry.push(self.parse_stmt()?);
        }
        let mut states = Vec::new();
        while self.kind() == TokKind::Ident && self.cur_text() == b"state" {
            states.push(self.parse_state_decl()?);
        }
        if self.kind() != TokKind::RBrace {
            return Err("alpha-onramp: parse error: expected '}' or 'state' in machine body".into());
        }
        self.bump(); // '}'
        Ok((entry, states))
    }

    // Scan from the current position (just inside the body '{') to the matching
    // '}', recording every top-level `state <name>` so transitions can refer to a
    // state declared later in the body.
    fn prescan_state_names(&mut self) -> Result<(), String> {
        let mut p = self.pos;
        let mut depth = 1i32;
        while depth > 0 {
            match self.toks[p].kind {
                TokKind::LBrace => depth += 1,
                TokKind::RBrace => depth -= 1,
                TokKind::Eof => return Err("alpha-onramp: parse error: unterminated machine body".into()),
                TokKind::Ident
                    if depth == 1 && tok_text(&self.toks[p], self.src) == b"state" =>
                {
                    if self.toks[p + 1].kind == TokKind::Ident {
                        self.state_names
                            .push(tok_text(&self.toks[p + 1], self.src).to_vec());
                    }
                }
                _ => {}
            }
            p += 1;
        }
        Ok(())
    }

    fn parse_state_decl(&mut self) -> Result<Vec<Stmt>, String> {
        self.bump(); // "state"
        self.expect(TokKind::Ident)?; // name (already pre-scanned)
        self.expect(TokKind::LParen)?;
        while self.kind() != TokKind::RParen {
            if self.kind() == TokKind::Eof {
                return Err("alpha-onramp: parse error: unterminated state parameter list".into());
            }
            self.bump(); // skip params (e.g. &mut self)
        }
        self.expect(TokKind::RParen)?;
        self.expect(TokKind::LBrace)?;
        let mut stmts = Vec::new();
        while self.kind() != TokKind::RBrace {
            if self.kind() == TokKind::Eof {
                return Err("alpha-onramp: parse error: unterminated state body".into());
            }
            stmts.push(self.parse_stmt()?);
        }
        self.bump(); // '}'
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        if self.kind() == TokKind::Ident && self.cur_text() == b"let" {
            self.bump(); // let
            let name = tok_text(&self.expect(TokKind::Ident)?, self.src).to_vec();
            self.expect(TokKind::Colon)?;
            self.expect(TokKind::Ident)?; // type (ignored at slice 2: everything is i32)
            self.expect(TokKind::Eq)?;
            let init = self.parse_expr()?;
            if self.kind() == TokKind::Semi {
                self.bump();
            }
            if self.local_index(&name).is_some() {
                return Err(format!(
                    "alpha-onramp: local '{}' already declared",
                    String::from_utf8_lossy(&name)
                ));
            }
            let idx = self.local_names.len();
            self.local_names.push(name);
            return Ok(Stmt::Let(idx, init));
        }
        if self.kind() == TokKind::Ident && self.cur_text() == b"transition" {
            self.bump(); // transition
            let subject = self.parse_expr()?;
            self.expect(TokKind::LBrace)?;
            let mut arms = Vec::new();
            while self.kind() != TokKind::RBrace {
                if self.kind() == TokKind::Eof {
                    return Err("alpha-onramp: parse error: unterminated transition".into());
                }
                let pat = match self.kind() {
                    TokKind::Int => {
                        let t = self.bump();
                        let text = std::str::from_utf8(tok_text(&t, self.src)).unwrap();
                        let v: i64 = text
                            .parse()
                            .map_err(|_| format!("alpha-onramp: bad integer pattern '{}'", text))?;
                        Pat::Int(v as i32)
                    }
                    TokKind::Ident => {
                        let t = self.bump();
                        match tok_text(&t, self.src) {
                            b"_" => Pat::Wild,
                            b"true" => Pat::Int(1),
                            b"false" => Pat::Int(0),
                            other => {
                                return Err(format!(
                                    "alpha-onramp: unsupported transition pattern '{}'",
                                    String::from_utf8_lossy(other)
                                ))
                            }
                        }
                    }
                    k => {
                        return Err(format!(
                            "alpha-onramp: parse error: expected a transition pattern, found {:?}",
                            k
                        ))
                    }
                };
                self.expect(TokKind::Arrow)?;
                let target_name = tok_text(&self.expect(TokKind::Ident)?, self.src).to_vec();
                self.expect(TokKind::LParen)?;
                while self.kind() != TokKind::RParen {
                    if self.kind() == TokKind::Eof {
                        return Err("alpha-onramp: parse error: unterminated transition target".into());
                    }
                    self.bump(); // skip target args (slice 4b: parameterless states)
                }
                self.expect(TokKind::RParen)?;
                let target = self.state_index(&target_name).ok_or_else(|| {
                    format!(
                        "alpha-onramp: transition to unknown state '{}'",
                        String::from_utf8_lossy(&target_name)
                    )
                })?;
                arms.push(Arm { pat, target });
            }
            self.bump(); // '}'
            if self.kind() == TokKind::Semi {
                self.bump();
            }
            return Ok(Stmt::Transition(subject, arms));
        }
        // call statement: path "(" arg ")" — last path segment selects the boundary op
        let mut last = tok_text(&self.expect(TokKind::Ident)?, self.src).to_vec();
        while self.kind() == TokKind::Dot {
            self.bump();
            last = tok_text(&self.expect(TokKind::Ident)?, self.src).to_vec();
        }
        self.expect(TokKind::LParen)?;
        let stmt = match last.as_slice() {
            b"exit_process" => Stmt::Exit(self.parse_expr()?),
            b"write_line" => {
                let s = self.expect(TokKind::Str)?;
                let mut bytes = tok_text(&s, self.src).to_vec();
                bytes.push(b'\n'); // write_line appends a newline
                let idx = self.strings.len();
                self.strings.push(bytes);
                Stmt::WriteLine(idx)
            }
            other => {
                return Err(format!(
                    "alpha-onramp: unsupported call '{}' (supported: exit_process, write_line)",
                    String::from_utf8_lossy(other)
                ))
            }
        };
        self.expect(TokKind::RParen)?;
        if self.kind() == TokKind::Semi {
            self.bump();
        }
        Ok(stmt)
    }

    fn parse_expr(&mut self) -> Result<usize, String> {
        self.parse_binary(1)
    }

    fn parse_binary(&mut self, min_prec: u8) -> Result<usize, String> {
        let mut lhs = self.parse_primary()?;
        loop {
            let (op, prec) = match self.kind() {
                TokKind::Lt => (BinOp::Lt, 1u8),
                TokKind::Gt => (BinOp::Gt, 1),
                TokKind::Le => (BinOp::Le, 1),
                TokKind::Ge => (BinOp::Ge, 1),
                TokKind::EqEq => (BinOp::EqEq, 1),
                TokKind::Ne => (BinOp::Ne, 1),
                TokKind::Plus => (BinOp::Add, 2),
                TokKind::Minus => (BinOp::Sub, 2),
                TokKind::Star => (BinOp::Mul, 3),
                TokKind::Slash => (BinOp::Div, 3),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            self.bump(); // operator
            let rhs = self.parse_binary(prec + 1)?; // left-associative
            lhs = self.add_expr(ExprKind::Bin(op, lhs, rhs));
        }
        Ok(lhs)
    }

    fn parse_primary(&mut self) -> Result<usize, String> {
        match self.kind() {
            TokKind::Int => {
                let t = self.bump();
                let text = std::str::from_utf8(tok_text(&t, self.src)).unwrap();
                let v: i64 = text
                    .parse()
                    .map_err(|_| format!("alpha-onramp: bad integer literal '{}'", text))?;
                if v > i32::MAX as i64 {
                    return Err(format!("alpha-onramp: integer literal {} out of i32 range", v));
                }
                Ok(self.add_expr(ExprKind::Int(v as i32)))
            }
            TokKind::Ident => {
                let t = self.bump();
                let name = tok_text(&t, self.src).to_vec();
                match self.local_index(&name) {
                    Some(i) => Ok(self.add_expr(ExprKind::Local(i))),
                    None => Err(format!(
                        "alpha-onramp: unknown identifier '{}'",
                        String::from_utf8_lossy(&name)
                    )),
                }
            }
            TokKind::LParen => {
                self.bump();
                let e = self.parse_expr()?;
                self.expect(TokKind::RParen)?;
                Ok(e)
            }
            k => Err(format!(
                "alpha-onramp: parse error: expected an expression, found {:?}",
                k
            )),
        }
    }
}

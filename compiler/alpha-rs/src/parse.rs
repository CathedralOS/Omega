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

use crate::ast::{BinOp, ExprKind, Main, Stmt};
use crate::lex::{tok_text, Token, TokKind};

pub struct Parser<'a> {
    toks: &'a [Token],
    src: &'a [u8],
    pos: usize,
    exprs: Vec<ExprKind>,
    local_names: Vec<Vec<u8>>,
}

impl<'a> Parser<'a> {
    pub fn new(toks: &'a [Token], src: &'a [u8]) -> Parser<'a> {
        Parser {
            toks,
            src,
            pos: 0,
            exprs: Vec::new(),
            local_names: Vec::new(),
        }
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
        let mut entry: Option<Vec<Stmt>> = None;
        while self.kind() != TokKind::Eof {
            if self.kind() != TokKind::Ident {
                return Err(format!(
                    "alpha-onramp: parse error: expected a top-level item, found {:?}",
                    self.kind()
                ));
            }
            match self.cur_text() {
                b"boundary" | b"data" => self.skip_braced_item()?,
                b"machine" => entry = Some(self.parse_machine_body()?),
                other => {
                    return Err(format!(
                        "alpha-onramp: unsupported top-level keyword '{}' (Alpha subset)",
                        String::from_utf8_lossy(other)
                    ))
                }
            }
        }
        let stmts = entry.ok_or_else(|| "alpha-onramp: no machine to compile".to_string())?;
        let n_locals = self.local_names.len();
        Ok(Main {
            stmts,
            exprs: std::mem::take(&mut self.exprs),
            n_locals,
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

    fn parse_machine_body(&mut self) -> Result<Vec<Stmt>, String> {
        self.local_names.clear(); // per-machine locals (slice 2: one entry machine)
        while self.kind() != TokKind::LBrace {
            if self.kind() == TokKind::Eof {
                return Err("alpha-onramp: parse error: expected machine body '{'".into());
            }
            self.bump();
        }
        self.bump(); // '{'
        let mut stmts = Vec::new();
        while self.kind() != TokKind::RBrace {
            if self.kind() == TokKind::Eof {
                return Err("alpha-onramp: parse error: unterminated machine body".into());
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
        // call statement: path "(" expr ")" — path's last segment must be exit_process
        let mut last = tok_text(&self.expect(TokKind::Ident)?, self.src).to_vec();
        while self.kind() == TokKind::Dot {
            self.bump();
            last = tok_text(&self.expect(TokKind::Ident)?, self.src).to_vec();
        }
        if last != b"exit_process" {
            return Err(format!(
                "alpha-onramp: unsupported call '{}' (slice 2 supports exit_process only)",
                String::from_utf8_lossy(&last)
            ));
        }
        self.expect(TokKind::LParen)?;
        let arg = self.parse_expr()?;
        self.expect(TokKind::RParen)?;
        if self.kind() == TokKind::Semi {
            self.bump();
        }
        Ok(Stmt::Exit(arg))
    }

    fn parse_expr(&mut self) -> Result<usize, String> {
        self.parse_binary(1)
    }

    fn parse_binary(&mut self, min_prec: u8) -> Result<usize, String> {
        let mut lhs = self.parse_primary()?;
        loop {
            let (op, prec) = match self.kind() {
                TokKind::Plus => (BinOp::Add, 1u8),
                TokKind::Minus => (BinOp::Sub, 1),
                TokKind::Star => (BinOp::Mul, 2),
                TokKind::Slash => (BinOp::Div, 2),
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

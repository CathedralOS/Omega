// beta-lang — the Beta-language compiler, throwaway Rust on-ramp.
//
// Reads Beta source on stdin, writes Alpha assembly on stdout. The existing
// assembler (../beta) lowers that to a tape; the seed runs it. This crate exists
// only to discover + pin the Beta language ergonomically; the trusted Beta
// compiler is later transcribed into Alpha assembly and this is discarded (exactly
// as alpha-rs/beta-rs are throwaway). Deliberately dumb, index/arena-based,
// monomorphic Rust so the eventual port to assembly is mechanical.
//
//   SLICE 1: `proc main() { return <integer expression> }`.
//   SLICE 2: multiple procedures with parameters and calls (the calling
//            convention). `proc f(a, b) { return a + g(b) }`, etc. Body is still a
//            single `return <expr>`; `if`/`while`/`let` arrive in slice 3.
//
// Calling convention (see ../beta/CALLING_CONVENTION.md): control returns ride the
// VM's hidden call/ret stack; data rides an explicit stack via r15 (sp). r14 is the
// frame pointer (fp): params/locals are addressed at a stable [fp - 8 - 8*slot]
// while sp moves freely for expression temporaries. Args in r0..r3, result in r0.

use std::io::Read;

#[derive(Clone, PartialEq)]
enum Tok {
    Proc,
    Return,
    Ident(String),
    Int(i64),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eof,
}

fn lex(src: &str) -> Vec<Tok> {
    let bytes = src.as_bytes();
    let mut i = 0;
    let mut toks = Vec::new();
    while i < bytes.len() {
        let c = bytes[i];
        if c == b';' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            toks.push(Tok::Int(src[start..i].parse().unwrap()));
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            toks.push(match &src[start..i] {
                "proc" => Tok::Proc,
                "return" => Tok::Return,
                word => Tok::Ident(word.to_string()),
            });
            continue;
        }
        toks.push(match c {
            b'(' => Tok::LParen,
            b')' => Tok::RParen,
            b'{' => Tok::LBrace,
            b'}' => Tok::RBrace,
            b',' => Tok::Comma,
            b'+' => Tok::Plus,
            b'-' => Tok::Minus,
            b'*' => Tok::Star,
            b'/' => Tok::Slash,
            b'%' => Tok::Percent,
            other => panic!("beta-lang: unexpected character '{}'", other as char),
        });
        i += 1;
    }
    toks.push(Tok::Eof);
    toks
}

// Expression nodes live in a flat arena; children are indices, not boxes, so the
// assembly port (which has no recursion) is a mechanical worklist walk.
enum Node {
    Int(i64),
    Var(usize),               // parameter slot
    Bin(u8, usize, usize),    // operator byte, left index, right index
    Call(String, Vec<usize>), // callee name, argument node indices
}

struct Proc {
    name: String,
    nparams: usize,
    body: usize, // expression node index (the `return` value)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    nodes: Vec<Node>,
    params: Vec<String>, // the current procedure's parameters, for Var resolution
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }
    fn next(&mut self) -> Tok {
        let t = self.toks[self.pos].clone();
        self.pos += 1;
        t
    }
    fn eat(&mut self, t: Tok) {
        if self.next() != t {
            panic!("beta-lang: parse error near token {}", self.pos);
        }
    }
    fn push(&mut self, n: Node) -> usize {
        self.nodes.push(n);
        self.nodes.len() - 1
    }

    fn proc(&mut self) -> Proc {
        self.eat(Tok::Proc);
        let name = match self.next() {
            Tok::Ident(s) => s,
            _ => panic!("beta-lang: expected a procedure name after `proc`"),
        };
        self.eat(Tok::LParen);
        let mut params = Vec::new();
        if *self.peek() != Tok::RParen {
            loop {
                match self.next() {
                    Tok::Ident(s) => params.push(s),
                    _ => panic!("beta-lang: expected a parameter name"),
                }
                if *self.peek() == Tok::Comma {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        self.eat(Tok::RParen);
        if params.len() > 4 {
            panic!("beta-lang: >4 parameters not supported yet (stack-passed args deferred)");
        }
        self.eat(Tok::LBrace);
        self.eat(Tok::Return);
        self.params = params.clone();
        let body = self.expr();
        self.eat(Tok::RBrace);
        Proc {
            name,
            nparams: params.len(),
            body,
        }
    }

    fn expr(&mut self) -> usize {
        self.sum()
    }
    fn sum(&mut self) -> usize {
        let mut left = self.term();
        loop {
            let op = match self.peek() {
                Tok::Plus => b'+',
                Tok::Minus => b'-',
                _ => break,
            };
            self.pos += 1;
            let right = self.term();
            left = self.push(Node::Bin(op, left, right));
        }
        left
    }
    fn term(&mut self) -> usize {
        let mut left = self.factor();
        loop {
            let op = match self.peek() {
                Tok::Star => b'*',
                Tok::Slash => b'/',
                Tok::Percent => b'%',
                _ => break,
            };
            self.pos += 1;
            let right = self.factor();
            left = self.push(Node::Bin(op, left, right));
        }
        left
    }
    fn factor(&mut self) -> usize {
        match self.next() {
            Tok::Int(n) => self.push(Node::Int(n)),
            Tok::LParen => {
                let e = self.expr();
                self.eat(Tok::RParen);
                e
            }
            Tok::Ident(name) => {
                if *self.peek() == Tok::LParen {
                    self.eat(Tok::LParen);
                    let mut args = Vec::new();
                    if *self.peek() != Tok::RParen {
                        loop {
                            args.push(self.expr());
                            if *self.peek() == Tok::Comma {
                                self.pos += 1;
                            } else {
                                break;
                            }
                        }
                    }
                    self.eat(Tok::RParen);
                    self.push(Node::Call(name, args))
                } else {
                    let slot = self
                        .params
                        .iter()
                        .position(|p| *p == name)
                        .unwrap_or_else(|| panic!("beta-lang: unknown name '{}'", name));
                    self.push(Node::Var(slot))
                }
            }
            _ => panic!("beta-lang: expected an expression"),
        }
    }
}

// push r0 onto the data stack / pop the data stack top into rN.
fn push_r0(out: &mut String) {
    out.push_str("        imm   r2, 8\n        sub   r15, r2\n        store r15, r0\n");
}
fn pop_into(out: &mut String, reg: usize) {
    out.push_str(&format!(
        "        load  r{}, r15\n        imm   r5, 8\n        add   r15, r5\n",
        reg
    ));
}

// Evaluate an expression, leaving the result in r0. Left operands of binary ops and
// in-flight call arguments are held on the data stack, so nothing is assumed to
// survive a call in a register — params live in the frame, addressed via fp (r14).
fn gen(nodes: &[Node], root: usize, out: &mut String) {
    match &nodes[root] {
        Node::Int(n) => out.push_str(&format!("        imm   r0, {}\n", n)),
        Node::Var(slot) => {
            // r0 = mem[fp - 8 - 8*slot]
            out.push_str("        mov   r0, r14\n");
            out.push_str(&format!("        imm   r1, {}\n", 8 + 8 * slot));
            out.push_str("        sub   r0, r1\n        load  r0, r0\n");
        }
        Node::Bin(op, left, right) => {
            gen(nodes, *left, out);
            push_r0(out); // save left
            gen(nodes, *right, out);
            out.push_str("        mov   r1, r0\n"); // r1 = right
            pop_into(out, 0); // r0 = left
            let mnem = match op {
                b'+' => "add",
                b'-' => "sub",
                b'*' => "mul",
                b'/' => "div",
                b'%' => "mod",
                _ => unreachable!(),
            };
            out.push_str(&format!("        {}   r0, r1\n", mnem));
        }
        Node::Call(name, args) => {
            for &a in args {
                gen(nodes, a, out);
                push_r0(out); // stage each argument on the data stack (left to right)
            }
            for k in (0..args.len()).rev() {
                pop_into(out, k); // unstage into r0..r(n-1)
            }
            out.push_str(&format!("        call  {}\n", name));
        }
    }
}

fn emit_proc(p: &Proc, nodes: &[Node], out: &mut String) {
    out.push_str(&format!("{}:\n", p.name));
    // prologue: push caller fp, fp = sp, allocate the frame, store params into slots
    out.push_str("        imm   r2, 8\n        sub   r15, r2\n        store r15, r14\n");
    out.push_str("        mov   r14, r15\n");
    let frame = p.nparams * 8;
    if frame > 0 {
        out.push_str(&format!("        imm   r2, {}\n        sub   r15, r2\n", frame));
    }
    for k in 0..p.nparams {
        // mem[fp - 8 - 8*k] = arg k  (scratch r4/r5, never an argument register)
        out.push_str("        mov   r5, r14\n");
        out.push_str(&format!("        imm   r4, {}\n", 8 + 8 * k));
        out.push_str(&format!("        sub   r5, r4\n        store r5, r{}\n", k));
    }
    // body: `return expr`
    gen(nodes, p.body, out);
    // epilogue: sp = fp, restore caller fp, ret
    out.push_str("        mov   r15, r14\n        load  r14, r15\n");
    out.push_str("        imm   r2, 8\n        add   r15, r2\n        ret\n");
}

fn main() {
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).unwrap();

    let mut p = Parser {
        toks: lex(&src),
        pos: 0,
        nodes: Vec::new(),
        params: Vec::new(),
    };
    let mut procs = Vec::new();
    while *p.peek() != Tok::Eof {
        procs.push(p.proc());
    }
    if !procs.iter().any(|q| q.name == "main") {
        panic!("beta-lang: no `proc main`");
    }

    let mut out = String::new();
    out.push_str("; generated by beta-lang (Beta -> Alpha assembly)\n");
    out.push_str("        imm   r15, 1048576      ; data stack pointer (sp)\n");
    out.push_str("        imm   r14, 1048576      ; frame pointer (fp)\n");
    out.push_str("        call  main\n");
    out.push_str("        halt  r0\n");
    for q in &procs {
        emit_proc(q, &p.nodes, &mut out);
    }
    print!("{}", out);
}

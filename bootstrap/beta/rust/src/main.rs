// beta-lang — the Beta-language compiler, throwaway Rust on-ramp.
//
// Reads Beta source on stdin, writes Alpha assembly on stdout. The assembler
// The canonical Alpha assembler lowers that to a tape; the seed runs it. This crate
// only helped discover and pin the Beta language ergonomically. The Beta-written
// compiler now self-hosts, so this Rust producer is retained only as optional
// cold-start/reference tooling.
// Deliberately dumb, index/arena-based, monomorphic Rust so the port is mechanical.
//
//   SLICE 1: `proc main() { return <arith> }`.
//   SLICE 2: procedures with parameters and calls (the calling convention).
//   SLICE 3: `let` locals, assignment, comparisons, CFG control flow (`state`
//            blocks + guarded `to … when …` transitions — no if/while; loops are
//            self-transitioning states)  ->  unlocks recursion + loops.
//
// Convention (see bootstrap/beta/CALLING_CONVENTION.md): control returns ride the VM's
// hidden call/ret stack; data rides an explicit stack via r15 (sp), r14 = frame
// pointer (fp). Params + locals are frame slots at [fp - 8 - 8*slot]; args in
// r0..r3, result in r0. Locals are function-scoped (no block scoping yet).

use std::io::Read;

#[derive(Clone, PartialEq)]
enum Tok {
    Proc,
    Return,
    State, // CFG / Omega-style control flow: a basic block
    To,    // a transition (jump) to another state
    When,  // a guard on a transition
    Let,
    Ident(String),
    Int(i64),
    Str(Vec<u8>), // string literal payload (decoded bytes), only as emit("...")
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Assign, // =
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    Ne,
    Byte,
    Word,
    LBracket,
    RBracket,
    Eof,
}

fn lex(src: &str) -> Vec<Tok> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut toks = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c == b';' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let s = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            toks.push(Tok::Int(src[s..i].parse().unwrap()));
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            toks.push(match &src[s..i] {
                "proc" => Tok::Proc,
                "return" => Tok::Return,
                "state" => Tok::State,
                "to" => Tok::To,
                "when" => Tok::When,
                "let" => Tok::Let,
                "byte" => Tok::Byte,
                "word" => Tok::Word,
                w => Tok::Ident(w.to_string()),
            });
            continue;
        }
        if c == b'"' {
            // string literal: only valid as emit("..."); decoded to bytes here.
            i += 1;
            let mut bytes = Vec::new();
            loop {
                if i >= b.len() {
                    panic!("beta-lang: unterminated string literal");
                }
                if b[i] == b'"' {
                    i += 1;
                    break;
                }
                if b[i] == b'\\' {
                    i += 1;
                    bytes.push(match b.get(i) {
                        Some(b'n') => 10,
                        Some(b't') => 9,
                        Some(b'r') => 13,
                        Some(b'0') => 0,
                        Some(b'\\') => 92,
                        Some(b'"') => 34,
                        other => panic!(
                            "beta-lang: unknown escape '\\{}'",
                            other.map(|c| *c as char).unwrap_or('?')
                        ),
                    });
                    i += 1;
                } else {
                    bytes.push(b[i]);
                    i += 1;
                }
            }
            toks.push(Tok::Str(bytes));
            continue;
        }
        if c == b'\'' {
            // char literal: 'x' or '\n' '\t' '\r' '\0' '\\' '\'' -> the byte value
            i += 1;
            if i >= b.len() {
                panic!("beta-lang: unterminated char literal");
            }
            let v: i64 = if b[i] == b'\\' {
                i += 1;
                match b.get(i) {
                    Some(b'n') => 10,
                    Some(b't') => 9,
                    Some(b'r') => 13,
                    Some(b'0') => 0,
                    Some(b'\\') => 92,
                    Some(b'\'') => 39,
                    other => panic!(
                        "beta-lang: unknown escape '\\{}'",
                        other.map(|c| *c as char).unwrap_or('?')
                    ),
                }
            } else {
                b[i] as i64
            };
            i += 1;
            if i >= b.len() || b[i] != b'\'' {
                panic!("beta-lang: unterminated char literal");
            }
            i += 1;
            toks.push(Tok::Int(v));
            continue;
        }
        let c2 = if i + 1 < b.len() { b[i + 1] } else { 0 };
        let (tok, adv) = match c {
            b'(' => (Tok::LParen, 1),
            b')' => (Tok::RParen, 1),
            b'{' => (Tok::LBrace, 1),
            b'}' => (Tok::RBrace, 1),
            b',' => (Tok::Comma, 1),
            b'[' => (Tok::LBracket, 1),
            b']' => (Tok::RBracket, 1),
            b'+' => (Tok::Plus, 1),
            b'-' => (Tok::Minus, 1),
            b'*' => (Tok::Star, 1),
            b'/' => (Tok::Slash, 1),
            b'%' => (Tok::Percent, 1),
            b'=' if c2 == b'=' => (Tok::EqEq, 2),
            b'=' => (Tok::Assign, 1),
            b'<' if c2 == b'=' => (Tok::Le, 2),
            b'<' => (Tok::Lt, 1),
            b'>' if c2 == b'=' => (Tok::Ge, 2),
            b'>' => (Tok::Gt, 1),
            b'!' if c2 == b'=' => (Tok::Ne, 2),
            other => panic!("beta-lang: unexpected character '{}'", other as char),
        };
        toks.push(tok);
        i += adv;
    }
    toks.push(Tok::Eof);
    toks
}

// Expressions live in a flat arena (children are indices).
enum Node {
    Int(i64),
    Var(usize),               // frame slot
    Bin(u8, usize, usize),    // + - * / %
    Cmp(u8, usize, usize),    // 0=< 1=> 2=== 3=!= 4=<= 5=>=  (yields 0/1)
    Call(String, Vec<usize>), // callee, argument indices
    Load(u8, usize),          // width (1=byte, 8=word), address expr
    Emit(Vec<u8>),            // emit("...") -> write each byte; no string type
}

enum Stmt {
    Let(usize, usize),                // slot, expr
    Assign(usize, usize),             // slot, expr
    Return(usize),                    // expr
    State(String, Vec<Stmt>),         // a CFG basic block: a label + its straight-line body
    Goto(String),                     // `to NAME`         — unconditional transition
    GotoWhen(usize, String),          // `to NAME when E`  — guarded transition
    Store(u8, usize, usize),          // width, address, value
    CallStmt(usize),                  // call expr evaluated for effect (result discarded)
}

struct Proc {
    name: String,
    nparams: usize,
    nslots: usize, // params + locals
    body: Vec<Stmt>,
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    nodes: Vec<Node>,
    names: Vec<String>, // current proc's params + locals; slot = index
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
    fn ident(&mut self) -> String {
        match self.next() {
            Tok::Ident(s) => s,
            _ => panic!("beta-lang: expected an identifier"),
        }
    }
    fn slot_of(&self, name: &str) -> usize {
        self.names
            .iter()
            .position(|n| n == name)
            .unwrap_or_else(|| panic!("beta-lang: unknown name '{}'", name))
    }
    fn declare(&mut self, name: String) -> usize {
        if self.names.iter().any(|n| *n == name) {
            panic!("beta-lang: '{}' already declared in this procedure", name);
        }
        self.names.push(name);
        self.names.len() - 1
    }

    fn program(&mut self) -> Vec<Proc> {
        let mut procs = Vec::new();
        while *self.peek() != Tok::Eof {
            procs.push(self.proc());
        }
        procs
    }
    fn proc(&mut self) -> Proc {
        self.eat(Tok::Proc);
        let name = self.ident();
        self.eat(Tok::LParen);
        self.names.clear();
        if *self.peek() != Tok::RParen {
            loop {
                let p = self.ident();
                self.declare(p);
                if *self.peek() == Tok::Comma {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        self.eat(Tok::RParen);
        let nparams = self.names.len();
        if nparams > 4 {
            panic!("beta-lang: >4 parameters not supported yet");
        }
        let body = self.block();
        Proc {
            name,
            nparams,
            nslots: self.names.len(),
            body,
        }
    }
    fn block(&mut self) -> Vec<Stmt> {
        self.eat(Tok::LBrace);
        let mut stmts = Vec::new();
        while *self.peek() != Tok::RBrace {
            stmts.push(self.stmt());
        }
        self.eat(Tok::RBrace);
        stmts
    }
    fn stmt(&mut self) -> Stmt {
        match self.peek() {
            Tok::Let => {
                self.pos += 1;
                let name = self.ident();
                self.eat(Tok::Assign);
                let e = self.expr();
                let slot = self.declare(name);
                Stmt::Let(slot, e)
            }
            Tok::Return => {
                self.pos += 1;
                Stmt::Return(self.expr())
            }
            Tok::State => {
                // `state NAME { stmts }` — a basic block in CFG/Omega style.
                self.pos += 1;
                let name = self.ident();
                let body = self.block();
                Stmt::State(name, body)
            }
            Tok::To => {
                // `to NAME` (unconditional) or `to NAME when COND` (guarded transition).
                self.pos += 1;
                let name = self.ident();
                if *self.peek() == Tok::When {
                    self.pos += 1;
                    Stmt::GotoWhen(self.expr(), name)
                } else {
                    Stmt::Goto(name)
                }
            }
            Tok::Byte | Tok::Word => {
                let w = if *self.peek() == Tok::Byte { 1 } else { 8 };
                self.pos += 1;
                self.eat(Tok::LBracket);
                let addr = self.expr();
                self.eat(Tok::RBracket);
                self.eat(Tok::Assign);
                let val = self.expr();
                Stmt::Store(w, addr, val)
            }
            Tok::Ident(_) => {
                // `f(args)` -> call for effect; `x = e` -> assignment.
                if self.toks[self.pos + 1] == Tok::LParen {
                    let e = self.expr(); // factor() parses the whole call
                    Stmt::CallStmt(e)
                } else {
                    let name = self.ident();
                    self.eat(Tok::Assign);
                    let e = self.expr();
                    Stmt::Assign(self.slot_of(&name), e)
                }
            }
            _ => panic!("beta-lang: expected a statement"),
        }
    }

    fn expr(&mut self) -> usize {
        let left = self.sum();
        let code = match self.peek() {
            Tok::Lt => 0,
            Tok::Gt => 1,
            Tok::EqEq => 2,
            Tok::Ne => 3,
            Tok::Le => 4,
            Tok::Ge => 5,
            _ => return left,
        };
        self.pos += 1;
        let right = self.sum();
        self.push(Node::Cmp(code, left, right))
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
            Tok::Byte => {
                self.eat(Tok::LBracket);
                let a = self.expr();
                self.eat(Tok::RBracket);
                self.push(Node::Load(1, a))
            }
            Tok::Word => {
                self.eat(Tok::LBracket);
                let a = self.expr();
                self.eat(Tok::RBracket);
                self.push(Node::Load(8, a))
            }
            Tok::Ident(name) => {
                if name == "emit" {
                    // emit("literal") — the sole place a string literal is valid.
                    self.eat(Tok::LParen);
                    let bytes = match self.next() {
                        Tok::Str(b) => b,
                        _ => panic!("beta-lang: emit(...) takes a string literal"),
                    };
                    self.eat(Tok::RParen);
                    return self.push(Node::Emit(bytes));
                }
                if *self.peek() == Tok::LParen {
                    self.pos += 1;
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
                    self.push(Node::Var(self.slot_of(&name)))
                }
            }
            _ => panic!("beta-lang: expected an expression"),
        }
    }
}

struct Gen<'a> {
    nodes: &'a [Node],
    out: String,
    label: usize,
    strings: Vec<Vec<u8>>, // deduped pool of emit() literals -> a `db` data section
    cur_proc: String,      // for naming CFG state labels `<proc>__<state>`
}

impl<'a> Gen<'a> {
    fn line(&mut self, s: &str) {
        self.out.push_str("        ");
        self.out.push_str(s);
        self.out.push('\n');
    }
    fn at(&mut self, label: &str) {
        self.out.push_str(label);
        self.out.push_str(":\n");
    }
    fn newlabel(&mut self) -> String {
        let l = format!("_L{}", self.label);
        self.label += 1;
        l
    }
    fn push_r0(&mut self) {
        self.line("imm   r2, 8");
        self.line("sub   r15, r2");
        self.line("store r15, r0");
    }
    fn pop_into(&mut self, reg: usize) {
        self.line(&format!("load  r{}, r15", reg));
        self.line("imm   r5, 8");
        self.line("add   r15, r5");
    }
    // address of frame slot -> reg (clobbers reg + r-scratch); value untouched in r0.
    fn slot_addr(&mut self, reg: usize, scratch: usize, slot: usize) {
        self.line(&format!("mov   r{}, r14", reg));
        self.line(&format!("imm   r{}, {}", scratch, 8 + 8 * slot));
        self.line(&format!("sub   r{}, r{}", reg, scratch));
    }

    fn expr(&mut self, root: usize) {
        match &self.nodes[root] {
            Node::Int(n) => self.line(&format!("imm   r0, {}", n)),
            Node::Var(slot) => {
                let slot = *slot;
                self.slot_addr(0, 1, slot); // r0 = &slot
                self.line("load  r0, r0");
            }
            Node::Bin(op, l, r) => {
                let (op, l, r) = (*op, *l, *r);
                self.expr(l);
                self.push_r0();
                self.expr(r);
                self.line("mov   r1, r0");
                self.pop_into(0);
                let m = match op {
                    b'+' => "add",
                    b'-' => "sub",
                    b'*' => "mul",
                    b'/' => "div",
                    b'%' => "mod",
                    _ => unreachable!(),
                };
                self.line(&format!("{}   r0, r1", m));
            }
            Node::Cmp(code, l, r) => {
                let (code, l, r) = (*code, *l, *r);
                self.expr(l);
                self.push_r0();
                self.expr(r);
                self.line("mov   r1, r0");
                self.pop_into(0); // r0 = left, r1 = right
                let set = self.newlabel();
                let done = self.newlabel();
                // branch to `set`; for 0/1/2 the branch means TRUE, for 3/4/5 it means FALSE.
                match code {
                    0 => self.line(&format!("jlt   r0, r1, {}", set)), // <
                    1 => self.line(&format!("jlt   r1, r0, {}", set)), // >
                    2 => self.line(&format!("jeq   r0, r1, {}", set)), // ==
                    3 => self.line(&format!("jeq   r0, r1, {}", set)), // != (branch=equal=false)
                    4 => self.line(&format!("jlt   r1, r0, {}", set)), // <= (branch=b<a=false)
                    5 => self.line(&format!("jlt   r0, r1, {}", set)), // >= (branch=a<b=false)
                    _ => unreachable!(),
                }
                let (fall, hit) = if code < 3 { (0, 1) } else { (1, 0) };
                self.line(&format!("imm   r0, {}", fall));
                self.line(&format!("jmp   {}", done));
                self.at(&set);
                self.line(&format!("imm   r0, {}", hit));
                self.at(&done);
            }
            Node::Call(name, args) => {
                let name = name.clone();
                let args = args.clone();
                // Built-in host-boundary intrinsics (the only path to Alpha read/write).
                if name == "read_byte" {
                    assert!(args.is_empty(), "beta-lang: read_byte() takes no arguments");
                    self.line("read  r0"); // -> byte in r0, or -1 at EOF
                    return;
                }
                if name == "write_byte" {
                    assert!(args.len() == 1, "beta-lang: write_byte(x) takes one argument");
                    self.expr(args[0]);
                    self.line("write r0"); // write low byte of r0
                    return;
                }
                for &a in &args {
                    self.expr(a);
                    self.push_r0(); // stage args left-to-right
                }
                for k in (0..args.len()).rev() {
                    self.pop_into(k); // unstage into r0..r(n-1)
                }
                self.line(&format!("call  {}", name));
            }
            Node::Load(w, addr) => {
                let (w, addr) = (*w, *addr);
                self.expr(addr);
                if w == 1 {
                    self.line("loadb r0, r0");
                } else {
                    self.line("load  r0, r0");
                }
            }
            Node::Emit(bytes) => {
                // intern the literal in the data pool; emit addr/len + a call to
                // the write_str helper (compact: the bytes live once in a `db`
                // data section, ~1 tape byte/char, not an imm+write per byte).
                let idx = match self.strings.iter().position(|s| s == bytes) {
                    Some(i) => i,
                    None => {
                        self.strings.push(bytes.clone());
                        self.strings.len() - 1
                    }
                };
                let len = bytes.len();
                self.line(&format!("imm   r0, _str{}", idx));
                self.line(&format!("imm   r1, {}", len));
                self.line("call  __write_str");
            }
        }
    }

    fn epilogue(&mut self) {
        self.line("mov   r15, r14");
        self.line("load  r14, r15");
        self.line("imm   r2, 8");
        self.line("add   r15, r2");
        self.line("ret");
    }

    fn store_slot(&mut self, slot: usize, e: usize) {
        self.expr(e); // value in r0
        self.slot_addr(1, 2, slot); // r1 = &slot (r0 preserved)
        self.line("store r1, r0");
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Let(slot, e) | Stmt::Assign(slot, e) => self.store_slot(*slot, *e),
            Stmt::Return(e) => {
                self.expr(*e);
                self.epilogue();
            }
            Stmt::State(name, body) => {
                // a CFG basic block: a label, then its straight-line body. Falls through
                // to the next state unless the body transitions (`to`) or returns.
                let lbl = format!("{}__{}", self.cur_proc, name);
                self.at(&lbl);
                self.block(body);
            }
            Stmt::Goto(name) => {
                self.line(&format!("jmp   {}__{}", self.cur_proc, name));
            }
            Stmt::GotoWhen(cond, name) => {
                self.expr(*cond);
                let skip = self.newlabel();
                self.line(&format!("jz    r0, {}", skip)); // guard false -> fall through
                self.line(&format!("jmp   {}__{}", self.cur_proc, name));
                self.at(&skip);
            }
            Stmt::Store(w, addr, val) => {
                self.expr(*addr);
                self.push_r0();
                self.expr(*val);
                self.pop_into(1); // r1 = address
                if *w == 1 {
                    self.line("storeb r1, r0");
                } else {
                    self.line("store r1, r0");
                }
            }
            Stmt::CallStmt(e) => {
                self.expr(*e); // evaluate for effect; result in r0 is discarded
            }
        }
    }
    fn block(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    fn proc(&mut self, p: &Proc) {
        self.cur_proc = p.name.clone();
        self.at(&p.name);
        // prologue: push caller fp, fp = sp, allocate frame, store params.
        // Scratch is r5, NOT r2 — args arrive live in r0..r3, and r2 (arg 3) must
        // survive until it is stored to its frame slot below.
        self.line("imm   r5, 8");
        self.line("sub   r15, r5");
        self.line("store r15, r14");
        self.line("mov   r14, r15");
        if p.nslots > 0 {
            self.line(&format!("imm   r5, {}", p.nslots * 8));
            self.line("sub   r15, r5");
        }
        for k in 0..p.nparams {
            self.slot_addr(5, 4, k); // r5 = &param k (scratch r4; never an arg register)
            self.line(&format!("store r5, r{}", k));
        }
        self.block(&p.body);
        // fall-through safety: tear down the frame and return (r0 as-is).
        self.epilogue();
    }
}

fn main() {
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).unwrap();

    let mut parser = Parser {
        toks: lex(&src),
        pos: 0,
        nodes: Vec::new(),
        names: Vec::new(),
    };
    let procs = parser.program();
    if !procs.iter().any(|p| p.name == "main") {
        panic!("beta-lang: no `proc main`");
    }

    let mut g = Gen {
        nodes: &parser.nodes,
        out: String::new(),
        label: 0,
        strings: Vec::new(),
        cur_proc: String::new(),
    };
    g.out.push_str("; generated by beta-lang (Beta -> Alpha assembly)\n");
    g.out.push_str("        imm   r15, 1048576      ; data stack pointer (sp)\n");
    g.out.push_str("        imm   r14, 1048576      ; frame pointer (fp)\n");
    g.out.push_str("        call  main\n");
    g.out.push_str("        halt  r0\n");
    for p in &procs {
        g.proc(p);
    }
    // emit() support: a leaf write_str helper + the interned string data section.
    // Only emitted when something used emit(), so non-emitting programs are
    // byte-identical to before.
    if !g.strings.is_empty() {
        g.out.push_str("__write_str:\n"); // r0 = addr, r1 = len; write len bytes from [addr]
        g.out.push_str("__ws_loop:\n");
        g.line("imm   r3, 0");
        g.line("jeq   r1, r3, __ws_done");
        g.line("loadb r2, r0");
        g.line("write r2");
        g.line("imm   r3, 1");
        g.line("add   r0, r3");
        g.line("sub   r1, r3");
        g.line("jmp   __ws_loop");
        g.out.push_str("__ws_done:\n");
        g.line("ret");
        let strings = std::mem::take(&mut g.strings);
        for (i, s) in strings.iter().enumerate() {
            let mut esc = String::new();
            for &b in s {
                match b {
                    b'\n' => esc.push_str("\\n"),
                    b'\t' => esc.push_str("\\t"),
                    b'\r' => esc.push_str("\\r"),
                    0 => esc.push_str("\\0"),
                    b'\\' => esc.push_str("\\\\"),
                    b'"' => esc.push_str("\\\""),
                    _ => esc.push(b as char),
                }
            }
            g.out.push_str(&format!("_str{}:\n        db \"{}\"\n", i, esc));
        }
    }
    print!("{}", g.out);
}

#!/usr/bin/env python3
# bc2.py — a SECOND, independent Beta compiler (Beta source -> Alpha assembly), written from scratch
# in Python against alpha/SEMANTICS.md and the Beta language, NOT ported from beta-lang-rs.
#
# WHY THIS EXISTS — the Thompson diversity gap (decision D5). bc.beta's only source->assembly path is the
# Rust on-ramp (beta-lang-rs); self-host reproduces bc but does not DIVERSIFY it, so a Trojan injected by
# the Rust on-ramp would perpetuate through the fixed point undetected. Diverse double compilation needs a
# SECOND, independent compiler for the same language. bc2.py is that second path.
#
# TRUST STATUS: UNTRUSTED, exactly like elab.py / prover.py / tv-encode.py. Its output is CHECKED, never
# trusted — the diverse-double-compilation gate assembles + runs what it emits and compares against the
# independent path. A bug or Trojan in bc2.py makes the comparison FAIL loudly; it can never make a wrong
# result silently pass. So Python is fine here: it is a verification tool, not part of the runtime TCB.
#
# Reads Beta on stdin, writes Alpha assembly on stdout — same interface as beta-lang-rs and bc.beta.
#
# SLICES SO FAR:
#   1  a single `proc main()` with `let` locals, assignment, `+ - * / %` (parens + precedence), `return`.
#   2  the six comparisons (materialised as 0/1) and CFG control flow: `state NAME { ... }` basic blocks
#      linked by `to STATE` / `to STATE when (cond)` (Beta has no if/while).
# Grown slice by slice toward the full language (procs/params/calls/recursion, byte[]/word[] memory,
# char/string literals, read_byte/write_byte) until it compiles bc.beta itself.
#
# Codegen is a straightforward stack machine over the register ISA, mirroring the shape the lattice's
# other Beta compilers use (r15 = data stack, r14 = frame pointer, r0 = accumulator, r1 = rhs; the ISA's
# own `sp` handles call/ret return addresses). Emitting the same shape is fine — independence lives in the
# CODE (a fresh implementation), and the gate checks behaviour, not that we differ.
import sys

# ---- lexer -------------------------------------------------------------------------------------
# Beta tokens: identifiers/keywords, decimal integers, and single/double-char operators. `;` starts a
# comment to end-of-line. Whitespace (incl. newlines) separates tokens; there are no statement terminators.
OPS = ['<=', '>=', '==', '!=', '+', '-', '*', '/', '%', '<', '>', '=',
       '(', ')', '{', '}', '[', ']', ',']

def lex(src):
    toks = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == ';':                                   # comment to end of line
            while i < n and src[i] != '\n':
                i += 1
            continue
        if c in ' \t\r\n':
            i += 1
            continue
        if c.isdigit():
            j = i
            while j < n and src[j].isdigit():
                j += 1
            toks.append(('num', int(src[i:j])))
            i = j
            continue
        if c.isalpha() or c == '_':
            j = i
            while j < n and (src[j].isalnum() or src[j] == '_'):
                j += 1
            toks.append(('word', src[i:j]))
            i = j
            continue
        for op in OPS:                                 # longest-match handled by OPS ordering (2-char first)
            if src.startswith(op, i):
                toks.append(('op', op))
                i += len(op)
                break
        else:
            raise SyntaxError(f'bc2: unexpected char {c!r} at offset {i}')
    toks.append(('eof', None))
    return toks

# ---- parser ------------------------------------------------------------------------------------
# Produces a tiny AST of tuples. Slice 1 grammar:
#   program = proc*
#   proc    = 'proc' name '(' ')' '{' stmt* '}'
#   stmt    = 'let' name '=' expr | name '=' expr | 'return' expr
#   expr    = term  (('+'|'-') term)*
#   term    = factor(('*'|'/'|'%') factor)*
#   factor  = num | name | '(' expr ')'
class Parser:
    def __init__(self, toks):
        self.toks = toks
        self.i = 0

    def peek(self):
        return self.toks[self.i]

    def nxt(self):
        t = self.toks[self.i]
        self.i += 1
        return t

    def expect(self, kind, val=None):
        t = self.nxt()
        if t[0] != kind or (val is not None and t[1] != val):
            raise SyntaxError(f'bc2: expected {kind} {val!r}, got {t}')
        return t

    def parse(self):
        procs = []
        while self.peek()[0] != 'eof':
            procs.append(self.proc())
        return procs

    def proc(self):
        self.expect('word', 'proc')
        name = self.expect('word')[1]
        self.expect('op', '(')
        params = []
        while self.peek() != ('op', ')'):
            params.append(self.expect('word')[1])
            if self.peek() == ('op', ','):
                self.nxt()
        self.expect('op', ')')
        self.expect('op', '{')
        body = []
        while self.peek() != ('op', '}'):
            body.append(self.stmt())
        self.expect('op', '}')
        return ('proc', name, params, body)

    def stmt(self):
        t = self.peek()
        if t == ('word', 'let'):
            self.nxt()
            name = self.expect('word')[1]
            self.expect('op', '=')
            return ('let', name, self.expr())
        if t == ('word', 'return'):
            self.nxt()
            return ('return', self.expr())
        if t == ('word', 'state'):                     # state NAME { stmt* }
            self.nxt()
            name = self.expect('word')[1]
            self.expect('op', '{')
            body = []
            while self.peek() != ('op', '}'):
                body.append(self.stmt())
            self.expect('op', '}')
            return ('state', name, body)
        if t == ('word', 'to'):                        # to STATE [when (cond)]
            self.nxt()
            target = self.expect('word')[1]
            cond = None
            if self.peek() == ('word', 'when'):
                self.nxt()
                self.expect('op', '(')
                cond = self.expr()
                self.expect('op', ')')
            return ('goto', target, cond)
        if t[0] == 'word':                             # assignment: name = expr
            name = self.nxt()[1]
            self.expect('op', '=')
            return ('assign', name, self.expr())
        raise SyntaxError(f'bc2: bad statement at {t}')

    # expr = comparison (lowest); comparison = addsub ((< > <= >= == !=) addsub)* ; addsub = term ...
    CMP = ['<', '>', '<=', '>=', '==', '!=']

    def expr(self):
        e = self.addsub()
        while self.peek()[0] == 'op' and self.peek()[1] in self.CMP:
            op = self.nxt()[1]
            e = ('bin', op, e, self.addsub())
        return e

    def addsub(self):
        e = self.term()
        while self.peek() in (('op', '+'), ('op', '-')):
            op = self.nxt()[1]
            e = ('bin', op, e, self.term())
        return e

    def term(self):
        e = self.factor()
        while self.peek() in (('op', '*'), ('op', '/'), ('op', '%')):
            op = self.nxt()[1]
            e = ('bin', op, e, self.factor())
        return e

    def factor(self):
        t = self.nxt()
        if t[0] == 'num':
            return ('num', t[1])
        if t[0] == 'word':
            return ('var', t[1])
        if t == ('op', '('):
            e = self.expr()
            self.expect('op', ')')
            return e
        raise SyntaxError(f'bc2: bad factor at {t}')

# ---- codegen -----------------------------------------------------------------------------------
BINOP = {'+': 'add', '-': 'sub', '*': 'mul', '/': 'div', '%': 'mod'}

class Gen:
    def __init__(self):
        self.out = []
        self.lc = 0                                    # fresh-label counter (_L0, _L1, ...)
        self.curproc = None

    def emit(self, s):
        self.out.append('        ' + s)

    def label(self, s):
        self.out.append(s + ':')

    def newlabel(self):
        self.lc += 1
        return f'_L{self.lc - 1}'

    def program(self, procs):
        self.out.append('; generated by bc2.py (Beta -> Alpha assembly; independent second path, D5)')
        self.emit('imm   r15, 1048576')               # data stack pointer
        self.emit('imm   r14, 1048576')               # frame pointer
        self.emit('call  main')
        self.emit('halt  r0')
        for p in procs:
            self.proc(p)
        return '\n'.join(self.out) + '\n'

    def collect_lets(self, body, slots):
        """prescan (source order, incl. inside states) so the frame is sized once up front"""
        for s in body:
            if s[0] == 'let' and s[1] not in slots:
                slots[s[1]] = len(slots)
            elif s[0] == 'state':
                self.collect_lets(s[2], slots)

    def proc(self, p):
        _, name, params, body = p
        self.curproc = name
        slots = {}
        for k, pname in enumerate(params):             # params occupy the first slots
            slots[pname] = k
        self.collect_lets(body, slots)
        self.label(name)
        self.emit('imm   r5, 8')                       # prologue: push old fp, fp = sp
        self.emit('sub   r15, r5')
        self.emit('store r15, r14')
        self.emit('mov   r14, r15')
        if slots:
            self.emit(f'imm   r5, {8 * len(slots)}')    # allocate frame for all locals
            self.emit('sub   r15, r5')
        # entry statements run, then fall into the first state; each state is a labelled block.
        for s in body:
            if s[0] == 'state':
                self.label(f'{name}__{s[1]}')
                for st in s[2]:
                    self.stmt(st, slots)
            else:
                self.stmt(s, slots)
        self.epilogue()                                # fallthrough epilogue (mirrors the on-ramp)

    def off(self, slots, name):
        if name not in slots:
            raise SyntaxError(f'bc2: unknown name {name!r}')
        return 8 * (slots[name] + 1)                    # fp - 8*(slot+1)

    def stmt(self, s, slots):
        if s[0] in ('let', 'assign'):
            self.expr(s[2], slots)                       # value -> r0
            self.emit('mov   r1, r14')                   # store r0 to fp - off
            self.emit(f'imm   r2, {self.off(slots, s[1])}')
            self.emit('sub   r1, r2')
            self.emit('store r1, r0')
            return
        if s[0] == 'return':
            self.expr(s[1], slots)                        # value -> r0
            self.epilogue()
            return
        if s[0] == 'goto':                                # to STATE [when (cond)]
            _, target, cond = s
            dest = f'{self.curproc}__{target}'
            if cond is None:
                self.emit(f'jmp   {dest}')
            else:
                self.expr(cond, slots)                     # cond -> r0 (0/1)
                skip = self.newlabel()
                self.emit(f'jz    r0, {skip}')             # cond false -> fall through
                self.emit(f'jmp   {dest}')                 # cond true  -> take the transition
                self.label(skip)
            return
        raise SyntaxError(f'bc2: bad stmt {s}')

    def expr(self, e, slots):
        if e[0] == 'num':
            self.emit(f'imm   r0, {e[1]}')
            return
        if e[0] == 'var':
            self.emit('mov   r0, r14')
            self.emit(f'imm   r1, {self.off(slots, e[1])}')
            self.emit('sub   r0, r1')
            self.emit('load  r0, r0')
            return
        if e[0] == 'bin':
            _, op, a, b = e
            self.expr(a, slots)                           # A -> r0
            self.emit('imm   r2, 8')                      # push A
            self.emit('sub   r15, r2')
            self.emit('store r15, r0')
            self.expr(b, slots)                           # B -> r0
            self.emit('mov   r1, r0')                     # B -> r1
            self.emit('load  r0, r15')                    # pop A -> r0
            self.emit('imm   r5, 8')
            self.emit('add   r15, r5')
            if op in BINOP:
                self.emit(f'{BINOP[op]:5} r0, r1')        # r0 = A op B (arithmetic)
            else:
                self.cmp(op)                              # r0 = (A op B) ? 1 : 0
            return
        raise SyntaxError(f'bc2: bad expr {e}')

    # materialise a comparison of r0 (A) and r1 (B) into r0 as 0/1, using jlt (signed <) and jeq.
    # <,>,== jump straight to the true case; <=,>=,!= are the negations (jump to the false case).
    def cmp(self, op):
        JUMP = {'<':  'jlt   r0, r1', '>':  'jlt   r1, r0', '==': 'jeq   r0, r1',
                '<=': 'jlt   r1, r0', '>=': 'jlt   r0, r1', '!=': 'jeq   r0, r1'}
        true_on_jump = op in ('<', '>', '==')
        lj, lend = self.newlabel(), self.newlabel()
        self.emit(f'{JUMP[op]}, {lj}')
        self.emit(f'imm   r0, {0 if true_on_jump else 1}')   # fall-through case
        self.emit(f'jmp   {lend}')
        self.label(lj)
        self.emit(f'imm   r0, {1 if true_on_jump else 0}')   # jump-taken case
        self.label(lend)

    def epilogue(self):
        self.emit('mov   r15, r14')                       # sp = fp; pop old fp; ret
        self.emit('load  r14, r15')
        self.emit('imm   r2, 8')
        self.emit('add   r15, r2')
        self.emit('ret')

def main():
    src = sys.stdin.read()
    procs = Parser(lex(src)).parse()
    sys.stdout.write(Gen().program(procs))

main()

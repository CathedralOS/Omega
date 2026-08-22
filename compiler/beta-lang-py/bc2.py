#!/usr/bin/env python3
# bc2.py — a separately written Beta reference compiler (Beta source -> Alpha assembly)
# in Python against alpha/SEMANTICS.md and the Beta language, NOT ported from beta-lang-rs.
#
# HISTORICAL ROLE — this formerly implemented a DDC ruling that has since been
# superseded by checked source-to-artifact refinement. It remains because its
# parser and compiler are useful to optional reference/regression tools.
#
# TRUST STATUS: UNTRUSTED, exactly like elab.py / prover.py / tv-encode.py. Its output is CHECKED, never
# trusted. A disagreement with canonical meaning or another implementation is a
# useful diagnostic, but agreement grants no authority. Python is an implementation
# detail of this reference tool, not part of the runtime TCB.
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
        if c == '"':                                   # string literal (for emit) -> raw inner text
            j = i + 1
            inner = ''
            while j < n and src[j] != '"':
                if src[j] == '\\':
                    inner += src[j:j + 2]
                    j += 2
                else:
                    inner += src[j]
                    j += 1
            toks.append(('str', inner))
            i = j + 1
            continue
        if c == "'":                                   # char literal 'x' / '\n' -> its byte value
            i += 1
            if src[i] == '\\':
                val = {'n': 10, 't': 9, 'r': 13, '0': 0, '\\': 92, "'": 39, '"': 34}[src[i + 1]]
                i += 2
            else:
                val = ord(src[i])
                i += 1
            if src[i] != "'":
                raise SyntaxError('bc2: unterminated char literal')
            i += 1
            toks.append(('num', val))
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
        if t[0] == 'word' and t[1] in ('word', 'byte') and self.toks[self.i + 1] == ('op', '['):
            kind = self.nxt()[1]                       # memory write: word[addr] = val / byte[addr] = val
            self.expect('op', '[')
            addr = self.expr()
            self.expect('op', ']')
            self.expect('op', '=')
            return ('memset', kind, addr, self.expr())
        if t == ('word', 'emit'):                      # emit("...") — write a string literal
            self.nxt()
            self.expect('op', '(')
            s = self.expect('str')[1]
            self.expect('op', ')')
            return ('emit', s)
        if t[0] == 'word' and self.toks[self.i + 1] == ('op', '('):   # call statement (result discarded)
            name = self.nxt()[1]
            self.nxt()                                 # '('
            args = []
            while self.peek() != ('op', ')'):
                args.append(self.expr())
                if self.peek() == ('op', ','):
                    self.nxt()
            self.expect('op', ')')
            return ('callstmt', ('call', name, args))
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
            if t[1] in ('word', 'byte') and self.peek() == ('op', '['):   # memory read
                self.nxt()
                addr = self.expr()
                self.expect('op', ']')
                return ('mem', t[1], addr)
            if self.peek() == ('op', '('):             # call: name ( args )
                self.nxt()
                args = []
                while self.peek() != ('op', ')'):
                    args.append(self.expr())
                    if self.peek() == ('op', ','):
                        self.nxt()
                self.expect('op', ')')
                return ('call', t[1], args)
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
        self.strings = []                              # string literals -> _strN db data (emitted at end)

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
        if self.strings:                               # once-per-program string runtime + data
            self.write_str_helper()
            for idx, s in enumerate(self.strings):
                self.label(f'_str{idx}')
                self.emit(f'db "{s}"')
        return '\n'.join(self.out) + '\n'

    def write_str_helper(self):
        # __write_str(r0 = addr, r1 = length): write r1 bytes starting at r0.
        self.label('__write_str')
        self.label('__ws_loop')
        self.emit('imm   r3, 0')
        self.emit('jeq   r1, r3, __ws_done')
        self.emit('loadb r2, r0')
        self.emit('write r2')
        self.emit('imm   r3, 1')
        self.emit('add   r0, r3')
        self.emit('sub   r1, r3')
        self.emit('jmp   __ws_loop')
        self.label('__ws_done')
        self.emit('ret')

    @staticmethod
    def strbytes(inner):
        """decoded byte length of a string literal (each \\X escape is one byte)"""
        n = i = 0
        while i < len(inner):
            i += 2 if inner[i] == '\\' else 1
            n += 1
        return n

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
        for k in range(len(params)):                    # spill incoming arg regs r0..r3 into param slots
            self.emit('mov   r5, r14')
            self.emit(f'imm   r4, {8 * (k + 1)}')
            self.emit('sub   r5, r4')
            self.emit(f'store r5, r{k}')
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
        if s[0] == 'emit':                                # emit("...") -> write the string via __write_str
            idx = len(self.strings)
            self.strings.append(s[1])
            self.emit(f'imm   r0, _str{idx}')
            self.emit(f'imm   r1, {self.strbytes(s[1])}')
            self.emit('call  __write_str')
            return
        if s[0] == 'callstmt':                            # a call used for effect; result in r0 discarded
            self.expr(s[1], slots)
            return
        if s[0] == 'memset':                              # word[addr] = val  /  byte[addr] = val
            _, kind, addr, val = s
            self.expr(addr, slots)                        # addr -> r0
            self.emit('imm   r2, 8')                      # push addr
            self.emit('sub   r15, r2')
            self.emit('store r15, r0')
            self.expr(val, slots)                         # val -> r0
            self.emit('load  r1, r15')                    # pop addr -> r1
            self.emit('imm   r5, 8')
            self.emit('add   r15, r5')
            self.emit(f'{"store" if kind == "word" else "storeb"} r1, r0')
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
        if e[0] == 'mem':                                 # word[addr] / byte[addr] read
            _, kind, addr = e
            self.expr(addr, slots)                        # addr -> r0
            self.emit(f'{"load " if kind == "word" else "loadb"} r0, r0')
            return
        if e[0] == 'call':                                # name(args): push args L->R, pop reverse to r0..
            _, name, args = e
            if name == 'read_byte':                       # intrinsic: next input byte (0xFFFF..FF at EOF) -> r0
                self.emit('read  r0')
                return
            if name == 'write_byte':                      # intrinsic: append low byte of the arg to output
                self.expr(args[0], slots)
                self.emit('write r0')
                return
            if len(args) > 4:
                raise SyntaxError(f'bc2: >4 args unsupported (call {name})')
            for a in args:
                self.expr(a, slots)                       # arg -> r0
                self.emit('imm   r2, 8')                  # push it (args share the data stack, popped below)
                self.emit('sub   r15, r2')
                self.emit('store r15, r0')
            for i in range(len(args) - 1, -1, -1):        # top of stack is the last arg -> highest reg
                self.emit(f'load  r{i}, r15')
                self.emit('imm   r5, 8')
                self.emit('add   r15, r5')
            self.emit(f'call  {name}')                    # result in r0
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

if __name__ == '__main__':                             # importable (beta_interp.py reuses lex + Parser)
    main()

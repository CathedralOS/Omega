#!/usr/bin/env python3
"""Shared untrusted lexer/parser for Beta reference and refinement tools.

This module owns source recognition only. It deliberately contains no compiler,
interpreter, symbolic evaluator, or trust claim; those consumers assign meaning
or produce diagnostics in their own responsibility-specific modules. Its
recursive `state` parser still admits arbitrary statement/state interleaving;
BETA-FLATTENED-CFG-INITIALIZATION requires the reference migration to enforce
one ordinary prefix followed by child states in every block.
"""

# Beta tokens: identifiers/keywords, decimal integers, and single/double-char
# operators. `;` starts a comment to end-of-line. Whitespace separates tokens;
# there are no statement terminators.
OPS = [
    '<=', '>=', '==', '!=', '+', '-', '*', '/', '%', '<', '>', '=',
    '(', ')', '{', '}', '[', ']', ',',
]
MAX_WORD = (1 << 64) - 1


def lex(src):
    toks = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == ';':
            while i < n and src[i] != '\n':
                i += 1
            continue
        if c == '"':
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
        if c == "'":
            i += 1
            if src[i] == '\\':
                val = {
                    'n': 10, 't': 9, 'r': 13, '0': 0, '\\': 92,
                    "'": 39, '"': 34,
                }[src[i + 1]]
                i += 2
            else:
                val = ord(src[i])
                i += 1
            if src[i] != "'":
                raise SyntaxError('beta parser: unterminated char literal')
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
            value = int(src[i:j])
            if value > MAX_WORD:
                raise SyntaxError('beta parser: decimal literal exceeds Word')
            toks.append(('num', value))
            i = j
            continue
        if c.isalpha() or c == '_':
            j = i
            while j < n and (src[j].isalnum() or src[j] == '_'):
                j += 1
            toks.append(('word', src[i:j]))
            i = j
            continue
        for op in OPS:
            if src.startswith(op, i):
                toks.append(('op', op))
                i += len(op)
                break
        else:
            raise SyntaxError(f'beta parser: unexpected char {c!r} at offset {i}')
    toks.append(('eof', None))
    return toks


class Parser:
    """Parse the reference Beta surface into the historical tuple AST."""

    def __init__(self, toks):
        self.toks = toks
        self.i = 0

    def peek(self):
        return self.toks[self.i]

    def nxt(self):
        token = self.toks[self.i]
        self.i += 1
        return token

    def expect(self, kind, val=None):
        token = self.nxt()
        if token[0] != kind or (val is not None and token[1] != val):
            raise SyntaxError(
                f'beta parser: expected {kind} {val!r}, got {token}'
            )
        return token

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
        token = self.peek()
        if token == ('word', 'let'):
            self.nxt()
            name = self.expect('word')[1]
            self.expect('op', '=')
            return ('let', name, self.expr())
        if token == ('word', 'return'):
            self.nxt()
            return ('return', self.expr())
        if token == ('word', 'state'):
            self.nxt()
            name = self.expect('word')[1]
            self.expect('op', '{')
            body = []
            while self.peek() != ('op', '}'):
                body.append(self.stmt())
            self.expect('op', '}')
            return ('state', name, body)
        if token == ('word', 'to'):
            self.nxt()
            target = self.expect('word')[1]
            condition = None
            if self.peek() == ('word', 'when'):
                self.nxt()
                condition = self.expr()
            return ('goto', target, condition)
        if (
            token[0] == 'word'
            and token[1] in ('word', 'byte')
            and self.toks[self.i + 1] == ('op', '[')
        ):
            kind = self.nxt()[1]
            self.expect('op', '[')
            address = self.expr()
            self.expect('op', ']')
            self.expect('op', '=')
            return ('memset', kind, address, self.expr())
        if token == ('word', 'emit'):
            self.nxt()
            self.expect('op', '(')
            string = self.expect('str')[1]
            self.expect('op', ')')
            return ('emit', string)
        if token[0] == 'word' and self.toks[self.i + 1] == ('op', '('):
            name = self.nxt()[1]
            self.nxt()
            args = []
            while self.peek() != ('op', ')'):
                args.append(self.expr())
                if self.peek() == ('op', ','):
                    self.nxt()
            self.expect('op', ')')
            return ('callstmt', ('call', name, args))
        if token[0] == 'word':
            name = self.nxt()[1]
            self.expect('op', '=')
            return ('assign', name, self.expr())
        raise SyntaxError(f'beta parser: bad statement at {token}')

    CMP = ['<', '>', '<=', '>=', '==', '!=']

    def expr(self):
        expression = self.addsub()
        while self.peek()[0] == 'op' and self.peek()[1] in self.CMP:
            operator = self.nxt()[1]
            expression = ('bin', operator, expression, self.addsub())
        return expression

    def addsub(self):
        expression = self.term()
        while self.peek() in (('op', '+'), ('op', '-')):
            operator = self.nxt()[1]
            expression = ('bin', operator, expression, self.term())
        return expression

    def term(self):
        expression = self.factor()
        while self.peek() in (('op', '*'), ('op', '/'), ('op', '%')):
            operator = self.nxt()[1]
            expression = ('bin', operator, expression, self.factor())
        return expression

    def factor(self):
        token = self.nxt()
        if token[0] == 'num':
            return ('num', token[1])
        if token[0] == 'word':
            if token[1] in ('word', 'byte') and self.peek() == ('op', '['):
                self.nxt()
                address = self.expr()
                self.expect('op', ']')
                return ('mem', token[1], address)
            if self.peek() == ('op', '('):
                self.nxt()
                args = []
                while self.peek() != ('op', ')'):
                    args.append(self.expr())
                    if self.peek() == ('op', ','):
                        self.nxt()
                self.expect('op', ')')
                return ('call', token[1], args)
            return ('var', token[1])
        if token == ('op', '('):
            expression = self.expr()
            self.expect('op', ')')
            return expression
        raise SyntaxError(f'beta parser: bad factor at {token}')

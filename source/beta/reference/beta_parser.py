#!/usr/bin/env python3
"""Shared untrusted lexer/parser for Beta reference and refinement tools.

This module owns source recognition and the written, finite Beta formation
judgments. It deliberately contains no compiler, interpreter, symbolic
evaluator, or trust claim; those consumers assign runtime meaning or produce
diagnostics in their own responsibility-specific modules.
"""

# Beta tokens: identifiers/keywords, decimal integers, and single/double-char
# operators. `;` starts a comment to end-of-line. Whitespace separates tokens;
# there are no statement terminators.
OPS = [
    '<=', '>=', '==', '!=', '+', '-', '*', '/', '%', '<', '>', '=',
    '(', ')', '{', '}', '[', ']', ',',
]
MAX_WORD = (1 << 64) - 1
STRING_ESCAPES = {'n', 't', 'r', '0', '\\', '"'}
CHAR_ESCAPES = {
    'n': 10, 't': 9, 'r': 13, '0': 0, '\\': 92, "'": 39,
}


def lex(src):
    if isinstance(src, bytes):
        src = src.decode('latin1')
    for offset, c in enumerate(src):
        value = ord(c)
        if value not in (9, 10, 13) and not 32 <= value <= 126:
            raise SyntaxError(
                f'beta parser: invalid source byte at offset {offset}'
            )
    toks = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == ';':
            while i < n and src[i] not in '\r\n':
                i += 1
            continue
        if c == '"':
            j = i + 1
            inner = ''
            while j < n:
                if src[j] == '"':
                    break
                if src[j] == '\\':
                    if j + 1 >= n or src[j + 1] not in STRING_ESCAPES:
                        raise SyntaxError('beta parser: invalid string escape')
                    inner += src[j:j + 2]
                    j += 2
                else:
                    inner += src[j]
                    j += 1
            if j >= n:
                raise SyntaxError('beta parser: unterminated string literal')
            toks.append(('str', inner))
            i = j + 1
            continue
        if c == "'":
            i += 1
            if i >= n:
                raise SyntaxError('beta parser: unterminated char literal')
            if src[i] == '\\':
                if i + 1 >= n or src[i + 1] not in CHAR_ESCAPES:
                    raise SyntaxError('beta parser: invalid char escape')
                val = CHAR_ESCAPES[src[i + 1]]
                i += 2
            else:
                val = ord(src[i])
                i += 1
            if i >= n or src[i] != "'":
                raise SyntaxError('beta parser: unterminated char literal')
            i += 1
            toks.append(('num', val))
            continue
        if c in ' \t\r\n':
            i += 1
            continue
        if '0' <= c <= '9':
            j = i
            while j < n and '0' <= src[j] <= '9':
                j += 1
            value = int(src[i:j])
            if value > MAX_WORD:
                raise SyntaxError('beta parser: decimal literal exceeds Word')
            toks.append(('num', value))
            i = j
            continue
        if 'A' <= c <= 'Z' or 'a' <= c <= 'z' or c == '_':
            j = i
            while j < n and (
                'A' <= src[j] <= 'Z' or 'a' <= src[j] <= 'z' or
                '0' <= src[j] <= '9' or src[j] == '_'
            ):
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
        validate_program(procs)
        return procs

    def proc(self):
        self.expect('word', 'proc')
        name = self.expect('word')[1]
        self.expect('op', '(')
        params = []
        if self.peek() != ('op', ')'):
            params.append(self.expect('word')[1])
            while self.peek() == ('op', ','):
                self.nxt()
                if self.peek() == ('op', ')'):
                    raise SyntaxError('beta parser: trailing parameter comma')
                params.append(self.expect('word')[1])
        self.expect('op', ')')
        body = self.block()
        return ('proc', name, params, body)

    def block(self):
        self.expect('op', '{')
        body = []
        states_started = False
        terminated = False
        while self.peek() != ('op', '}'):
            if self.peek() == ('word', 'state'):
                states_started = True
                body.append(self.state())
                continue
            if states_started:
                raise SyntaxError(
                    'beta parser: ordinary statement after child state'
                )
            if terminated:
                raise SyntaxError(
                    'beta parser: ordinary statement after block terminator'
                )
            statement = self.stmt()
            body.append(statement)
            terminated = statement[0] == 'return' or (
                statement[0] == 'goto' and statement[2] is None
            )
        self.expect('op', '}')
        return body

    def state(self):
        self.expect('word', 'state')
        name = self.expect('word')[1]
        return ('state', name, self.block())

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
            args = self.args()
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
        if self.peek()[0] == 'op' and self.peek()[1] in self.CMP:
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
                args = self.args()
                self.expect('op', ')')
                return ('call', token[1], args)
            return ('var', token[1])
        if token == ('op', '('):
            expression = self.expr()
            self.expect('op', ')')
            return expression
        raise SyntaxError(f'beta parser: bad factor at {token}')

    def args(self):
        args = []
        if self.peek() == ('op', ')'):
            return args
        args.append(self.expr())
        while self.peek() == ('op', ','):
            self.nxt()
            if self.peek() == ('op', ')'):
                raise SyntaxError('beta parser: trailing argument comma')
            args.append(self.expr())
        return args


RESERVED = {
    'proc', 'return', 'let', 'state', 'to', 'when', 'byte', 'word',
    'emit', 'read_byte', 'write_byte',
}


def split_block(body):
    """Return one block's ordinary prefix and child-state suffix."""
    first_state = next(
        (index for index, statement in enumerate(body) if statement[0] == 'state'),
        len(body),
    )
    return body[:first_state], body[first_state:]


def flatten_blocks(body):
    """Flatten an authored block into entry/state blocks in DFS lexical order."""
    ordinary, states = split_block(body)
    flattened = [(None, ordinary)]

    def visit(children):
        for state in children:
            state_ordinary, state_children = split_block(state[2])
            flattened.append((state[1], state_ordinary))
            visit(state_children)

    visit(states)
    return flattened


def walk_expression(expression, visible, generated, required, calls):
    kind = expression[0]
    if kind == 'num':
        return
    if kind == 'var':
        name = expression[1]
        if name not in visible:
            raise SyntaxError(f'beta formation: unresolved local {name!r}')
        if name not in generated:
            required.add(name)
        return
    if kind == 'mem':
        walk_expression(expression[2], visible, generated, required, calls)
        return
    if kind == 'call':
        for argument in expression[2]:
            walk_expression(argument, visible, generated, required, calls)
        calls.append((expression[1], len(expression[2])))
        return
    if kind == 'bin':
        walk_expression(expression[2], visible, generated, required, calls)
        walk_expression(expression[3], visible, generated, required, calls)
        return
    raise SyntaxError(f'beta formation: unknown expression {kind!r}')


def validate_procedure(proc, calls):
    _, proc_name, params, body = proc
    if proc_name in RESERVED:
        raise SyntaxError(f'beta formation: reserved procedure {proc_name!r}')
    if len(params) > 4 or len(set(params)) != len(params):
        raise SyntaxError(f'beta formation: invalid parameters for {proc_name!r}')
    if any(param in RESERVED for param in params):
        raise SyntaxError(f'beta formation: reserved parameter in {proc_name!r}')

    blocks = flatten_blocks(body)
    labels = {}
    for index, (label, _) in enumerate(blocks[1:], 1):
        if label in RESERVED or label in labels:
            raise SyntaxError(f'beta formation: invalid state {label!r}')
        labels[label] = index

    visible = set(params)
    declarations = set(params)
    summaries = []
    for block_index, (_, statements) in enumerate(blocks):
        generated = set()
        required = set()
        edges = []
        terminal = False
        for statement in statements:
            kind = statement[0]
            if kind == 'let':
                walk_expression(statement[2], visible, generated, required, calls)
                name = statement[1]
                if name in RESERVED or name in declarations:
                    raise SyntaxError(f'beta formation: invalid local {name!r}')
                declarations.add(name)
                visible.add(name)
                generated.add(name)
            elif kind == 'assign':
                name = statement[1]
                if name not in visible:
                    raise SyntaxError(f'beta formation: unresolved assignment {name!r}')
                walk_expression(statement[2], visible, generated, required, calls)
                generated.add(name)
            elif kind == 'return':
                walk_expression(statement[1], visible, generated, required, calls)
                terminal = True
            elif kind == 'callstmt':
                walk_expression(statement[1], visible, generated, required, calls)
            elif kind == 'memset':
                walk_expression(statement[2], visible, generated, required, calls)
                walk_expression(statement[3], visible, generated, required, calls)
            elif kind == 'emit':
                pass
            elif kind == 'goto':
                target = statement[1]
                if statement[2] is not None:
                    walk_expression(statement[2], visible, generated, required, calls)
                edges.append((target, set(generated)))
                terminal = statement[2] is None
            else:
                raise SyntaxError(f'beta formation: unknown statement {kind!r}')
        summaries.append({
            'required': required,
            'generated': generated,
            'edges': edges,
            'terminal': terminal,
        })

    successors = [[] for _ in blocks]
    for index, summary in enumerate(summaries):
        for target, generated in summary['edges']:
            if target not in labels:
                raise SyntaxError(f'beta formation: unresolved state {target!r}')
            successors[index].append((labels[target], generated))
        if not summary['terminal'] and index + 1 < len(blocks):
            successors[index].append((index + 1, set(summary['generated'])))

    reachable = {0}
    changed = True
    while changed:
        changed = False
        for source in tuple(reachable):
            for target, _ in successors[source]:
                if target not in reachable:
                    reachable.add(target)
                    changed = True

    top = set(declarations)
    incoming = [set(top) for _ in blocks]
    incoming[0] = set(params)
    changed = True
    while changed:
        changed = False
        for source in reachable:
            for target, generated in successors[source]:
                if target not in reachable:
                    continue
                candidate = incoming[source] | generated
                merged = incoming[target] & candidate
                if merged != incoming[target]:
                    incoming[target] = merged
                    changed = True

    for index in reachable:
        missing = summaries[index]['required'] - incoming[index]
        if missing:
            raise SyntaxError(
                f'beta formation: possibly uninitialized local {min(missing)!r}'
            )


def validate_program(procs):
    procedure_arities = {}
    for proc in procs:
        name = proc[1]
        if name in procedure_arities:
            raise SyntaxError(f'beta formation: duplicate procedure {name!r}')
        procedure_arities[name] = len(proc[2])
    if procedure_arities.get('main') != 0:
        raise SyntaxError('beta formation: expected one zero-parameter main')

    calls = []
    for proc in procs:
        validate_procedure(proc, calls)
    for name, arity in calls:
        if name == 'read_byte':
            expected = 0
        elif name == 'write_byte':
            expected = 1
        else:
            expected = procedure_arities.get(name)
        if expected is None or expected != arity:
            raise SyntaxError(f'beta formation: unresolved call {name!r}/{arity}')

"""Independent reconstruction of CheckEpsilon and RunEpsilon.

This module is the test-owned second implementation of the Epsilon v1
contract (bootstrap/4_epsilon/LANGUAGE.md), written from the contract text
rather than from the Delta evaluator's source. The refinement gate
(gate.py) runs the canonical evaluator edge on a source/stdin pair and
requires the published observation to equal this model's result byte-for-byte,
under mutations of the source, stdin, request profile, and expected
observation.

Modeled fragment: the grammar of section 3 (the boundary trait, record and
sum data declarations, machines with states and transitions, the full
expression precedence chain, fixed arrays, slices and views, Console
effects) with these declared limits:

- records are built only by zero initialization and field writes; the
  contract has no record literal syntax;
- `never` is modeled only as a machine/boundary return annotation;
- `u8` is modeled for fields, payload members, array elements, locals, and
  parameters: reads produce i32, stores range-check to ByteRange;
- `&[T]` views come from string literals, `.as_slice`, and `[lo..hi]` slices;
- lexical, parse, and the checking rejections exercised by the corpus are
  modeled at their contract coordinates; anything outside this fragment
  raises ModelExcluded rather than guessing a judgment.

Two disambiguations the contract grammar leaves open are reproduced from
the checked evaluator so both sides read the same program: the
return-continuation probe (`Parser.return_is_bare`), and duplicate names
inside bodies are reported at the first `let`/`state`/`case` construct byte
in traversal order rather than by a separate whole-program earliest-later-
declaration census.

Observations are encoded in the canonical edge grammar
(bootstrap/4_epsilon/EVALUATOR_ENTRY.md): 00 Exit u32 code + stdout,
01 Trap kind + stdout prefix, 02 Reject reason + u32 coordinate.
Divergence is actual nontermination; a bounded check cannot produce it, so
the corpus contains no divergent case.
"""

from dataclasses import dataclass

# --- Closed identities (LANGUAGE.md section 9) -------------------------------

REASONS = {
    "InvalidSourceByte": 1, "InvalidToken": 2, "InvalidCharacterLiteral": 3,
    "UnterminatedString": 4, "InvalidEscape": 5, "IntegerLiteralOutOfRange": 6,
    "UnexpectedToken": 7, "UnexpectedEnd": 8, "DuplicateName": 9,
    "MissingEntry": 10, "InvalidEntry": 11, "InvalidBoundary": 12,
    "UnknownType": 13, "RecursiveValueType": 14, "InvalidDataShape": 15,
    "InvalidArrayLength": 16, "UnknownName": 17, "TypeMismatch": 18,
    "ArityMismatch": 19, "InvalidPlace": 20, "UseBeforeInitialization": 21,
    "EscapingView": 22, "InvalidControlTarget": 23, "InvalidTerminal": 24,
    "DuplicatePattern": 25, "NonexhaustiveSum": 26,
}

TRAPS = {
    "Overflow": 1, "DivisionByZero": 2, "SignedDivisionOverflow": 3,
    "ShiftCount": 4, "ByteRange": 5, "Bounds": 6, "NonBoolean": 7,
    "Assertion": 8, "NonExhaustiveTransition": 9,
}

I32_MIN = -2147483648
I32_MAX = 2147483647


class Reject(Exception):
    def __init__(self, reason, offset):
        super().__init__(reason, offset)
        self.reason = REASONS[reason]
        self.offset = offset


class Trap(Exception):
    def __init__(self, kind):
        super().__init__(kind)
        self.kind = TRAPS[kind]


class ExitProcess(Exception):
    def __init__(self, code):
        super().__init__(code)
        self.code = code


class ModelExcluded(Exception):
    """A contract construct outside this reconstruction's declared fragment."""


# --- Lexer --------------------------------------------------------------------

KEYWORDS = {
    b"boundary", b"trait", b"data", b"case", b"machine", b"state",
    b"transition", b"let", b"return", b"assert", b"true", b"false",
    b"self", b"mut", b"i32", b"u8", b"never",
}

# Multi-byte operators first; the table is scanned in order at each position.
PUNCT = [b"->", b"..", b"::", b"==", b"!=", b"<=", b">=", b"<<", b">>",
         b"&&", b"||", b"(", b")", b"{", b"}", b"[", b"]", b";", b",",
         b":", b".", b"=", b"<", b">", b"+", b"-", b"*", b"/", b"%",
         b"&", b"^", b"|"]

ESCAPES = {0x6E: 10, 0x72: 13, 0x74: 9}  # n r t


@dataclass
class Token:
    kind: str        # "ident", "int", "char", "string", "punct", "eof"
    text: bytes
    start: int
    end: int
    value: object = None


def _hexdigit(byte):
    if 48 <= byte <= 57:
        return byte - 48
    if 65 <= byte <= 70:
        return byte - 55
    if 97 <= byte <= 102:
        return byte - 87
    return -1


def lex(source):
    tokens = []
    position = 0
    extent = len(source)
    while position < extent:
        byte = source[position]
        if byte not in (9, 10, 13) and not 32 <= byte <= 126:
            raise Reject("InvalidSourceByte", position)
        if byte in (9, 10, 13, 32):
            position += 1
            continue
        if source[position:position + 2] == b"//":
            while position < extent and source[position] not in (10, 13):
                position += 1
            continue
        if 65 <= byte <= 90 or 97 <= byte <= 122 or byte == 95:
            end = position + 1
            while end < extent and (
                    65 <= source[end] <= 90 or 97 <= source[end] <= 122
                    or 48 <= source[end] <= 57 or source[end] == 95):
                end += 1
            tokens.append(Token("ident", source[position:end], position, end))
            position = end
            continue
        if 48 <= byte <= 57:
            end = position + 1
            while end < extent and 48 <= source[end] <= 57:
                end += 1
            value = int(source[position:end])
            if value > 2147483648:
                raise Reject("IntegerLiteralOutOfRange", position)
            tokens.append(Token("int", source[position:end], position, end,
                                value))
            position = end
            continue
        if byte == 39:  # character literal
            start = position
            position += 1
            if position >= extent or source[position] in (10, 13, 39):
                raise Reject("InvalidCharacterLiteral", start)
            if source[position] == 92:
                position += 1
                if position >= extent or source[position] in (10, 13):
                    raise Reject("InvalidCharacterLiteral", start)
                esc = source[position]
                if esc in ESCAPES:
                    value = ESCAPES[esc]
                elif esc in (34, 92):
                    value = esc
                elif esc == 120:  # \xHH
                    if (position + 2 >= extent
                            or _hexdigit(source[position + 1]) < 0
                            or _hexdigit(source[position + 2]) < 0):
                        raise Reject("InvalidEscape", position - 1)
                    value = _hexdigit(source[position + 1]) * 16 \
                        + _hexdigit(source[position + 2])
                    position += 2
                else:
                    raise Reject("InvalidEscape", position - 1)
                position += 1
            elif 32 <= source[position] <= 126 and source[position] != 39:
                value = source[position]
                position += 1
            else:
                raise Reject("InvalidCharacterLiteral", start)
            if position >= extent or source[position] != 39:
                raise Reject("InvalidCharacterLiteral", start)
            position += 1
            tokens.append(Token("char", source[start:position], start,
                                position, value))
            continue
        if byte == 34:  # string literal
            start = position
            position += 1
            content = bytearray()
            while True:
                if position >= extent or source[position] in (10, 13):
                    raise Reject("UnterminatedString", start)
                byte = source[position]
                if byte == 34:
                    position += 1
                    break
                if byte == 92:
                    position += 1
                    if position >= extent or source[position] in (10, 13):
                        raise Reject("UnterminatedString", start)
                    esc = source[position]
                    if esc in ESCAPES:
                        content.append(ESCAPES[esc])
                    elif esc in (34, 92):
                        content.append(esc)
                    elif esc == 120:
                        if (position + 2 >= extent
                                or _hexdigit(source[position + 1]) < 0
                                or _hexdigit(source[position + 2]) < 0):
                            raise Reject("InvalidEscape", position - 1)
                        content.append(
                            _hexdigit(source[position + 1]) * 16
                            + _hexdigit(source[position + 2]))
                        position += 2
                    else:
                        raise Reject("InvalidEscape", position - 1)
                    position += 1
                    continue
                content.append(byte)
                position += 1
            tokens.append(Token("string", source[start:position], start,
                                position, bytes(content)))
            continue
        matched = False
        for spelling in PUNCT:
            if source[position:position + len(spelling)] == spelling:
                tokens.append(Token("punct", spelling, position,
                                    position + len(spelling)))
                position += len(spelling)
                matched = True
                break
        if matched:
            continue
        raise Reject("InvalidToken", position)
    tokens.append(Token("eof", b"", extent, extent))
    return tokens


# --- Parser -------------------------------------------------------------------

@dataclass
class TypeExpr:
    kind: str              # "i32" | "u8" | "never" | "named" | "array" | "view"
    start: int
    name: bytes = b""
    element: object = None
    length: int = 0
    length_start: int = 0  # the array length literal


@dataclass
class Expr:
    kind: str
    start: int
    args: tuple = ()


@dataclass
class Arm:
    start: int
    pattern: tuple
    continuation: tuple    # ("return", kw_start, Expr|None) or ("expr", Expr)


@dataclass
class StateDecl:
    name: bytes
    start: int
    params: list           # [(name, TypeExpr, name_start)]
    body: list
    terminal: tuple
    body_end: int = 0      # the body's closing `}`


@dataclass
class MachineDecl:
    qualified: bool
    owner: bytes
    name: bytes
    start: int             # first byte of `machine`
    params: list
    returns: object        # TypeExpr | None
    body: list
    terminal: tuple
    states: dict
    state_dups: list = ()  # later-declaration starts of duplicate states
    body_end: int = 0      # the machine body's closing `}`


@dataclass
class DataDecl:
    name: bytes
    start: int
    name_start: int
    fields: list           # [(name, TypeExpr, name_start)]
    cases: list            # [(name, [(pname, TypeExpr, pstart)], name_start)]


@dataclass
class BoundaryDecl:
    name: bytes
    start: int
    machines: list         # [(name, params, TypeExpr|None, name_start,
                         #   machine_start)]


@dataclass
class Statement:
    kind: str              # "let" | "assign" | "call" | "assert"
    start: int
    args: tuple = ()
    end: int = 0           # the terminating `;`


class Parser:
    def __init__(self, tokens, extent):
        self.tokens = tokens
        self.index = 0
        self.extent = extent

    def peek(self):
        return self.tokens[self.index]

    def take(self):
        token = self.tokens[self.index]
        if token.kind == "eof":
            raise Reject("UnexpectedEnd", self.extent)
        self.index += 1
        return token

    def expect(self, text):
        token = self.peek()
        if token.kind == "eof":
            raise Reject("UnexpectedEnd", self.extent)
        spelling = text if isinstance(text, bytes) else text.encode()
        if (token.kind == "punct" and token.text == spelling) or (
                token.kind == "ident" and token.text == spelling
                and spelling in KEYWORDS):
            self.index += 1
            return token
        raise Reject("UnexpectedToken", token.start)

    def expect_ident(self):
        token = self.peek()
        if token.kind == "eof":
            raise Reject("UnexpectedEnd", self.extent)
        if token.kind != "ident" or token.text in KEYWORDS:
            raise Reject("UnexpectedToken", token.start)
        return self.take()

    def is_kw(self, word):
        token = self.peek()
        return token.kind == "ident" and token.text == word

    def is_punct(self, text):
        token = self.peek()
        return token.kind == "punct" and token.text == text

    def parse_program(self):
        declarations = []
        while self.peek().kind != "eof":
            declarations.append(self.parse_declaration())
        if not declarations:
            # program := declaration+; an empty source is an unexpected end
            # at the extent, not a zero-declaration program.
            raise Reject("UnexpectedEnd", self.extent)
        return declarations

    def parse_declaration(self):
        token = self.peek()
        if token.kind == "eof":
            raise Reject("UnexpectedEnd", self.extent)
        if token.text == b"boundary":
            return ("boundary", self.parse_boundary())
        if token.text == b"data":
            return ("data", self.parse_data())
        if token.text == b"machine":
            return ("machine", self.parse_machine())
        raise Reject("UnexpectedToken", token.start)

    def parse_boundary(self):
        start = self.expect(b"boundary").start
        self.expect(b"trait")
        name = self.expect_ident()
        self.expect(b"{")
        machines = []
        while not self.is_punct(b"}"):
            machine_start = self.expect(b"machine").start
            mname = self.expect_ident()
            self.expect(b"(")
            params = []
            if not self.is_punct(b")"):
                params = self.parse_parameters()
            self.expect(b")")
            returns = None
            if self.is_punct(b"->"):
                self.take()
                returns = self.parse_type()
            self.expect(b";")
            machines.append((mname.text, params, returns, mname.start,
                             machine_start))
        self.expect(b"}")
        return BoundaryDecl(name.text, start, machines)

    def parse_data(self):
        start = self.expect(b"data").start
        name = self.expect_ident()
        self.expect(b"{")
        fields = []
        cases = []
        while not self.is_punct(b"}"):
            if self.is_kw(b"case"):
                case_start = self.take().start
                case_name = self.expect_ident()
                payload = []
                if self.is_punct(b"("):
                    self.take()
                    if not self.is_punct(b")"):
                        payload = self.parse_parameters()
                    self.expect(b")")
                self.expect(b";")
                cases.append((case_name.text, payload, case_start))
            else:
                member = self.expect_ident()
                self.expect(b":")
                type_expr = self.parse_type()
                self.expect(b";")
                fields.append((member.text, type_expr, member.start))
        self.expect(b"}")
        return DataDecl(name.text, start, name.start, fields, cases)

    def parse_parameters(self):
        params = [self.parse_parameter()]
        while self.is_punct(b","):
            self.take()
            params.append(self.parse_parameter())
        return params

    def parse_parameter(self):
        name = self.expect_ident()
        self.expect(b":")
        return (name.text, self.parse_type(), name.start)

    def parse_type(self):
        token = self.peek()
        if token.kind == "eof":
            raise Reject("UnexpectedEnd", self.extent)
        if token.kind == "ident" and token.text in (b"i32", b"u8", b"never"):
            self.take()
            return TypeExpr(token.text.decode(), token.start)
        if token.kind == "ident" and token.text not in KEYWORDS:
            self.take()
            return TypeExpr("named", token.start, name=token.text)
        if self.is_punct(b"["):
            self.take()
            element = self.parse_type()
            self.expect(b";")
            count = self.peek()
            if count.kind == "eof":
                raise Reject("UnexpectedEnd", self.extent)
            if count.kind != "int":
                raise Reject("UnexpectedToken", count.start)
            self.take()
            self.expect(b"]")
            return TypeExpr("array", token.start, element=element,
                            length=count.value, length_start=count.start)
        if self.is_punct(b"&"):
            self.take()
            self.expect(b"[")
            element = self.parse_type()
            self.expect(b"]")
            return TypeExpr("view", token.start, element=element)
        raise Reject("UnexpectedToken", token.start)

    def parse_machine(self):
        start = self.expect(b"machine").start
        first = self.expect_ident()
        if self.is_punct(b"::"):
            self.take()
            name = self.expect_ident()
            self.expect(b"(")
            self.expect(b"&")
            self.expect(b"mut")
            if not self.is_kw(b"self"):
                token = self.peek()
                if token.kind == "eof":
                    raise Reject("UnexpectedEnd", self.extent)
                raise Reject("UnexpectedToken", token.start)
            self.take()
            params = []
            if self.is_punct(b","):
                self.take()
                params = self.parse_parameters()
            self.expect(b")")
            returns = self.parse_return_type()
            body, terminal, states, state_dups, body_end = \
                self.parse_machine_body()
            return MachineDecl(True, first.text, name.text, start, params,
                               returns, body, terminal, states, state_dups,
                               body_end)
        self.expect(b"(")
        params = []
        if not self.is_punct(b")"):
            params = self.parse_parameters()
        self.expect(b")")
        returns = self.parse_return_type()
        body, terminal, states, state_dups, body_end = \
            self.parse_machine_body()
        return MachineDecl(False, b"", first.text, start, params,
                           returns, body, terminal, states, state_dups,
                           body_end)

    def parse_return_type(self):
        if self.is_punct(b"->"):
            self.take()
            return self.parse_type()
        return None

    def parse_block(self):
        """statement* terminal? — used for machine bodies and state bodies."""
        self.expect(b"{")
        body = []
        terminal = None
        while not self.is_punct(b"}") and not self.is_kw(b"state"):
            if self.is_kw(b"transition") or self.is_kw(b"return"):
                terminal = self.parse_terminal()
                break
            body.append(self.parse_statement())
        return body, terminal

    def parse_machine_body(self):
        body, terminal = self.parse_block()
        states = {}
        state_dups = []
        while self.is_kw(b"state"):
            state = self.parse_state()
            if state.name in states:
                state_dups.append(state.start)
            else:
                states[state.name] = state
        body_end = self.expect(b"}").start
        return body, terminal, states, state_dups, body_end

    def parse_terminal(self):
        if self.is_kw(b"return"):
            token = self.take()
            value = None
            if not self.is_punct(b";"):
                value = self.parse_expression()
            end = self.expect(b";").start
            return ("return", token.start, value, end)
        start = self.expect(b"transition").start
        subject = self.parse_expression()
        self.expect(b"{")
        arms = []
        if self.is_punct(b"}"):
            raise Reject("UnexpectedToken", self.peek().start)
        while True:
            arm = self.parse_arm()
            arms.append(arm)
            if arm.pattern[0] == "wildcard" and not self.is_punct(b"}"):
                token = self.peek()
                if token.kind == "eof":
                    raise Reject("UnexpectedEnd", self.extent)
                raise Reject("UnexpectedToken", token.start)
            if self.is_punct(b"}"):
                break
        end = self.expect(b"}").start
        return ("transition", start, subject, arms, end)

    def parse_arm(self):
        token = self.peek()
        if token.kind == "eof":
            raise Reject("UnexpectedEnd", self.extent)
        start = token.start
        if token.text == b"_" and token.kind == "ident":
            self.take()
            pattern = ("wildcard", start)
        elif token.kind == "int":
            self.take()
            if token.value > I32_MAX:
                raise Reject("IntegerLiteralOutOfRange", start)
            pattern = ("int", token.value, start)
        elif token.kind == "ident" and token.text in (b"true", b"false"):
            self.take()
            pattern = ("int", 1 if token.text == b"true" else 0, start)
        elif token.kind == "ident" and token.text not in KEYWORDS:
            owner = self.take()
            self.expect(b"::")
            case = self.expect_ident()
            binders = []
            if self.is_punct(b"{"):
                self.take()
                while not self.is_punct(b"}"):
                    binder = self.expect_ident()
                    binders.append((binder.text, binder.start))
                    if self.is_punct(b","):
                        self.take()
                    else:
                        break
                self.expect(b"}")
            pattern = ("case", owner.text, case.text, tuple(binders),
                       start, case.start)
        else:
            raise Reject("UnexpectedToken", start)
        self.expect(b"->")
        if self.is_kw(b"return"):
            keyword = self.take()
            value = None
            if not self.return_is_bare():
                value = self.parse_expression()
            return Arm(start, pattern, ("return", keyword.start, value))
        return Arm(start, pattern, ("expr", self.parse_postfix_expression()))

    def return_is_bare(self):
        """Whether a `return` continuation carries no expression. The contract
        grammar leaves this ambiguous before a following arm; the checked
        evaluator resolves it with a pattern-arrow probe (a `return` is bare
        when the next token is `}` or end of source, or when the tokens ahead
        form `pattern ->`), and this model reproduces that disambiguation so
        both sides read the same program."""
        token = self.peek()
        if token.kind == "eof":
            return True
        if token.kind == "punct" and token.text == b"}":
            return True
        return self.probe_pattern_arrow(self.index) >= 0

    def probe_pattern_arrow(self, index):
        """The token index just past a `pattern ->` prefix, or -1."""
        end = self.probe_pattern_end(index)
        if end < 0:
            return -1
        arrow = self.tokens[end]
        if arrow.kind == "punct" and arrow.text == b"->":
            return end + 1
        return -1

    def probe_pattern_end(self, index):
        """The token index just past a pattern spelled at `index`, or -1."""
        token = self.tokens[index]
        if token.kind == "int":
            return index + 1
        if token.kind == "ident":
            if token.text in (b"true", b"false") or token.text == b"_":
                return index + 1
            if token.text in KEYWORDS:
                return -1
            scope = self.tokens[index + 1]
            if scope.kind != "punct" or scope.text != b"::":
                return -1
            case = self.tokens[index + 2]
            if case.kind != "ident" or case.text in KEYWORDS:
                return -1
            binder = self.tokens[index + 3]
            if binder.kind == "punct" and binder.text == b"{":
                return self.probe_binder_names(index + 4)
            return index + 3
        return -1

    def probe_binder_names(self, index):
        """The token index just past `}` of a binder list at `index`, or -1."""
        token = self.tokens[index]
        if token.kind == "punct" and token.text == b"}":
            return index + 1
        if token.kind != "ident" or token.text in KEYWORDS:
            return -1
        following = self.tokens[index + 1]
        if following.kind == "punct" and following.text == b"}":
            return index + 2
        if following.kind == "punct" and following.text == b",":
            if self.tokens[index + 2].kind == "punct" \
                    and self.tokens[index + 2].text == b"}":
                return -1
            return self.probe_binder_names(index + 2)
        return -1

    def parse_state(self):
        start = self.expect(b"state").start
        name = self.expect_ident()
        self.expect(b"(")
        params = []
        if not self.is_punct(b")"):
            params = self.parse_parameters()
        self.expect(b")")
        body, terminal = self.parse_block()
        body_end = self.expect(b"}").start
        return StateDecl(name.text, start, params, body, terminal,
                         body_end)

    def parse_statement(self):
        token = self.peek()
        if self.is_kw(b"let"):
            self.take()
            name = self.expect_ident()
            self.expect(b":")
            type_expr = self.parse_type()
            self.expect(b"=")
            value = self.parse_expression()
            end = self.expect(b";").start
            return Statement("let", token.start,
                             (name.text, type_expr, value, name.start), end)
        if self.is_kw(b"assert"):
            self.take()
            value = self.parse_expression()
            end = self.expect(b";").start
            return Statement("assert", token.start, (value,), end)
        expression = self.parse_postfix_expression()
        if self.is_punct(b"="):
            self.take()
            value = self.parse_expression()
            end = self.expect(b";").start
            return Statement("assign", expression.start,
                             (expression, value), end)
        end = self.expect(b";").start
        return Statement("call", expression.start, (expression,), end)

    LEVELS = [
        [b"||"], [b"&&"], [b"|"], [b"^"], [b"&"], [b"==", b"!="],
        [b"<", b"<=", b">", b">="], [b"<<", b">>"], [b"+", b"-"],
        [b"*", b"/", b"%"],
    ]

    def parse_expression(self):
        return self.parse_binary(0)

    def parse_binary(self, level):
        if level >= len(self.LEVELS):
            return self.parse_unary()
        left = self.parse_binary(level + 1)
        while self.peek().kind == "punct" \
                and self.peek().text in self.LEVELS[level]:
            operator = self.take()
            right = self.parse_binary(level + 1)
            left = Expr("binary", left.start,
                        (operator.text.decode(), left, right))
        return left

    def parse_unary(self):
        token = self.peek()
        if token.kind == "punct" and token.text == b"-":
            self.take()
            operand_token = self.peek()
            if operand_token.kind == "int" \
                    and operand_token.value == 2147483648:
                # The one magnitude admitted only as the direct operand
                # spelling of unary minus.
                self.take()
                return Expr("int", token.start, (I32_MIN,))
            operand = self.parse_unary()
            return Expr("negate", token.start, (operand,))
        return self.parse_postfix_expression()

    def parse_postfix_expression(self):
        expression = self.parse_primary()
        while True:
            if self.is_punct(b"."):
                self.take()
                member = self.peek()
                if member.kind == "eof":
                    raise Reject("UnexpectedEnd", self.extent)
                if member.kind != "ident":
                    raise Reject("UnexpectedToken", member.start)
                self.take()
                expression = Expr("field", expression.start,
                                  (expression, member.text, member.start))
                continue
            if self.is_punct(b"("):
                self.take()
                arguments = []
                if not self.is_punct(b")"):
                    arguments = self.parse_arguments()
                self.expect(b")")
                expression = Expr("call", expression.start,
                                  (expression, arguments))
                continue
            if self.is_punct(b"["):
                self.take()
                if self.is_punct(b".."):
                    self.take()
                    high = None
                    if not self.is_punct(b"]"):
                        high = self.parse_expression()
                    self.expect(b"]")
                    expression = Expr("slice", expression.start,
                                      (expression, None, high))
                    continue
                index = self.parse_expression()
                if self.is_punct(b".."):
                    self.take()
                    high = None
                    if not self.is_punct(b"]"):
                        high = self.parse_expression()
                    self.expect(b"]")
                    expression = Expr("slice", expression.start,
                                      (expression, index, high))
                    continue
                self.expect(b"]")
                expression = Expr("index", expression.start,
                                  (expression, index))
                continue
            break
        return expression

    def parse_arguments(self):
        arguments = [self.parse_expression()]
        while self.is_punct(b","):
            self.take()
            arguments.append(self.parse_expression())
        return arguments

    def parse_primary(self):
        token = self.peek()
        if token.kind == "eof":
            raise Reject("UnexpectedEnd", self.extent)
        if token.kind == "int":
            self.take()
            if token.value > I32_MAX:
                raise Reject("IntegerLiteralOutOfRange", token.start)
            return Expr("int", token.start, (token.value,))
        if token.kind == "char":
            self.take()
            return Expr("int", token.start, (token.value,))
        if token.kind == "string":
            self.take()
            return Expr("string", token.start, (token.value,))
        if token.kind == "ident":
            if token.text in (b"true", b"false"):
                self.take()
                return Expr("int", token.start,
                            (1 if token.text == b"true" else 0,))
            if token.text == b"self":
                self.take()
                return Expr("self", token.start)
            if token.text in KEYWORDS:
                raise Reject("UnexpectedToken", token.start)
            self.take()
            if self.is_punct(b"::"):
                self.take()
                member = self.expect_ident()
                return Expr("qualified", token.start,
                            (token.text, member.text, member.start))
            return Expr("name", token.start, (token.text,))
        if self.is_punct(b"("):
            self.take()
            inner = self.parse_expression()
            self.expect(b")")
            return inner
        raise Reject("UnexpectedToken", token.start)


# --- Types ---------------------------------------------------------------------
#
# Runtime types are tuples: ("i32",) ("u8",) ("never",) ("record", name)
# ("sum", name) ("array", element, length) ("view", element) ("console",)
# ("resultless",). Names are str for convenience.

I32 = ("i32",)
U8 = ("u8",)
NEVER = ("never",)
CONSOLE = ("console",)
RESULTLESS = ("resultless",)


def norm(type_):
    """Expression-level scalar identity: u8 reads are i32 values."""
    return I32 if type_ == U8 else type_


class Checker:
    def __init__(self, declarations, extent):
        self.extent = extent
        self.records = {}      # str -> [(field_str, type)]
        self.sums = {}         # str -> [(case_str, [(member_str, type)])]
        self.machines = {}     # bytes -> MachineDecl
        self.receivers = {}    # (bytes owner, bytes name) -> MachineDecl
        self.boundary = None
        self.data_names = {}   # bytes -> "record" | "sum"
        self.data_decls = {}   # bytes -> DataDecl
        # Reserved entry duplicates: a duplicate `Main`/`Console` owner, a
        # duplicate `Main::main`, a duplicate `Main.console` field, or a
        # duplicate `Console` member is reserved for the entry judgment when
        # an authored `Main::main` candidate exists (recorded as
        # (duplicate anchor, construct start)); otherwise it is an ordinary
        # DuplicateName.
        self.reserved_dups = []
        # Mixed field/case data declarations are InvalidDataShape
        # candidates inside the formation merge rather than census
        # rejections.
        self.invalid_shapes = []
        boundary_owners = {d.name for k, d in declarations
                           if k == "boundary"}
        for kind, decl in declarations:
            if kind == "boundary":
                if self.boundary is not None or \
                        decl.name in self.data_names:
                    if decl.name == b"Console":
                        self.reserved_dups.append(
                            (decl.start, decl.start))
                        continue
                    raise Reject("DuplicateName", decl.start)
                self.boundary = decl
            elif kind == "data":
                if decl.name in self.data_names or \
                        (self.boundary is not None
                         and decl.name == self.boundary.name):
                    if decl.name == b"Main":
                        self.reserved_dups.append(
                            (decl.start, decl.start))
                        continue
                    raise Reject("DuplicateName", decl.start)
                if decl.fields and decl.cases:
                    # InvalidDataShape is a formation candidate at the
                    # data name, merged with the other formed-type
                    # candidates at minimum offset.
                    self.invalid_shapes.append(
                        ("InvalidDataShape", decl.name_start))
                self.data_names[decl.name] = (
                    "sum" if decl.cases else "record")
                self.data_decls[decl.name] = decl
            else:
                if decl.qualified and decl.owner in boundary_owners:
                    raise Reject("InvalidBoundary", decl.start)
                table = self.receivers if decl.qualified else self.machines
                key = (decl.owner, decl.name) if decl.qualified else decl.name
                if key in table:
                    if key == (b"Main", b"main"):
                        self.reserved_dups.append(
                            (decl.start, decl.start))
                        continue
                    raise Reject("DuplicateName", decl.start)
                table[key] = decl
        # The program-wide type-formation pass: every authored type is
        # checked against its placement — stored fields and payloads admit
        # `u8`; parameters, locals, and returns do not; `never` admits only
        # the return placement; a view escapes only into a parameter or
        # local slot; `Console` names no declared type; an unknown owner is
        # UnknownType; a zero array length is InvalidArrayLength at the
        # length literal. Candidates merge at minimum offset.
        formation = self.invalid_shapes + \
            self.validate_formed_types(declarations)
        if formation:
            reason, offset = min(formation, key=lambda c: c[1])
            raise Reject(reason, offset)
        # Data member types resolve in type-formation order after every
        # declaration name is collected.
        for kind, decl in declarations:
            if kind != "data":
                continue
            if decl.name not in self.data_decls:
                continue  # a deferred reserved duplicate
            if decl.cases:
                seen = set()
                members = []
                for case_name, payload, case_start in decl.cases:
                    if case_name in seen:
                        raise Reject("DuplicateName", case_start)
                    seen.add(case_name)
                    members.append((case_name.decode(), [
                        (n.decode(), self.resolve_type(t))
                        for n, t, _ in payload]))
                self.sums[decl.name.decode()] = members
            else:
                seen = set()
                members = []
                for field_name, type_expr, field_start in decl.fields:
                    if field_name in seen:
                        if decl.name == b"Main" \
                                and field_name == b"console":
                            self.reserved_dups.append(
                                (field_start, field_start))
                            continue
                        raise Reject("DuplicateName", field_start)
                    seen.add(field_name)
                    members.append((field_name.decode(),
                                    self.resolve_type(type_expr)))
                self.records[decl.name.decode()] = members
        # Boundary member types are type-formation candidates too; member
        # identity duplicates are censused here and deferred to the entry
        # judgment when they are reserved.
        if self.boundary is not None:
            seen_members = set()
            for name, params, returns, name_start, machine_start in \
                    self.boundary.machines:
                if name in seen_members:
                    if self.boundary.name == b"Console":
                        self.reserved_dups.append(
                            (name_start, machine_start))
                        continue
                    raise Reject("DuplicateName", name_start)
                seen_members.add(name)
                for _, type_expr, _ in params:
                    self.resolve_type(type_expr)
                if returns is not None and returns.kind != "never":
                    self.resolve_type(returns)

    def validate_formed_types(self, declarations):
        """Collect the (reason, offset) type-formation candidates of every
        authored type under its placement. `Main.console` is the one named
        Console field admission and is exempt from stored-type checks."""
        candidates = []

        def check(type_expr, placement):
            kind = type_expr.kind
            if kind == "i32":
                return
            if kind == "u8":
                if placement in ("parameter", "local", "return"):
                    candidates.append(("TypeMismatch", type_expr.start))
                return
            if kind == "never":
                if placement != "return":
                    candidates.append(("TypeMismatch", type_expr.start))
                return
            if kind == "named":
                if type_expr.name == b"Console":
                    candidates.append(("InvalidEntry", type_expr.start))
                elif type_expr.name not in self.data_names:
                    candidates.append(("UnknownType", type_expr.start))
                return
            if kind == "array":
                check(type_expr.element, "nested")
                if type_expr.length == 0:
                    candidates.append(("InvalidArrayLength",
                                       type_expr.length_start))
                return
            # view
            if placement in ("stored", "return", "nested"):
                candidates.append(("EscapingView", type_expr.start))
                return
            check(type_expr.element, "nested")

        def check_body(machine):
            for statement in machine.body:
                if statement.kind == "let":
                    check(statement.args[1], "local")
            for state in machine.states.values():
                for _, type_expr, _ in state.params:
                    check(type_expr, "parameter")
                for statement in state.body:
                    if statement.kind == "let":
                        check(statement.args[1], "local")

        for kind, decl in declarations:
            if kind == "boundary":
                for _, params, returns, _, _ in decl.machines:
                    for _, type_expr, _ in params:
                        check(type_expr, "parameter")
                    if returns is not None:
                        check(returns, "return")
            elif kind == "data":
                for field_name, type_expr, _ in decl.fields:
                    if decl.name == b"Main" and field_name == b"console":
                        continue
                    check(type_expr, "stored")
                for _, payload, _ in decl.cases:
                    for _, type_expr, _ in payload:
                        check(type_expr, "stored")
            else:
                for _, type_expr, _ in decl.params:
                    check(type_expr, "parameter")
                if decl.returns is not None:
                    check(decl.returns, "return")
                check_body(decl)
        return candidates

    def resolve_type(self, type_expr):
        if type_expr.kind == "i32":
            return I32
        if type_expr.kind == "u8":
            return U8
        if type_expr.kind == "never":
            raise Reject("TypeMismatch", type_expr.start)
        if type_expr.kind == "named":
            if type_expr.name in self.data_names:
                return (self.data_names[type_expr.name],
                        type_expr.name.decode())
            if type_expr.name == b"Console":
                return CONSOLE
            raise Reject("UnknownType", type_expr.start)
        if type_expr.kind == "array":
            if type_expr.length == 0:
                raise Reject("InvalidArrayLength", type_expr.length_start)
            return ("array", self.resolve_type(type_expr.element),
                    type_expr.length)
        if type_expr.kind == "view":
            return ("view", self.resolve_type(type_expr.element))
        raise ModelExcluded(f"type form {type_expr.kind}")

    def check_program(self):
        # The entry-shape judgment is the final type-formation subjudgment.
        # It first asks whether an authored `Main::main` candidate exists;
        # absent one, MissingEntry at the source extent is the sole verdict
        # and absent supporting components add no candidates.
        main = self.receivers.get((b"Main", b"main"))
        if main is None:
            if self.reserved_dups:
                anchor, _ = min(self.reserved_dups)
                raise Reject("DuplicateName", anchor)
            raise Reject("MissingEntry", self.extent)
        # Once the candidate exists, every malformed, duplicate, or
        # competing entry and supporting component is InvalidEntry.
        if self.reserved_dups:
            raise Reject("InvalidEntry",
                         min(start for _, start in self.reserved_dups))
        if self.boundary is None:
            raise Reject("InvalidEntry", self.extent)
        if self.boundary.name != b"Console":
            raise Reject("InvalidEntry", self.boundary.start)
        expected = {
            b"exit_process": ([I32], NEVER),
            b"write_byte": ([I32], RESULTLESS),
            b"read_byte": ([], I32),
            b"write_line": ([("view", U8)], RESULTLESS),
        }
        members = {}
        for name, params, returns, _, machine_start in \
                self.boundary.machines:
            want = expected.get(name)
            got_params = [self.resolve_type(t) for _, t, _ in params]
            got_returns = RESULTLESS if returns is None else (
                NEVER if returns.kind == "never"
                else self.resolve_type(returns))
            if want is None or want != (got_params, got_returns):
                raise Reject("InvalidEntry", machine_start)
            members[name] = True
        if set(members) != set(expected):
            # A required but absent supporting component anchors at extent.
            raise Reject("InvalidEntry", self.extent)
        if self.data_names.get(b"Main") != "record":
            decl = self.data_decls.get(b"Main")
            raise Reject("InvalidEntry",
                         self.extent if decl is None else decl.start)
        console_count = 0
        for field_name, type_expr, field_start in \
                self.data_decls[b"Main"].fields:
            if self.resolve_type(type_expr) == CONSOLE:
                if field_name != b"console":
                    raise Reject("InvalidEntry", field_start)
                console_count += 1
        if console_count != 1:
            raise Reject("InvalidEntry", self.extent)
        if main.params or main.returns is not None:
            raise Reject("InvalidEntry", main.start)
        for machine in list(self.machines.values()) + \
                list(self.receivers.values()):
            self.check_machine(machine)

    def machine_result(self, decl):
        if decl.returns is None:
            return RESULTLESS
        if decl.returns.kind == "never":
            return NEVER
        return self.resolve_type(decl.returns)

    def owner_type(self, machine):
        if machine.owner == b"Console":
            raise Reject("InvalidBoundary", machine.start)
        if machine.owner not in self.data_names:
            raise Reject("UnknownName", machine.start)
        return (self.data_names[machine.owner], machine.owner.decode())

    def check_machine(self, machine):
        owner_type = None
        scope = {}
        if machine.qualified:
            owner_type = self.owner_type(machine)
            scope[b"self"] = owner_type
        return_type = self.machine_result(machine)
        for name, type_expr, name_start in machine.params:
            if name in scope:
                raise Reject("DuplicateName", name_start)
            scope[name] = self.resolve_type(type_expr)
        if machine.state_dups:
            raise Reject("DuplicateName", min(machine.state_dups))
        self.check_block(machine.body, machine.terminal, machine, scope,
                         owner_type, return_type, machine.body_end)
        for state in machine.states.values():
            state_scope = dict(scope)
            for name, type_expr, name_start in state.params:
                if name in state_scope:
                    raise Reject("DuplicateName", name_start)
                state_scope[name] = self.resolve_type(type_expr)
            self.check_block(state.body, state.terminal, machine,
                             state_scope, owner_type, return_type,
                             state.body_end)

    def check_block(self, body, terminal, machine, scope, owner_type,
                    return_type, body_end):
        # `body_end` is the body's closing `}` offset — the authored
        # anchor for an incompatible falloff. The evaluator collects
        # candidates and reports the minimum offset, so an after-never
        # statement's children are checked before its own delimiter.
        block_scope = dict(scope)
        after_never = False
        for statement in body:
            if after_never:
                self.check_statement(statement, machine, block_scope,
                                     owner_type)
                raise Reject("InvalidTerminal", statement.end)
            if self.check_statement(statement, machine, block_scope,
                                    owner_type) == NEVER:
                after_never = True
        if terminal is None:
            # A never statement supplies the block's exit; otherwise an
            # absent terminal is Falloff, admitted only by a resultless
            # machine and anchored at the closing `}`.
            if not after_never and return_type != RESULTLESS:
                raise Reject("TypeMismatch", body_end)
            return
        if after_never:
            # The terminal's children still check and can win on minimum
            # offset; a clean construct anchors InvalidTerminal at its
            # terminating `;`/`}`.
            if terminal[0] == "return":
                _, kw_start, value, _ = terminal
                self.check_return(kw_start, value, machine, block_scope,
                                  owner_type, return_type)
            else:
                _, _, subject, arms, _ = terminal
                subject_type = self.check_expr(subject, machine,
                                               block_scope, owner_type)
                self.check_transition(subject, subject_type, arms,
                                      machine, block_scope, owner_type,
                                      return_type, body_end)
            raise Reject("InvalidTerminal", terminal[-1])
        if terminal[0] == "return":
            _, kw_start, value, _ = terminal
            self.check_return(kw_start, value, machine, block_scope,
                              owner_type, return_type)
            return
        _, start, subject, arms, _ = terminal
        subject_type = self.check_expr(subject, machine, block_scope,
                                       owner_type)
        self.check_transition(subject, subject_type, arms, machine,
                              block_scope, owner_type, return_type,
                              body_end)

    def check_return(self, kw_start, value, machine, scope, owner_type,
                     return_type):
        if value is None:
            # ReturnNone: admitted only by a resultless machine; an
            # absent required value anchors at `return`.
            if return_type != RESULTLESS:
                raise Reject("TypeMismatch", kw_start)
            return
        actual = self.check_expr(value, machine, scope, owner_type)
        if actual == NEVER:
            raise Reject("InvalidTerminal", value.start)
        if actual == RESULTLESS:
            raise Reject("TypeMismatch", value.start)
        # A return value is a strict type-equality relation, not the
        # assignment relation; the compared side is the zero-extended
        # read type, so a u8 place read satisfies `-> i32`.
        if norm(actual) != return_type:
            raise Reject("TypeMismatch", value.start)

    def check_statement(self, statement, machine, scope, owner_type):
        if statement.kind == "let":
            name, type_expr, value, name_start = statement.args
            # Local census is declaration collection: a duplicate binds
            # before the initializer's body checking and anchors at the
            # later declaration's first byte (the `let` keyword).
            if name in scope:
                raise Reject("DuplicateName", statement.start)
            declared = self.resolve_type(type_expr)
            actual = self.check_expr(value, machine, scope, owner_type)
            if actual == NEVER:
                raise Reject("InvalidTerminal", value.start)
            self.require_equal(declared, actual, value.start)
            scope[name] = declared
            return
        if statement.kind == "assign":
            place, value = statement.args
            place_type, is_place = self.check_place(place, machine, scope,
                                                    owner_type)
            if not is_place:
                raise Reject("InvalidPlace", place.start)
            actual = self.check_expr(value, machine, scope, owner_type)
            if actual == NEVER:
                raise Reject("InvalidTerminal", value.start)
            self.require_storable(place_type, actual, value.start)
            return
        if statement.kind == "assert":
            (value,) = statement.args
            actual = self.check_expr(value, machine, scope, owner_type)
            if actual == NEVER:
                raise Reject("InvalidTerminal", value.start)
            if norm(actual) != I32:
                raise Reject("TypeMismatch", value.start)
            return
        (call,) = statement.args
        return self.check_expr(call, machine, scope, owner_type)

    def require_equal(self, expected, actual, anchor):
        """The storedEstablishment=0 relation: strict type equality on the
        read type (`norm` widens a u8 place read to i32, matching the
        evaluator's zero-extended read type)."""
        if norm(actual) != expected:
            raise Reject("TypeMismatch", anchor)

    def require_storable(self, expected, actual, anchor):
        """The storedEstablishment=1 relation: a u8 store admits any i32
        value for the later ByteRange check; every other expected type is
        strict equality."""
        if expected == U8 and norm(actual) == I32:
            return
        self.require_equal(expected, actual, anchor)

    def check_transition(self, subject, subject_type, arms, machine, scope,
                         owner_type, return_type, body_end):
        is_sum = subject_type[0] == "sum"
        if norm(subject_type) != I32 and not is_sum:
            raise Reject("TypeMismatch", subject.start)
        seen_scalar = set()
        seen_cases = set()
        wildcard = False
        for arm in arms:
            pattern = arm.pattern
            binders = {}
            if pattern[0] == "wildcard":
                wildcard = True
            elif pattern[0] == "int":
                if is_sum:
                    raise Reject("TypeMismatch", pattern[2])
                if pattern[1] in seen_scalar:
                    raise Reject("DuplicatePattern", pattern[2])
                seen_scalar.add(pattern[1])
            else:
                _, owner, case, names, pstart, cstart = pattern
                if not is_sum:
                    if owner in self.data_names or owner == b"Console":
                        raise Reject("TypeMismatch", pstart)
                    raise Reject("UnknownName", pstart)
                if owner.decode() != subject_type[1]:
                    if owner in self.data_names or owner == b"Console":
                        raise Reject("TypeMismatch", pstart)
                    raise Reject("UnknownName", pstart)
                payload = dict(self.sums[subject_type[1]]).get(case.decode())
                if payload is None:
                    raise Reject("UnknownName", cstart)
                if case.decode() in seen_cases:
                    raise Reject("DuplicatePattern", pstart)
                seen_cases.add(case.decode())
                if len(names) != len(payload):
                    raise Reject("ArityMismatch", pstart)
                binders = {}
                for (binder, binder_start), (_, type_) in \
                        zip(names, payload):
                    if binder in binders:
                        raise Reject("DuplicateName", binder_start)
                    binders[binder] = type_
            continuation = arm.continuation
            arm_scope = dict(scope)
            arm_scope.update(binders)
            if continuation[0] == "return":
                _, kw_start, value = continuation
                self.check_return(kw_start, value, machine, arm_scope,
                                  owner_type, return_type)
            else:
                self.check_continuation(continuation[1], machine, arm_scope,
                                        owner_type, return_type, body_end)
        if is_sum and not wildcard:
            complete = {case for case, _ in self.sums[subject_type[1]]}
            if seen_cases != complete:
                raise Reject("NonexhaustiveSum", subject.start)

    def check_continuation(self, expr, machine, scope, owner_type,
                           return_type, body_end):
        """A continuation resolves uniquely to a state transfer, a machine
        call, or a return. The machine call's own result category becomes
        the block-exit effect: a value return anchors TypeMismatch at the
        continuation expression, a resultless call becomes Falloff on
        return and is admitted only by a resultless machine (anchored at
        the body's closing `}`), and a never call is NoNormalReturn."""
        if expr.kind != "call":
            if expr.kind == "name":
                name = expr.args[0]
                if name in machine.states or name in self.machines:
                    raise Reject("InvalidControlTarget", expr.start)
                if name in scope:
                    # A bare local is a completed value, not a control
                    # target.
                    raise Reject("InvalidControlTarget", expr.start)
                raise Reject("UnknownName", expr.start)
            raise Reject("InvalidControlTarget", expr.start)
        callee, args = expr.args
        if callee.kind == "name":
            name = callee.args[0]
            in_states = name in machine.states
            in_machines = name in self.machines
            if in_states and in_machines:
                raise Reject("InvalidControlTarget", expr.start)
            if not in_states and not in_machines:
                if name in scope:
                    # Calling a completed local value is not a control
                    # target.
                    raise Reject("InvalidControlTarget", expr.start)
                raise Reject("UnknownName", callee.start)
            if in_states:
                self.check_call_args(machine.states[name].params, args,
                                     machine, scope, owner_type, expr.start)
                return
            decl = self.machines[name]
            self.check_call_args(decl.params, args, machine, scope,
                                 owner_type, expr.start)
            self.check_continuation_effect(self.machine_result(decl),
                                           return_type, expr, body_end)
            return
        if callee.kind == "field":
            base, member, member_start = callee.args
            base_type, base_place = self.check_typed(base, machine, scope,
                                                     owner_type)
            if base_type == CONSOLE:
                entry = {n: (p, r) for n, p, r, _, _ in
                         self.boundary.machines}.get(member)
                if entry is None:
                    raise Reject("UnknownName", member_start)
                params, returns = entry
                self.check_call_args(params, args, machine, scope,
                                     owner_type, expr.start)
                result = RESULTLESS if returns is None else (
                    NEVER if returns.kind == "never"
                    else self.resolve_type(returns))
                self.check_continuation_effect(result, return_type, expr,
                                               body_end)
                return
            if base_type[0] in ("record", "sum"):
                if not base_place:
                    raise Reject("InvalidPlace", base.start)
                decl = self.receivers.get(
                    (base_type[1].encode(), member))
                if decl is None:
                    # A known data member spelled as a callee is still not
                    # a control target; an unknown member is UnknownName.
                    if base_type[0] == "record":
                        members = dict(self.records[base_type[1]])
                    else:
                        members = {case for case, _
                                   in self.sums[base_type[1]]}
                    if member.decode() in members:
                        raise Reject("InvalidControlTarget", expr.start)
                    raise Reject("UnknownName", member_start)
                self.check_call_args(decl.params, args, machine, scope,
                                     owner_type, expr.start)
                self.check_continuation_effect(self.machine_result(decl),
                                               return_type, expr,
                                               body_end)
                return
            raise Reject("TypeMismatch", expr.start)
        raise Reject("InvalidControlTarget", expr.start)

    def check_continuation_effect(self, result, return_type, expr,
                                  body_end):
        if result == NEVER:
            return
        if result == RESULTLESS:
            # Falloff-on-return: admitted only in a resultless machine.
            if return_type != RESULTLESS:
                raise Reject("TypeMismatch", body_end)
            return
        raise Reject("TypeMismatch", expr.start)

    def check_call_args(self, params, args, machine, scope, owner_type,
                        anchor):
        if len(params) != len(args):
            raise Reject("ArityMismatch", anchor)
        for (_, type_expr, _), argument in zip(params, args):
            declared = self.resolve_type(type_expr)
            actual = self.check_expr(argument, machine, scope, owner_type)
            if actual == NEVER:
                raise Reject("InvalidTerminal", argument.start)
            self.require_equal(declared, actual, argument.start)

    # Expression checking returns the value type. check_place additionally
    # reports whether the expression carries an assignable place.

    def check_expr(self, expr, machine, scope, owner_type):
        type_, _ = self.check_typed(expr, machine, scope, owner_type)
        return type_

    def check_place(self, expr, machine, scope, owner_type):
        return self.check_typed(expr, machine, scope, owner_type)

    def check_typed(self, expr, machine, scope, owner_type):
        kind = expr.kind
        if kind == "int":
            return I32, False
        if kind == "string":
            return ("view", U8), False
        if kind == "self":
            if owner_type is None:
                raise Reject("UnknownName", expr.start)
            return owner_type, True
        if kind == "name":
            name = expr.args[0]
            if name in scope:
                return scope[name], True
            if name in self.machines or name in machine.states:
                raise Reject("TypeMismatch", expr.start)
            raise Reject("UnknownName", expr.start)
        if kind == "qualified":
            owner, member, member_start = expr.args
            if owner in self.data_names and \
                    self.data_names[owner] == "sum":
                for case, payload in self.sums[owner.decode()]:
                    if case == member.decode():
                        if payload:
                            raise Reject("ArityMismatch", expr.start)
                        return ("sum", owner.decode()), False
                raise Reject("UnknownName", member_start)
            raise Reject("UnknownName", expr.start)
        if kind == "negate":
            (operand,) = expr.args
            actual = self.check_expr(operand, machine, scope, owner_type)
            if norm(actual) != I32:
                raise Reject("TypeMismatch", expr.start)
            return I32, False
        if kind == "binary":
            operator, left, right = expr.args
            left_type = self.check_expr(left, machine, scope, owner_type)
            if left_type == NEVER:
                raise Reject("InvalidTerminal", left.start)
            right_type = self.check_expr(right, machine, scope, owner_type)
            if right_type == NEVER:
                raise Reject("InvalidTerminal", right.start)
            if norm(left_type) != I32 or norm(right_type) != I32:
                raise Reject("TypeMismatch", expr.start)
            return I32, False
        if kind == "field":
            base, member, member_start = expr.args
            base_type, base_place = self.check_typed(base, machine, scope,
                                                   owner_type)
            if base_type == CONSOLE:
                raise Reject("TypeMismatch", expr.start)
            if base_type[0] == "record":
                fields = dict(self.records[base_type[1]])
                if member.decode() not in fields:
                    raise Reject("UnknownName", member_start)
                return fields[member.decode()], base_place
            if base_type[0] in ("array", "view"):
                if member == b"len":
                    return I32, False
                if member == b"as_slice":
                    if base_type[0] != "array":
                        raise Reject("TypeMismatch", expr.start)
                    if not base_place:
                        raise Reject("InvalidPlace", base.start)
                    return ("view", base_type[1]), False
                raise Reject("UnknownName", member_start)
            raise Reject("TypeMismatch", expr.start)
        if kind == "index":
            base, index = expr.args
            base_type, base_place = self.check_typed(base, machine, scope,
                                                   owner_type)
            index_type = self.check_expr(index, machine, scope, owner_type)
            if index_type == NEVER:
                raise Reject("InvalidTerminal", index.start)
            if norm(index_type) != I32:
                raise Reject("TypeMismatch", expr.start)
            if base_type[0] == "array":
                return base_type[1], base_place
            if base_type[0] == "view":
                return base_type[1], False
            raise Reject("TypeMismatch", expr.start)
        if kind == "slice":
            base, low, high = expr.args
            base_type, _ = self.check_typed(base, machine, scope, owner_type)
            if base_type[0] not in ("array", "view"):
                raise Reject("TypeMismatch", expr.start)
            for bound in (low, high):
                if bound is not None:
                    bound_type = self.check_expr(bound, machine, scope,
                                                 owner_type)
                    if bound_type == NEVER:
                        raise Reject("InvalidTerminal", bound.start)
                    if norm(bound_type) != I32:
                        raise Reject("TypeMismatch", expr.start)
            return ("view", base_type[1]), False
        if kind == "call":
            callee, arguments = expr.args
            return self.check_call(expr, callee, arguments, machine, scope,
                                   owner_type), False
        raise ModelExcluded(f"expression {kind}")

    def check_call(self, expr, callee, arguments, machine, scope,
                   owner_type):
        if callee.kind == "name":
            name = callee.args[0]
            if name in machine.states:
                raise Reject("InvalidControlTarget", expr.start)
            if name not in self.machines:
                if name in scope:
                    # Calling a completed local value is a value call —
                    # TypeMismatch at the call start, not UnknownName.
                    raise Reject("TypeMismatch", expr.start)
                raise Reject("UnknownName", callee.start)
            decl = self.machines[name]
            self.check_call_args(decl.params, arguments, machine, scope,
                                 owner_type, expr.start)
            return self.machine_result(decl)
        if callee.kind == "qualified":
            owner, member, member_start = callee.args
            if owner in self.data_names and \
                    self.data_names[owner] == "sum":
                for case, payload in self.sums[owner.decode()]:
                    if case == member.decode():
                        if len(payload) != len(arguments):
                            raise Reject("ArityMismatch", expr.start)
                        for (_, ptype), argument in zip(payload, arguments):
                            actual = self.check_expr(
                                argument, machine, scope, owner_type)
                            if actual == NEVER:
                                raise Reject("InvalidTerminal",
                                             argument.start)
                            # Payload members are established storage: a
                            # u8 member admits an i32 argument for the
                            # later ByteRange check.
                            self.require_storable(ptype, actual,
                                                  argument.start)
                        return ("sum", owner.decode())
                raise Reject("UnknownName", member_start)
            if owner in self.data_names:
                raise Reject("UnknownName", member_start)
            raise Reject("UnknownName", expr.start)
        if callee.kind == "field":
            base, member, member_start = callee.args
            base_type, base_place = self.check_typed(base, machine, scope,
                                                   owner_type)
            if base_type == CONSOLE:
                entry = {n: (p, r) for n, p, r, _, _ in
                         self.boundary.machines}.get(member)
                if entry is None:
                    raise Reject("UnknownName", member_start)
                params, returns = entry
                self.check_call_args(params, arguments, machine, scope,
                                     owner_type, expr.start)
                if returns is None:
                    return RESULTLESS
                if returns.kind == "never":
                    return NEVER
                return self.resolve_type(returns)
            if base_type[0] in ("record", "sum"):
                if not base_place:
                    raise Reject("InvalidPlace", base.start)
                decl = self.receivers.get(
                    (base_type[1].encode(), member))
                if decl is None:
                    # A known data member spelled as a callee produces a
                    # value call — TypeMismatch at the call start; an
                    # unknown member is UnknownName.
                    if base_type[0] == "record":
                        members = dict(self.records[base_type[1]])
                    else:
                        members = {case for case, _
                                   in self.sums[base_type[1]]}
                    if member.decode() in members:
                        raise Reject("TypeMismatch", expr.start)
                    raise Reject("UnknownName", member_start)
                self.check_call_args(decl.params, arguments, machine,
                                     scope, owner_type, expr.start)
                return self.machine_result(decl)
            raise Reject("TypeMismatch", expr.start)
        raise Reject("TypeMismatch", expr.start)


# --- Evaluator -----------------------------------------------------------------
#
# Values: scalars are ints; records are ["record", name, {field: value}];
# arrays are ["array", element_type, [values]]; sums are
# ("sum", owner, case, {member: value}); views are
# ("view", element_type, backing, low, high) where backing is a Python list
# or bytes; the console field is ("console",). Places are (root, path) where
# root is the env dict or record/array container and path is a tuple of
# str/int keys. Aggregates are copied on every load so value semantics never
# alias.


def copy_value(value):
    if isinstance(value, int):
        return value
    if value[0] == "record":
        return ["record", value[1],
                {k: copy_value(v) for k, v in value[2].items()}]
    if value[0] == "array":
        return ["array", value[1], [copy_value(v) for v in value[2]]]
    if value[0] == "sum":
        return ("sum", value[1], value[2],
                {k: copy_value(v) for k, v in value[3].items()})
    if value[0] == "view":
        return value
    if value[0] == "console":
        return value
    raise ModelExcluded("value form")


class Env:
    """One activation's bindings: names to values plus their declared types."""

    def __init__(self):
        self.values = {}
        self.types = {}

    def derive(self, names, types):
        child = Env()
        child.values = dict(self.values)
        child.types = dict(self.types)
        child.values.update(names)
        child.types.update(types)
        return child


class Place:
    __slots__ = ("root", "path")

    def __init__(self, root, path=()):
        self.root = root
        self.path = path


def place_get(place):
    node = place.root
    for key in place.path:
        if isinstance(node, Env):
            node = node.values[key]
        elif isinstance(node, list) and node[0] == "record":
            node = node[2][key]
        elif isinstance(node, list) and node[0] == "array":
            node = node[2][key]
        else:
            raise ModelExcluded("place path")
    return node


def place_set(place, value):
    node = place.root
    for key in place.path[:-1]:
        if isinstance(node, Env):
            node = node.values[key]
        elif isinstance(node, list) and node[0] == "record":
            node = node[2][key]
        elif isinstance(node, list) and node[0] == "array":
            node = node[2][key]
        else:
            raise ModelExcluded("place path")
    last = place.path[-1]
    if isinstance(node, Env):
        node.values[last] = value
    elif isinstance(node, list) and node[0] in ("record", "array"):
        node[2][last] = value
    else:
        raise ModelExcluded("place store")


def place_type(place, checker):
    """The declared type at a place, walked from the root's type table."""
    node = place.root
    if isinstance(node, Env):
        type_ = node.types[place.path[0]]
        rest = place.path[1:]
    elif isinstance(node, list) and node[0] == "record":
        type_ = ("record", node[1])
        rest = place.path
    else:
        raise ModelExcluded("place root")
    for key in rest:
        if type_[0] == "record":
            type_ = dict(checker.records[type_[1]])[key]
        elif type_[0] == "array":
            type_ = type_[1]
        else:
            raise ModelExcluded("place type path")
    return type_


def view_bytes(value):
    _, _, backing, low, high = value
    if isinstance(backing, bytes):
        return backing[low:high]
    return bytes(v & 0xFF for v in backing[low:high])


class Evaluator:
    def __init__(self, checker, stdin):
        self.checker = checker
        self.stdin = stdin
        self.stdin_index = 0
        self.stdout = bytearray()

    def zero(self, type_):
        if type_ in (I32, U8):
            return 0
        if type_[0] == "array":
            return ["array", type_,
                    [self.zero(type_[1]) for _ in range(type_[2])]]
        if type_[0] == "record":
            return ["record", type_[1],
                    {n: self.zero(t)
                     for n, t in self.checker.records[type_[1]]}]
        if type_[0] == "sum":
            case, payload = self.checker.sums[type_[1]][0]
            return ("sum", type_[1], case,
                    {n: self.zero(t) for n, t in payload})
        raise ModelExcluded(f"zero for {type_}")

    def run(self):
        main = self.checker.receivers[(b"Main", b"main")]
        fields = {"console": ("console",)}
        for name, type_ in self.checker.records["Main"]:
            if name != "console":
                fields[name] = self.zero(type_)
        storage = ["record", "Main", fields]
        env = Env()
        receiver = Place(storage)
        self.call_machine(main, receiver, [])
        # Resultless falloff from main: exit zero.

    def call_machine(self, decl, receiver, args):
        env = Env()
        if decl.qualified:
            env.values[b"self"] = receiver
            env.types[b"self"] = (self.checker.data_names[decl.owner],
                                  decl.owner.decode())
        for (name, type_expr, _), value in zip(decl.params, args):
            env.values[name] = copy_value(value)
            env.types[name] = self.checker.resolve_type(type_expr)
        outcome = self.exec_block(decl.body, decl.terminal, decl, env,
                                  receiver)
        if decl.returns is not None and decl.returns.kind != "never":
            return outcome[1]
        return None

    def exec_block(self, body, terminal, machine, env, receiver):
        for statement in body:
            self.exec_statement(statement, machine, env, receiver)
        if terminal is None:
            return ("falloff",)
        if terminal[0] == "return":
            _, _, value, _ = terminal
            return ("return",
                    None if value is None
                    else self.eval(value, machine, env, receiver))
        _, _, subject, arms, _ = terminal
        while True:
            selected = self.eval(subject, machine, env, receiver)
            arm = self.select_arm(selected, arms)
            arm_env = env
            if arm.pattern[0] == "case":
                payload = arm.pattern[3]
                members = dict(self.checker.sums[selected[1]])[
                    arm.pattern[2].decode()]
                names = {}
                types = {}
                for (binder, _), (member, mtype) in zip(payload, members):
                    names[binder] = copy_value(selected[3][member])
                    types[binder] = mtype
                arm_env = env.derive(names, types)
            continuation = arm.continuation
            if continuation[0] == "return":
                _, _, value = continuation
                return ("return",
                        None if value is None
                        else self.eval(value, machine, arm_env, receiver))
            outcome = self.eval_continuation(continuation[1], machine,
                                             arm_env, receiver)
            if outcome[0] == "state":
                _, state, bound, bound_types = outcome
                env = env.derive(bound, bound_types)
                for statement in state.body:
                    self.exec_statement(statement, machine, env, receiver)
                if state.terminal is None:
                    return ("falloff",)
                if state.terminal[0] == "return":
                    _, _, value, _ = state.terminal
                    return ("return",
                            None if value is None
                            else self.eval(value, machine, env, receiver))
                _, _, subject, arms, _ = state.terminal
                continue
            # A resultless or never machine continuation: the call already
            # ran; this machine's invocation falls off.
            return ("falloff",)

    def select_arm(self, subject_value, arms):
        wildcard = None
        for arm in arms:
            pattern = arm.pattern
            if pattern[0] == "wildcard":
                wildcard = arm
                continue
            if pattern[0] == "int":
                if isinstance(subject_value, int) \
                        and subject_value == pattern[1]:
                    return arm
            elif isinstance(subject_value, tuple) \
                    and subject_value[0] == "sum" \
                    and subject_value[2] == pattern[2].decode():
                return arm
        if wildcard is not None:
            return wildcard
        raise Trap("NonExhaustiveTransition")

    def eval_continuation(self, expr, machine, env, receiver):
        if expr.kind != "call":
            raise ModelExcluded("non-call continuation at runtime")
        callee, arguments = expr.args
        args = [self.eval(a, machine, env, receiver) for a in arguments]
        if callee.kind == "name":
            name = callee.args[0]
            if name in machine.states:
                state = machine.states[name]
                bound = {}
                bound_types = {}
                for (pname, type_expr, _), value in zip(state.params,
                                                        args):
                    bound[pname] = copy_value(value)
                    bound_types[pname] = self.checker.resolve_type(type_expr)
                return ("state", state, bound, bound_types)
            decl = self.checker.machines[name]
            self.call_machine(decl, None, args)
            return ("machine",)
        if callee.kind == "field":
            base, member, _ = callee.args
            target = self.eval_place(base, machine, env, receiver)
            base_value = place_get(target)
            if base_value[0] == "console":
                self.call_console(member, args)
                return ("machine",)
            decl = self.checker.receivers[
                (base_value[1].encode(), member)]
            self.call_machine(decl, target, args)
            return ("machine",)
        raise ModelExcluded("continuation callee")

    def exec_statement(self, statement, machine, env, receiver):
        if statement.kind == "let":
            name, type_expr, value, _ = statement.args
            declared = self.checker.resolve_type(type_expr)
            result = self.eval(value, machine, env, receiver)
            env.values[name] = self.store_convert(declared, result)
            env.types[name] = declared
            return
        if statement.kind == "assign":
            place_expr, value_expr = statement.args
            place, target_type = self.eval_assign_place(place_expr, machine,
                                                        env, receiver)
            value = self.eval(value_expr, machine, env, receiver)
            place_set(place, self.store_convert(target_type, value))
            return
        if statement.kind == "assert":
            (value_expr,) = statement.args
            value = self.eval(value_expr, machine, env, receiver)
            if value not in (0, 1):
                raise Trap("NonBoolean")
            if value == 0:
                raise Trap("Assertion")
            return
        (call,) = statement.args
        self.eval(call, machine, env, receiver)

    def store_convert(self, target_type, value):
        if target_type == U8:
            if not 0 <= value <= 255:
                raise Trap("ByteRange")
            return value
        return copy_value(value)

    def eval_assign_place(self, expr, machine, env, receiver):
        """Destination evaluation for `place = value`: bases and indexes run
        before the right side, with the bounds check first."""
        if expr.kind == "index":
            base, was_place = self.eval_place_or_value(expr.args[0], machine,
                                                     env, receiver)
            index = self.eval(expr.args[1], machine, env, receiver)
            value = place_get(base) if was_place else base
            if value[0] == "array":
                length = len(value[2])
                element_type = value[1][1]
            elif value[0] == "view":
                length = value[4] - value[3]
                element_type = value[1]
            else:
                raise ModelExcluded("indexed store base")
            if not 0 <= index < length:
                raise Trap("Bounds")
            if was_place and value[0] == "array":
                return Place(base.root, base.path + (index,)), element_type
            raise ModelExcluded("indexed store through a non-place")
        place = self.eval_place(expr, machine, env, receiver)
        return place, place_type(place, self.checker)

    def eval_place_or_value(self, expr, machine, env, receiver):
        if expr.kind in ("name", "self", "field", "index"):
            try:
                return self.eval_place(expr, machine, env, receiver), True
            except ModelExcluded:
                pass
        return self.eval(expr, machine, env, receiver), False

    def eval_place(self, expr, machine, env, receiver):
        if expr.kind == "name":
            return Place(env, (expr.args[0],))
        if expr.kind == "self":
            if receiver is None:
                raise ModelExcluded("self outside a receiver machine")
            return receiver
        if expr.kind == "field":
            base = self.eval_place(expr.args[0], machine, env, receiver)
            member = expr.args[1].decode()
            value = place_get(base)
            if value[0] != "record":
                raise ModelExcluded("field place on non-record")
            return Place(base.root, base.path + (member,))
        if expr.kind == "index":
            base = self.eval_place(expr.args[0], machine, env, receiver)
            index = self.eval(expr.args[1], machine, env, receiver)
            value = place_get(base)
            if value[0] != "array":
                raise ModelExcluded("index place on non-array")
            if not 0 <= index < len(value[2]):
                raise Trap("Bounds")
            return Place(base.root, base.path + (index,))
        raise ModelExcluded(f"place {expr.kind}")

    def eval(self, expr, machine, env, receiver):
        kind = expr.kind
        if kind == "int":
            return expr.args[0]
        if kind == "string":
            data = expr.args[0]
            return ("view", U8, data, 0, len(data))
        if kind == "self":
            return copy_value(place_get(receiver))
        if kind == "name":
            name = expr.args[0]
            if name in env.values:
                stored = env.values[name]
                if isinstance(stored, Place):
                    return copy_value(place_get(stored))
                return copy_value(stored)
            raise ModelExcluded("non-value name")
        if kind == "qualified":
            owner, member, _ = expr.args
            for case, payload in self.checker.sums.get(owner.decode(), []):
                if case == member.decode():
                    return ("sum", owner.decode(), member.decode(),
                            {n: self.zero(t) for n, t in payload})
            raise ModelExcluded("qualified value")
        if kind == "negate":
            value = self.eval(expr.args[0], machine, env, receiver)
            if value == I32_MIN:
                raise Trap("Overflow")
            return -value
        if kind == "binary":
            return self.eval_binary(expr, machine, env, receiver)
        if kind == "field":
            base_expr, member, _ = expr.args
            base, was_place = self.eval_place_or_value(base_expr, machine,
                                                     env, receiver)
            value = place_get(base) if was_place else base
            if value[0] == "record":
                return copy_value(value[2][member.decode()])
            if value[0] == "array":
                if member == b"len":
                    return len(value[2])
                if member == b"as_slice":
                    if not was_place:
                        raise ModelExcluded("as_slice without place")
                    return ("view", value[1][1], value[2], 0, len(value[2]))
                raise ModelExcluded("array member")
            if value[0] == "view":
                if member == b"len":
                    return value[4] - value[3]
                raise ModelExcluded("view member")
            raise ModelExcluded("field base")
        if kind == "index":
            base_expr, index_expr = expr.args
            base, _ = self.eval_place_or_value(base_expr, machine, env,
                                               receiver)
            index = self.eval(index_expr, machine, env, receiver)
            value = place_get(base) if isinstance(base, Place) else base
            if value[0] == "array":
                if not 0 <= index < len(value[2]):
                    raise Trap("Bounds")
                return copy_value(value[2][index])
            if value[0] == "view":
                low, high = value[3], value[4]
                if not 0 <= index < high - low:
                    raise Trap("Bounds")
                return copy_value(value[2][low + index])
            raise ModelExcluded("index base")
        if kind == "slice":
            base_expr, low_expr, high_expr = expr.args
            base, _ = self.eval_place_or_value(base_expr, machine, env,
                                               receiver)
            value = place_get(base) if isinstance(base, Place) else base
            if value[0] == "array":
                backing = value[2]
                offset = 0
                length = len(backing)
                element = value[1][1]
            elif value[0] == "view":
                backing = value[2]
                offset = value[3]
                length = value[4] - value[3]
                element = value[1]
            else:
                raise ModelExcluded("slice base")
            low = 0 if low_expr is None else self.eval(low_expr, machine,
                                                       env, receiver)
            high = length if high_expr is None else self.eval(
                high_expr, machine, env, receiver)
            if not 0 <= low <= high <= length:
                raise Trap("Bounds")
            return ("view", element, backing, offset + low, offset + high)
        if kind == "call":
            return self.eval_call(expr, machine, env, receiver)
        raise ModelExcluded(f"expression {kind}")

    def eval_binary(self, expr, machine, env, receiver):
        operator, left_expr, right_expr = expr.args
        left = self.eval(left_expr, machine, env, receiver)
        if operator == "&&":
            if left not in (0, 1):
                raise Trap("NonBoolean")
            if left == 0:
                return 0
            right = self.eval(right_expr, machine, env, receiver)
            if right not in (0, 1):
                raise Trap("NonBoolean")
            return right
        if operator == "||":
            if left not in (0, 1):
                raise Trap("NonBoolean")
            if left == 1:
                return 1
            right = self.eval(right_expr, machine, env, receiver)
            if right not in (0, 1):
                raise Trap("NonBoolean")
            return right
        right = self.eval(right_expr, machine, env, receiver)
        if operator == "+":
            result = left + right
            if not I32_MIN <= result <= I32_MAX:
                raise Trap("Overflow")
            return result
        if operator == "-":
            result = left - right
            if not I32_MIN <= result <= I32_MAX:
                raise Trap("Overflow")
            return result
        if operator == "*":
            result = left * right
            if not I32_MIN <= result <= I32_MAX:
                raise Trap("Overflow")
            return result
        if operator == "/" or operator == "%":
            if right == 0:
                raise Trap("DivisionByZero")
            if left == I32_MIN and right == -1:
                raise Trap("SignedDivisionOverflow")
            quotient = abs(left) // abs(right)
            if (left < 0) != (right < 0):
                quotient = -quotient
            if operator == "/":
                return quotient
            return left - quotient * right
        if operator == "<<":
            if not 0 <= right <= 31:
                raise Trap("ShiftCount")
            result = (left << right) & 0xFFFFFFFF
            return result - 0x100000000 if result >= 0x80000000 else result
        if operator == ">>":
            if not 0 <= right <= 31:
                raise Trap("ShiftCount")
            return left >> right
        if operator == "&":
            return left & right
        if operator == "^":
            return left ^ right
        if operator == "|":
            return left | right
        if operator == "==":
            return 1 if left == right else 0
        if operator == "!=":
            return 1 if left != right else 0
        if operator == "<":
            return 1 if left < right else 0
        if operator == "<=":
            return 1 if left <= right else 0
        if operator == ">":
            return 1 if left > right else 0
        if operator == ">=":
            return 1 if left >= right else 0
        raise ModelExcluded(f"operator {operator}")

    def eval_call(self, expr, machine, env, receiver):
        callee, arguments = expr.args
        if callee.kind == "name":
            decl = self.checker.machines[callee.args[0]]
            args = [self.eval(a, machine, env, receiver)
                    for a in arguments]
            return self.call_machine(decl, None, args)
        if callee.kind == "qualified":
            owner, member, _ = callee.args
            for case, payload in self.checker.sums[owner.decode()]:
                if case == member.decode():
                    fields = {}
                    for (pname, ptype), argument in zip(payload,
                                                        arguments):
                        value = self.eval(argument, machine, env,
                                          receiver)
                        # Each payload field establishes before the next
                        # argument evaluates.
                        fields[pname] = self.store_convert(ptype, value)
                    return ("sum", owner.decode(), member.decode(), fields)
            raise ModelExcluded("qualified call")
        if callee.kind == "field":
            base_expr, member, _ = callee.args
            target = self.eval_place(base_expr, machine, env, receiver)
            base_value = place_get(target)
            args = [self.eval(a, machine, env, receiver)
                    for a in arguments]
            if base_value[0] == "console":
                return self.call_console(member, args)
            decl = self.checker.receivers[
                (base_value[1].encode(), member)]
            return self.call_machine(decl, target, args)
        raise ModelExcluded("callee form")

    def call_console(self, member, args):
        if member == b"exit_process":
            raise ExitProcess(args[0])
        if member == b"write_byte":
            value = args[0]
            if not 0 <= value <= 255:
                raise Trap("ByteRange")
            self.stdout.append(value)
            return None
        if member == b"read_byte":
            if self.stdin_index >= len(self.stdin):
                return -1
            byte = self.stdin[self.stdin_index]
            self.stdin_index += 1
            return byte
        if member == b"write_line":
            self.stdout += view_bytes(args[0])
            self.stdout.append(10)
            return None
        raise ModelExcluded(f"console {member}")


def observation(source, stdin):
    """Canonical observation bytes for one (source, stdin) request."""
    try:
        tokens = lex(source)
        declarations = Parser(tokens, len(source)).parse_program()
        checker = Checker(declarations, len(source))
        checker.check_program()
    except Reject as reject:
        return b"\x02" + bytes([reject.reason]) \
            + (reject.offset & 0xFFFFFFFF).to_bytes(4, "little")
    evaluator = Evaluator(checker, stdin)
    try:
        evaluator.run()
        code = 0
    except ExitProcess as exit_:
        code = exit_.code
    except Trap as trap:
        return b"\x01" + bytes([trap.kind]) + bytes(evaluator.stdout)
    return b"\x00" + (code & 0xFFFFFFFF).to_bytes(4, "little") \
        + bytes(evaluator.stdout)

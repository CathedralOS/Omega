#!/usr/bin/env python3
"""Bounded independent source parser/evaluator for the OMGRFN16 relation."""

from __future__ import annotations

import dataclasses
import re
import struct
import sys
from pathlib import Path

from omgrfn16_frame import RefinementError, RefinementResourceError, require


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
COMPILER = REPO / "bootstrap/omega-bootstrap/compiler"
GATES = REPO / "bootstrap/omega-bootstrap/gates"
sys.path[:0] = [str(COMPILER), str(GATES)]

import omega_bootstrap_compilation as compilation  # noqa: E402
from omgrsw7_arithmetic_resolution_fixture import decode as decode_witness  # noqa: E402


FULL_U32 = (2, 1, 0, 0, 0, 0, 0xFFFF_FFFF)
FULL_U8 = (1, 0, 0, 0, 0, 0, 255)
TOKEN = re.compile(
    rb"(?P<space>\s+)|(?P<comment>//[^\n]*|/\*.*?\*/)|"
    rb"(?P<number>[0-9]+)|(?P<ident>[A-Za-z_][A-Za-z0-9_]*)|"
    rb"(?P<punct>::|->|==|&&|\.\.|[{}()\[\];:,.=+*\-&])|"
    rb"(?P<string>\"(?:\\.|[^\"\\])*\")|(?P<other>.)",
    re.DOTALL,
)
MAX_TOKENS = 32_768
MAX_EXPRESSION_DEPTH = 8


@dataclasses.dataclass(frozen=True)
class Token:
    text: bytes
    start: int
    end: int
    kind: str


@dataclasses.dataclass(frozen=True)
class Expr:
    kind: str
    value: int | bytes
    left: "Expr | None" = None
    right: "Expr | None" = None
    token_start: int = 0
    token_end: int = 0


@dataclasses.dataclass(frozen=True)
class Program:
    source: bytes
    expressions: tuple[Expr, ...]
    assignments: tuple[tuple[bytes, Expr], ...]
    result: Expr
    field_types: dict[bytes, str]
    success_guards: tuple[tuple[bytes, int], ...] = ()
    failure_result: int | None = None

    def postorder(self) -> tuple[int, ...]:
        rows: list[int] = []
        for expression in self.expressions:
            walk_postorder(expression, rows)
        return tuple(rows)

    def widen_count(self) -> int:
        return sum(count_kind(expression, "widen") for expression in self.expressions)


def tokens(source: bytes) -> list[Token]:
    result: list[Token] = []
    cursor = 0
    for match in TOKEN.finditer(source):
        require(match.start() == cursor, "source token gap")
        cursor = match.end()
        kind = match.lastgroup or "other"
        if kind not in ("space", "comment"):
            result.append(Token(match.group(), match.start(), match.end(), kind))
            if len(result) > MAX_TOKENS:
                raise RefinementResourceError("source token exhaustion")
    require(cursor == len(source), "source token extent")
    return result


class Parser:
    def __init__(self, stream: list[Token], start: int, stop: int):
        self.stream = stream
        self.at = start
        self.stop = stop

    def peek(self, value: bytes | None = None) -> bool:
        return self.at < self.stop and (value is None or self.stream[self.at].text == value)

    def take(self, value: bytes | None = None) -> Token:
        require(self.peek(value), f"expected source token {value!r}")
        token = self.stream[self.at]
        self.at += 1
        return token

    def expression(self, minimum: int = 0, depth: int = 1) -> Expr:
        if depth > MAX_EXPRESSION_DEPTH:
            raise RefinementResourceError("expression-depth exhaustion")
        left = self.primary(depth)
        precedence = {b"+": 1, b"-": 1, b"*": 2}
        while self.peek() and self.stream[self.at].text in precedence:
            operator = self.stream[self.at]
            level = precedence[operator.text]
            if level < minimum:
                break
            self.at += 1
            right = self.expression(level + 1, depth + 1)
            left = Expr("operator", {b"+": 8, b"-": 26, b"*": 27}[operator.text],
                        left, right, operator.start, operator.end)
        return left

    def primary(self, depth: int) -> Expr:
        if self.peek(b"("):
            opening = self.take()
            inner = self.expression(0, depth + 1)
            closing = self.take(b")")
            if self.peek(b"as"):
                self.take(b"as"); self.take(b"u32"); self.take(b"in"); self.take(b"Trapping")
                return Expr("widen", b"u32 in Trapping", inner, None,
                            opening.start, self.stream[self.at - 1].end)
            return dataclasses.replace(inner, token_start=opening.start, token_end=closing.end)
        token = self.take()
        if token.kind == "number":
            value = int(token.text)
            require(value <= 0xFFFF_FFFF, "full-u32 literal range")
            return Expr("literal", value, token_start=token.start, token_end=token.end)
        require(token.kind == "ident", "admitted arithmetic leaf")
        name = token.text
        if name == b"self" and self.peek(b"."):
            self.take(b".")
            field = self.take()
            require(field.kind == "ident", "direct self field")
            name = b"self." + field.text
            end = field.end
        else:
            end = token.end
        leaf = Expr("leaf", name, token_start=token.start, token_end=end)
        if self.peek(b"as"):
            self.take(b"as"); self.take(b"u32"); self.take(b"in"); target = self.take(b"Trapping")
            return Expr("widen", b"u32 in Trapping", leaf, None, token.start, target.end)
        return leaf


def matching(stream: list[Token], opening: int, left: bytes = b"{", right: bytes = b"}") -> int:
    depth = 0
    for index in range(opening, len(stream)):
        depth += stream[index].text == left
        depth -= stream[index].text == right
        if depth == 0:
            return index
    raise RefinementError("unclosed source delimiter")


def field_types(source: bytes) -> dict[bytes, str]:
    result: dict[bytes, str] = {}
    for match in re.finditer(
        rb"\b([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(u8|u32\s+in\s+Trapping)\s*(?=[;,)])", source
    ):
        result[match.group(1)] = "u8" if match.group(2) == b"u8" else "u32"
    return result


def selected_run(source: bytes) -> Program:
    stream = tokens(source)
    candidates: list[tuple[int, int]] = []
    for index in range(len(stream) - 4):
        if (stream[index].text == b"machine" and stream[index + 2].text == b"::"
                and stream[index + 3].text == b"run"):
            opening = next((at for at in range(index + 4, len(stream))
                            if stream[at].text == b"{"), -1)
            require(opening >= 0, "run body")
            candidates.append((opening, matching(stream, opening)))
    require(len(candidates) == 1, "unique selected run source")
    opening, closing = candidates[0]
    parser = Parser(stream, opening + 1, closing)
    assignments: list[tuple[bytes, Expr]] = []
    expressions: list[Expr] = []
    result: Expr | None = None
    while parser.at < closing:
        start = parser.at
        if (parser.peek(b"self") and parser.at + 3 < closing
                and stream[parser.at + 1].text == b"."
                and stream[parser.at + 3].text == b"="):
            parser.take(b"self"); parser.take(b"."); name = parser.take().text; parser.take(b"=")
            expression = parser.expression()
            parser.take(b";")
            assignments.append((name, expression)); expressions.append(expression)
            continue
        try:
            expression = parser.expression()
            if parser.at == closing or parser.peek(b";") and parser.at + 1 == closing:
                if parser.peek(b";"):
                    parser.take()
                result = expression; expressions.append(expression)
                break
        except RefinementError:
            pass
        parser.at = start + 1
    def state_literal(name: bytes) -> int | None:
        for index in range(opening + 1, closing - 3):
            if stream[index].text != b"state" or stream[index + 1].text != name:
                continue
            body = next((at for at in range(index + 2, closing)
                         if stream[at].text == b"{"), -1)
            require(body >= 0, "state body")
            end = matching(stream, body)
            numbers = [int(token.text) for token in stream[body + 1:end]
                       if token.kind == "number"]
            require(len(numbers) == 1, "selected terminal state result")
            return numbers[0]
        return None

    failure = state_literal(b"failed")
    if result is None:
        passed = state_literal(b"passed")
        require(passed is not None, "selected run result expression")
        result = Expr("literal", passed)
    guards = tuple(
        (match.group(1), int(match.group(2)))
        for match in re.finditer(
            rb"\bself\.([A-Za-z_][A-Za-z0-9_]*)\s*==\s*([0-9]+)", source
        )
    )
    require(any(count_kind(expression, "operator") for expression in expressions),
            "recursive arithmetic expression")
    return Program(source, tuple(expressions), tuple(assignments), result,
                   field_types(source), guards, failure)


def count_kind(expression: Expr, kind: str) -> int:
    return int(expression.kind == kind) + sum(
        count_kind(child, kind) for child in (expression.left, expression.right) if child is not None
    )


def walk_postorder(expression: Expr, output: list[int]) -> None:
    if expression.left is not None:
        walk_postorder(expression.left, output)
    if expression.right is not None:
        walk_postorder(expression.right, output)
    if expression.kind == "operator":
        output.append(int(expression.value))


class SourceTrap(Exception):
    def __init__(self, opcode: int):
        self.opcode = opcode


def evaluate(expression: Expr, environment: dict[bytes, int], types: dict[bytes, str]) -> int:
    if expression.kind == "literal":
        return int(expression.value)
    if expression.kind == "leaf":
        name = bytes(expression.value)
        lookup = name[5:] if name.startswith(b"self.") else name
        require(lookup in environment, "bound direct source leaf")
        return environment[lookup]
    if expression.kind == "widen":
        assert expression.left is not None
        require(expression.left.kind == "leaf", "pure direct exact-u8 widening")
        name = bytes(expression.left.value)
        lookup = name[5:] if name.startswith(b"self.") else name
        require(types.get(lookup) == "u8", "exact-u8 widening source")
        return evaluate(expression.left, environment, types)
    assert expression.kind == "operator" and expression.left is not None and expression.right is not None
    left = evaluate(expression.left, environment, types)
    right = evaluate(expression.right, environment, types)
    opcode = int(expression.value)
    value = left + right if opcode == 8 else left - right if opcode == 26 else left * right
    if not 0 <= value <= 0xFFFF_FFFF:
        raise SourceTrap(opcode)
    return value


def execute(program: Program) -> int:
    environment = {name: 0 for name in program.field_types}
    strings = re.findall(rb'"((?:\\.|[^"\\])*)"', program.source)
    if strings and b"head" in environment and strings[0]:
        environment[b"head"] = strings[0][0]
    for name, expression in program.assignments:
        value = evaluate(expression, environment, program.field_types)
        kind = program.field_types.get(name)
        require(kind is not None, "assignment field declaration")
        require(value <= (255 if kind == "u8" else 0xFFFF_FFFF), "assignment carrier")
        environment[name] = value
    if program.success_guards and not all(
        environment.get(name) == value for name, value in program.success_guards
    ):
        require(program.failure_result is not None, "failed source guard result")
        return program.failure_result
    return evaluate(program.result, environment, program.field_types)


def source_contents(envelope: bytes) -> tuple[bytes, ...]:
    try:
        decoded = compilation.decode(envelope)
    except Exception as error:
        raise RefinementError(f"OMGCOMP1 source closure: {error}") from error
    require(getattr(decoded, "version", 1) == 1, "OMGCOMP1 identity")
    return tuple(decoded.bundle_entries[row.bundle_entry_id].content for row in decoded.sources)


def check_witness_relation(envelope: bytes, witness: bytes, program: Program) -> None:
    try:
        rows = decode_witness(witness)
    except Exception as error:
        raise RefinementError(f"OMGRSW7 framing: {error}") from error
    types = [struct.unpack("<IBBHIIII", row) for row in rows["types"]]
    full_ids = [row[0] for row in types if row[1:] == FULL_U32]
    u8_ids = [row[0] for row in types if row[1:] == FULL_U8]
    require(len(full_ids) == 1 and len(u8_ids) == 1, "canonical scalar interning")
    full_id, u8_id = full_ids[0], u8_ids[0]
    field_rows = [struct.unpack("<6I", row) for row in rows["fields"]]
    declarations = [struct.unpack_from("<I", row, 8)[0] for row in rows["declarations"]]
    records = [struct.unpack_from("<5I", row) for row in rows["records"]]
    sources = source_contents(envelope)
    linked: dict[bytes, int] = {}
    for _, owner, _, type_id, start, length in field_rows:
        require(owner < len(records), "field owner")
        declaration = records[owner][2]
        require(declaration < len(declarations), "field declaration")
        source_id = declarations[declaration]
        require(source_id < len(sources) and start <= len(sources[source_id])
                and length <= len(sources[source_id]) - start, "field name span")
        linked[sources[source_id][start:start + length]] = type_id
    for expression in program.expressions:
        pending = [expression]
        while pending:
            node = pending.pop()
            pending.extend(child for child in (node.left, node.right) if child is not None)
            if node.kind != "leaf":
                continue
            name = bytes(node.value)
            if not name.startswith(b"self."):
                continue
            field = name[5:]
            expected = u8_id if program.field_types.get(field) == "u8" else full_id
            require(linked.get(field) == expected, "named arithmetic leaf witness link")

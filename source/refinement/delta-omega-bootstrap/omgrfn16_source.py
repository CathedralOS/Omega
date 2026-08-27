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
COMPILER = REPO / "source/on-ramp/omega-bootstrap/compiler"
GATES = REPO / "source/on-ramp/omega-bootstrap/gates"
sys.path[:0] = [str(COMPILER), str(GATES)]

import omega_bootstrap_compilation as compilation  # noqa: E402
from omgrsw7_arithmetic_resolution_fixture import (  # noqa: E402
    HEADER as WITNESS_HEADER,
    decode as decode_witness,
)


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
    sites: tuple["ExpressionSite", ...] = ()
    steps: tuple["ExpressionSite", ...] = ()
    bindings: tuple["BindingStep", ...] = ()
    view_literals: tuple[bytes, ...] = ()
    view_nonempty: int = 0
    view_heads: int = 0
    view_tails: int = 0
    success_guards: tuple[tuple[bytes, int], ...] = ()
    failure_result: int | None = None

    def postorder(self) -> tuple[int, ...]:
        rows: list[int] = []
        for expression in self.expressions:
            walk_postorder(expression, rows)
        return tuple(rows)

    def widen_count(self) -> int:
        return sum(count_kind(expression, "widen") for expression in self.expressions)


@dataclasses.dataclass(frozen=True)
class ExpressionSite:
    context: str
    expression: Expr
    start: int
    end: int
    target: bytes | None = None


@dataclasses.dataclass(frozen=True)
class BindingStep:
    start: int
    end: int
    context: str
    callee: bytes
    assignments: tuple[tuple[bytes, Expr], ...]
    result_target: bytes | None = None
    result_expression: Expr | None = None


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


def _parse_exact(stream: list[Token], start: int, stop: int) -> Expr | None:
    if start >= stop:
        return None
    parser = Parser(stream, start, stop)
    try:
        expression = parser.expression()
    except RefinementResourceError:
        raise
    except RefinementError:
        return None
    if parser.at != stop:
        return None
    pending = [(expression, 1)]
    while pending:
        node, depth = pending.pop()
        if depth > MAX_EXPRESSION_DEPTH:
            raise RefinementResourceError("expression-depth exhaustion")
        pending.extend((child, depth + 1) for child in (node.left, node.right)
                       if child is not None)
    return expression


def _contains_arithmetic(expression: Expr) -> bool:
    return count_kind(expression, "operator") > 0


def _validate_admitted(expression: Expr, types: dict[bytes, str], *, widened: bool = False) -> None:
    if expression.kind == "literal":
        require(not widened, "exact widening requires direct u8 leaf")
        return
    if expression.kind == "leaf":
        name = bytes(expression.value)
        lookup = name[5:] if name.startswith(b"self.") else name
        require(types.get(lookup) == ("u8" if widened else "u32"),
                "exact arithmetic leaf carrier")
        return
    if expression.kind == "widen":
        require(not widened and expression.left is not None
                and expression.left.kind == "leaf", "pure direct exact-u8 widening")
        _validate_admitted(expression.left, types, widened=True)
        return
    require(expression.kind == "operator" and not widened
            and expression.left is not None and expression.right is not None,
            "admitted recursive arithmetic expression")
    _validate_admitted(expression.left, types)
    _validate_admitted(expression.right, types)


def _matching_semicolon(stream: list[Token], start: int, stop: int) -> int:
    parens = brackets = braces = 0
    for index in range(start, stop):
        text = stream[index].text
        if text == b"(": parens += 1
        elif text == b")": parens -= 1
        elif text == b"[": brackets += 1
        elif text == b"]": brackets -= 1
        elif text == b"{": braces += 1
        elif text == b"}": braces -= 1
        elif text == b";" and parens == brackets == braces == 0:
            return index
    return -1


def _argument_segments(stream: list[Token], opening: int, closing: int) -> list[tuple[int, int]]:
    result: list[tuple[int, int]] = []
    start = opening + 1
    parens = brackets = braces = 0
    for index in range(start, closing):
        text = stream[index].text
        if text == b"(": parens += 1
        elif text == b")": parens -= 1
        elif text == b"[": brackets += 1
        elif text == b"]": brackets -= 1
        elif text == b"{": braces += 1
        elif text == b"}": braces -= 1
        elif text == b"," and parens == brackets == braces == 0:
            result.append((start, index)); start = index + 1
    if start < closing:
        result.append((start, closing))
    return result


def _call_context(stream: list[Token], opening: int) -> str:
    # `name(...)` after an authored arrow is a state-transition argument list;
    # every other body call is an ordinary call argument list.
    name = opening - 1
    if name >= 0 and stream[name].kind == "ident":
        for index in range(name - 1, max(-1, name - 5), -1):
            if stream[index].text == b"->":
                return "transition-argument"
            if stream[index].text in (b";", b"{", b"}"):
                break
    return "call-argument"


def _string_payload(token: Token) -> bytes:
    require(token.kind == "string" and len(token.text) >= 2, "plain byte string")
    payload = token.text[1:-1]
    require(b"\\" not in payload and all(byte < 0x80 for byte in payload),
            "plain static byte-view literal")
    return payload


def _call_signatures(stream: list[Token]) -> dict[bytes, tuple[tuple[bytes, ...], Expr | None]]:
    result: dict[bytes, tuple[tuple[bytes, ...], Expr | None]] = {}
    for index, token in enumerate(stream):
        if token.text not in (b"machine", b"state"):
            continue
        if token.text == b"machine":
            require(index + 3 < len(stream) and stream[index + 2].text == b"::",
                    "machine signature")
            name_index = index + 3
        else:
            name_index = index + 1
        require(name_index < len(stream) and stream[name_index].kind == "ident",
                "callable signature name")
        opening = next((at for at in range(name_index + 1, len(stream))
                        if stream[at].text == b"("), -1)
        require(opening >= 0, "callable signature parameters")
        closing = matching(stream, opening, b"(", b")")
        names = tuple(stream[at].text for at in range(opening + 1, closing - 1)
                      if stream[at].kind == "ident" and stream[at + 1].text == b":")
        body = next((at for at in range(closing + 1, len(stream))
                     if stream[at].text == b"{"), -1)
        require(body >= 0, "callable signature body")
        body_end = matching(stream, body)
        returned = _parse_exact(stream, body + 1, body_end)
        result[stream[name_index].text] = (names, returned)
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
    types = field_types(source)
    assignments: list[tuple[bytes, Expr]] = []
    bindings: list[BindingStep] = []
    steps: dict[tuple[int, int], ExpressionSite] = {}
    sites: dict[tuple[int, int], ExpressionSite] = {}
    signatures = _call_signatures(stream)

    # Direct assignments are also the source-only evaluator's state updates.
    # A call-valued assignment is handled by its selected arithmetic argument
    # below, so excluded call syntax never enters the arithmetic grammar.
    for index in range(opening + 1, closing - 3):
        if not (stream[index].text == b"self" and stream[index + 1].text == b"."
                and stream[index + 2].kind == "ident" and stream[index + 3].text == b"="):
            continue
        end = _matching_semicolon(stream, index + 4, closing)
        require(end >= 0, "assignment terminator")
        expression = _parse_exact(stream, index + 4, end)
        if expression is None:
            continue
        assignments.append((stream[index + 2].text, expression))
        step = ExpressionSite("assignment", expression, index + 4, end,
                              stream[index + 2].text)
        steps[(index + 4, end)] = step
        if _contains_arithmetic(expression):
            _validate_admitted(expression, types)
            sites[(index + 4, end)] = step

    # Guard expressions end at the comparison token.  The result comparison is
    # deliberately outside the selected arithmetic subtree.
    for index in range(opening + 1, closing):
        if stream[index].text != b"transition":
            continue
        brace = next((at for at in range(index + 1, closing)
                      if stream[at].text == b"{"), -1)
        if brace < 0:
            continue
        equal = next((at for at in range(index + 1, brace)
                      if stream[at].text == b"=="), -1)
        if equal < 0:
            continue
        expression = _parse_exact(stream, index + 1, equal)
        if expression is not None and _contains_arithmetic(expression):
            _validate_admitted(expression, types)
            sites[(index + 1, equal)] = ExpressionSite(
                "guard", expression, index + 1, equal,
            )

    # Parse each top-level argument independently.  This admits the settled one
    # potentially trapping argument with pure siblings and naturally excludes
    # call/index/mutation leaves from the recursive expression grammar.
    for index in range(opening + 1, closing):
        if (stream[index].text != b"(" or index == 0
                or stream[index - 1].kind != "ident"
                or (index >= 2 and stream[index - 2].text in (b"state", b"machine"))
                or stream[index - 1].text in (b"transition", b"when", b"state", b"machine", b"data")):
            continue
        end = matching(stream, index, b"(", b")")
        target = None
        statement = index - 1
        while statement > opening and stream[statement].text not in (b";", b"{", b"}"):
            statement -= 1
        for cursor in range(statement + 1, max(statement + 1, index - 3)):
            if (stream[cursor].text == b"self" and stream[cursor + 1].text == b"."
                    and stream[cursor + 2].kind == "ident" and stream[cursor + 3].text == b"="):
                target = stream[cursor + 2].text
                break
        selected: list[tuple[int, int, Expr]] = []
        segments = _argument_segments(stream, index, end)
        parsed = [_parse_exact(stream, start, stop) for start, stop in segments]
        for (start, stop), expression in zip(segments, parsed):
            if expression is None or not _contains_arithmetic(expression):
                continue
            _validate_admitted(expression, types)
            selected.append((start, stop, expression))
        require(len(selected) <= 1, "one potentially trapping argument")
        for start, stop, expression in selected:
            if (start, stop) in sites:
                continue
            sites[(start, stop)] = ExpressionSite(
                _call_context(stream, index), expression, start, stop, target,
            )
        signature = signatures.get(stream[index - 1].text)
        if signature is not None and len(signature[0]) == len(parsed) and all(parsed):
            parameters, returned = signature
            bindings.append(BindingStep(
                start=index, end=end, context=_call_context(stream, index),
                callee=stream[index - 1].text,
                assignments=tuple(zip(parameters, parsed)),  # type: ignore[arg-type]
                result_target=target, result_expression=returned,
            ))

    ordered_sites = tuple(sorted(sites.values(), key=lambda site: (site.start, site.end)))
    steps.update({(site.start, site.end): site for site in ordered_sites})
    ordered_steps = tuple(sorted(steps.values(), key=lambda site: (site.start, site.end)))
    expressions = tuple(site.expression for site in ordered_sites)
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
    passed = state_literal(b"passed")
    require(passed is not None, "selected run result expression")
    result = Expr("literal", passed)
    guards = tuple(
        (match.group(1), int(match.group(2)))
        for match in re.finditer(
            rb"(?<![A-Za-z0-9_.])(?:self\.)?([A-Za-z_][A-Za-z0-9_]*)\s*==\s*([0-9]+)",
            source,
        )
    )
    require(any(count_kind(expression, "operator") for expression in expressions),
            "recursive arithmetic expression")
    body_start = stream[opening].end
    body_end = stream[closing].start
    body = source[body_start:body_end]
    view_literals = tuple(
        _string_payload(token) for token in stream[opening + 1:closing]
        if token.kind == "string"
    )
    return Program(
        source=source, expressions=expressions, assignments=tuple(assignments),
        result=result, field_types=types, sites=ordered_sites, steps=ordered_steps,
        bindings=tuple(bindings),
        view_literals=view_literals,
        view_nonempty=len(re.findall(rb"\.len\s*>\s*0", body)),
        view_heads=len(re.findall(rb"\[\s*0\s*\]", body)),
        view_tails=len(re.findall(rb"\[\s*1\s*\.\.\s*\]", body)),
        success_guards=guards, failure_result=failure,
    )


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
    if program.view_literals and b"head" in environment and program.view_literals[0]:
        environment[b"head"] = program.view_literals[0][0]
    events = ([(site.start, 1, site) for site in program.steps]
              + [(binding.start, 0, binding) for binding in program.bindings])
    for _, event_kind, event in sorted(events, key=lambda row: (row[0], row[1])):
        if isinstance(event, BindingStep):
            values = tuple(evaluate(expression, environment, program.field_types)
                           for _, expression in event.assignments)
            for (target, _), value in zip(event.assignments, values):
                carrier = program.field_types.get(target)
                require(carrier is not None, "call/transition parameter declaration")
                require(value <= (255 if carrier == "u8" else 0xFFFF_FFFF),
                        "call/transition argument carrier")
                environment[target] = value
            if event.result_target is not None and event.result_expression is not None:
                value = evaluate(event.result_expression, environment, program.field_types)
                carrier = program.field_types.get(event.result_target)
                require(carrier is not None, "call result field declaration")
                require(value <= (255 if carrier == "u8" else 0xFFFF_FFFF),
                        "call result carrier")
                environment[event.result_target] = value
            continue
        site = event
        value = evaluate(site.expression, environment, program.field_types)
        if site.target is None:
            continue
        carrier = program.field_types.get(site.target)
        require(carrier is not None, "assignment field declaration")
        require(value <= (255 if carrier == "u8" else 0xFFFF_FFFF), "assignment carrier")
        environment[site.target] = value
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


def _source_span(sources: tuple[bytes, ...], source_id: int,
                 start: int, length: int, label: str) -> bytes:
    require(source_id < len(sources), f"{label} source")
    source = sources[source_id]
    require(start <= len(source) and length <= len(source) - start, f"{label} span")
    return source[start:start + length]


def _identifier(contents: bytes, label: str, *, allow_empty: bool = False) -> bytes:
    if allow_empty and not contents:
        return contents
    require(re.fullmatch(rb"[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*",
                         contents) is not None, f"{label} identifier")
    return contents


def _dense_rows(rows: dict[str, list[bytes]]) -> None:
    ceilings = {
        "units": 16, "imports": 64, "bindings": 4096, "declarations": 256,
        "types": 8192, "records": 128, "fields": 8192, "sums": 128,
        "cases": 4096, "payloads": 4096, "machines": 128,
        "machine_parameters": 896, "blocks": 2048, "block_parameters": 4096,
    }
    for name, table in rows.items():
        if len(table) > ceilings[name]:
            raise RefinementResourceError(f"OMGRSW7 {name} exhaustion")
        for index, row in enumerate(table):
            require(struct.unpack_from("<I", row)[0] == index, f"dense {name} IDs")
    reserved = {
        "imports": (22, 24), "bindings": (10, 12), "declarations": (6, 8),
        "types": (6, 8), "records": (21, 24), "machines": (13, 16),
        "blocks": (13, 16),
    }
    for name, (start, stop) in reserved.items():
        require(all(not any(row[start:stop]) for row in rows[name]),
                f"canonical {name} reserved bytes")


def _canonical_order(rows: dict[str, list[bytes]]) -> None:
    def words(name: str, offsets: tuple[int, ...]) -> list[tuple[int, ...]]:
        return [tuple(struct.unpack_from("<I", row, offset)[0] for offset in offsets)
                for row in rows[name]]
    for name, offsets in (
        ("declarations", (8, 12)), ("records", (4,)), ("fields", (4, 8)),
        ("sums", (4,)), ("cases", (4, 8)), ("payloads", (4, 8)),
        ("machines", (4,)), ("machine_parameters", (4, 8)),
        ("blocks", (4, 8)), ("block_parameters", (4, 8)),
    ):
        keys = words(name, offsets)
        require(keys == sorted(keys), f"canonical {name} ordering")


def _parameter_names(stream: list[Token], opening: int, closing: int) -> list[tuple[int, int]]:
    return [
        (stream[index].start, stream[index].end - stream[index].start)
        for index in range(opening + 1, closing - 1)
        if stream[index].kind == "ident" and stream[index + 1].text == b":"
    ]


def _source_syntax_index(source: bytes) -> dict[str, object]:
    """Independently recover every OMGRSW7 source-backed canonical span."""
    stream = tokens(source)
    result: dict[str, object] = {"module": None, "imports": [], "declarations": []}
    declarations: list[dict[str, object]] = result["declarations"]  # type: ignore[assignment]
    imports: list[dict[str, object]] = result["imports"]  # type: ignore[assignment]
    at = 0
    ordinal = 0
    while at < len(stream):
        public = stream[at].text == b"pub"
        keyword = at + 1 if public else at
        if keyword >= len(stream):
            break
        if stream[keyword].text in (b"module", b"use"):
            end = next((index for index in range(keyword + 1, len(stream))
                        if stream[index].text == b";"), -1)
            require(end > keyword + 1, "module/use terminator")
            start = stream[keyword + 1].start
            stop = stream[end - 1].end
            if stream[keyword].text == b"module":
                require(result["module"] is None, "unique authored module")
                result["module"] = (start, stop - start)
            else:
                final = next((stream[index] for index in range(end - 1, keyword, -1)
                              if stream[index].kind == "ident"), None)
                require(final is not None, "use final name")
                imports.append({"path": (start, stop - start),
                                "final": (final.start, final.end - final.start)})
            at = end + 1
            continue
        if stream[keyword].text == b"data":
            require(keyword + 1 < len(stream) and stream[keyword + 1].kind == "ident",
                    "data declaration name")
            name = stream[keyword + 1]
            opening = next((index for index in range(keyword + 2, len(stream))
                            if stream[index].text == b"{"), -1)
            require(opening >= 0, "data declaration body")
            closing = matching(stream, opening)
            members: list[int] = []
            depth = 0
            for index in range(opening + 1, closing):
                text = stream[index].text
                if text in (b"{", b"(", b"["): depth += 1
                elif text in (b"}", b")", b"]"): depth -= 1
                elif depth == 0:
                    members.append(index)
            cases: list[dict[str, object]] = []
            fields: list[tuple[int, int]] = []
            member_set = set(members)
            for index in members:
                if (stream[index].kind == "ident" and index + 1 in member_set
                        and stream[index + 1].text == b":"):
                    fields.append((stream[index].start, stream[index].end - stream[index].start))
                if stream[index].text != b"case":
                    continue
                case_name = stream[index + 1]
                require(case_name.kind == "ident", "case name")
                payload_names: list[tuple[int, int]] = []
                if index + 2 < closing and stream[index + 2].text == b"(":
                    case_close = matching(stream, index + 2, b"(", b")")
                    payload_names = _parameter_names(stream, index + 2, case_close)
                cases.append({
                    "name": (case_name.start, case_name.end - case_name.start),
                    "payloads": payload_names,
                })
            kind = 3 if cases else 1
            declarations.append({
                "kind": kind, "visibility": int(public), "ordinal": ordinal,
                "name": (name.start, name.end - name.start),
                "copy": int(any(token.text == b"copy" for token in stream[keyword + 2:opening])),
                "fields": fields, "cases": cases,
            })
            ordinal += 1; at = closing + 1
            continue
        if stream[keyword].text == b"machine":
            owner = stream[keyword + 1]
            require(owner.kind == "ident" and stream[keyword + 2].text == b"::"
                    and stream[keyword + 3].kind == "ident", "machine declaration name")
            name = stream[keyword + 3]
            params_open = next((index for index in range(keyword + 4, len(stream))
                                if stream[index].text == b"("), -1)
            require(params_open >= 0, "machine parameters")
            params_close = matching(stream, params_open, b"(", b")")
            opening = next((index for index in range(params_close + 1, len(stream))
                            if stream[index].text == b"{"), -1)
            require(opening >= 0, "machine body")
            closing = matching(stream, opening)
            states: list[dict[str, object]] = []
            depth = 0; index = opening + 1
            while index < closing:
                text = stream[index].text
                if text == b"{": depth += 1
                elif text == b"}": depth -= 1
                elif depth == 0 and text == b"state":
                    state_name = stream[index + 1]
                    require(state_name.kind == "ident", "state name")
                    state_open = next((cursor for cursor in range(index + 2, closing)
                                       if stream[cursor].text == b"("), -1)
                    require(state_open >= 0, "state parameters")
                    state_params_close = matching(stream, state_open, b"(", b")")
                    state_body_open = next((cursor for cursor in range(state_params_close + 1, closing)
                                            if stream[cursor].text == b"{"), -1)
                    require(state_body_open >= 0, "state body")
                    state_close = matching(stream, state_body_open)
                    body_start = (stream[state_body_open + 1].start
                                  if state_body_open + 1 < state_close else stream[state_close].start)
                    receiver = 2 if any(token.text == b"mut"
                                        for token in stream[state_open + 1:state_params_close]) else 1
                    states.append({
                        "name": (state_name.start, state_name.end - state_name.start),
                        "access": receiver,
                        "params": _parameter_names(stream, state_open, state_params_close),
                        "body": (body_start, stream[state_close].start),
                        "keyword": stream[index].start,
                    })
                    index = state_close + 1
                    continue
                index += 1
            entry_end = int(states[0]["keyword"]) if states else stream[closing].start
            entry_start = (stream[opening + 1].start
                           if opening + 1 < closing else stream[closing].start)
            receiver = 2 if any(token.text == b"mut"
                                for token in stream[params_open + 1:params_close]) else 1
            blocks = [{"name": None, "access": receiver, "params": [],
                       "body": (entry_start, entry_end)}] + states
            declarations.append({
                "kind": 2, "visibility": int(public), "ordinal": ordinal,
                "name": (name.start, name.end - name.start),
                "owner": (owner.start, owner.end - owner.start),
                "access": receiver,
                "params": _parameter_names(stream, params_open, params_close),
                "blocks": blocks,
            })
            ordinal += 1; at = closing + 1
            continue
        at += 1
    result["tokens"] = {(token.start, token.end - token.start) for token in stream}
    result["token_starts"] = {token.start for token in stream}
    result["token_ends"] = {token.end for token in stream}
    return result


def _check_canonical_source_rows(closure, sources: tuple[bytes, ...],
                                 rows: dict[str, list[bytes]]) -> dict[bytes, set[int]]:
    syntax = tuple(_source_syntax_index(source) for source in sources)
    next_import = next_declaration = 0
    for unit_id, raw in enumerate(rows["units"]):
        (row_id, owner_package, module_string, module_start, module_length,
         import_start, import_count, declaration_start, declaration_count) = struct.unpack("<9I", raw)
        source_row = closure.sources[unit_id]
        require((row_id, owner_package, module_string) == (
            unit_id, source_row.owner_package_id, source_row.module_string_id,
        ), "canonical unit identity")
        expected_module = syntax[unit_id]["module"]
        require((module_start, module_length) == (
            (0xFFFF_FFFF, 0) if expected_module is None else expected_module
        ), "canonical authored module span")
        if expected_module is not None:
            start, length = expected_module
            require(sources[unit_id][start:start + length]
                    == closure.strings[module_string].encode("utf-8"),
                    "authored/resolver module identity")
        imports = syntax[unit_id]["imports"]
        declarations = syntax[unit_id]["declarations"]
        require((import_start, import_count) == (next_import, len(imports)),
                "canonical unit import partition")
        require((declaration_start, declaration_count)
                == (next_declaration, len(declarations)),
                "canonical unit declaration partition")
        next_import += len(imports); next_declaration += len(declarations)
    require(next_import == len(rows["imports"])
            and next_declaration == len(rows["declarations"]),
            "complete unit partitions")

    for raw in rows["imports"]:
        (row_id, source_id, ordinal, path_start, path_length,
         origin, target_kind, _, _, target_package, target_module,
         target_declaration, local_start, local_length) = struct.unpack("<5IBBH6I", raw)
        require(source_id < len(syntax), "import source")
        expected = syntax[source_id]["imports"]
        require(ordinal < len(expected), "import ordinal")
        item = expected[ordinal]
        require((path_start, path_length) == item["path"]
                and (local_start, local_length) == item["final"],
                "canonical import source spans")
        require(origin in (0, 1) and target_kind in (1, 2), "import resolution shape")
        require(target_package < len(closure.packages)
                and target_module < len(closure.strings)
                and target_declaration < len(rows["declarations"]),
                "import resolution target")

    declaration_rows = [struct.unpack("<IBBH5I", raw) for raw in rows["declarations"]]
    declaration_syntax: list[dict[str, object]] = []
    for row in declaration_rows:
        row_id, kind, visibility, _, source_id, ordinal, start, length, _ = row
        require(source_id < len(syntax), "declaration source")
        expected_rows = syntax[source_id]["declarations"]
        require(ordinal < len(expected_rows), "declaration ordinal")
        expected = expected_rows[ordinal]
        require((kind, visibility, start, length) == (
            expected["kind"], expected["visibility"], *expected["name"],
        ), "canonical declaration source row")
        declaration_syntax.append(expected)

    for raw in rows["bindings"]:
        _, source_id, role, target_kind, _, start, length, target, import_id = struct.unpack(
            "<2IBBH4I", raw
        )
        require(source_id < len(sources) and role in (1, 2, 3)
                and target_kind in (1, 2), "binding source/role")
        contents = _identifier(_source_span(sources, source_id, start, length, "binding"),
                               "binding")
        require(start in syntax[source_id]["token_starts"]
                and start + length in syntax[source_id]["token_ends"]
                and bool(contents), "canonical binding token span")
        require(target < len(rows["declarations"])
                and (import_id == 0xFFFF_FFFF or import_id < len(rows["imports"])),
                "binding target")

    records = [struct.unpack("<5IB3x", raw) for raw in rows["records"]]
    sums = [struct.unpack("<5IB3x", raw) for raw in rows["sums"]]
    machines = [struct.unpack("<3IBBH6I", raw) for raw in rows["machines"]]
    expected_records = [index for index, item in enumerate(declaration_syntax) if item["kind"] == 1]
    expected_sums = [index for index, item in enumerate(declaration_syntax) if item["kind"] == 3]
    expected_machines = [index for index, item in enumerate(declaration_syntax) if item["kind"] == 2]
    require(len(records) == len(expected_records) and len(sums) == len(expected_sums)
            and len(machines) == len(expected_machines), "canonical kind-table counts")

    next_field = 0
    for record_id, row in enumerate(records):
        _, declaration, nominal_type, field_start, field_count, flags = row
        expected = declaration_syntax[expected_records[record_id]]
        require(declaration == expected_records[record_id] and nominal_type == record_id
                and (field_start, field_count) == (next_field, len(expected["fields"]))
                and flags == expected["copy"], "canonical record row")
        next_field += field_count
    require(next_field == len(rows["fields"]), "complete record field partition")

    linked: dict[bytes, set[int]] = {}
    for raw in rows["fields"]:
        row_id, owner, ordinal, type_id, start, length = struct.unpack("<6I", raw)
        require(owner < len(records) and type_id < len(rows["types"]), "field owner/type")
        expected = declaration_syntax[records[owner][1]]["fields"]
        require(ordinal < len(expected) and (start, length) == expected[ordinal],
                "canonical field source span")
        source_id = declaration_rows[records[owner][1]][4]
        name = sources[source_id][start:start + length]
        linked.setdefault(name, set()).add(type_id)

    next_case = next_payload = 0
    for sum_id, row in enumerate(sums):
        _, declaration, nominal_type, case_start, case_count, flags = row
        expected = declaration_syntax[expected_sums[sum_id]]
        require(declaration == expected_sums[sum_id]
                and nominal_type == len(records) + sum_id
                and (case_start, case_count) == (next_case, len(expected["cases"]))
                and flags == expected["copy"], "canonical sum row")
        next_case += case_count
    require(next_case == len(rows["cases"]), "complete sum case partition")
    cases = [struct.unpack("<7I", raw) for raw in rows["cases"]]
    for case_id, case in enumerate(cases):
        _, owner, ordinal, payload_start, payload_count, start, length = case
        require(owner < len(sums), "case owner")
        expected_cases = declaration_syntax[sums[owner][1]]["cases"]
        require(ordinal < len(expected_cases), "case ordinal")
        expected = expected_cases[ordinal]
        require((start, length) == expected["name"]
                and (payload_start, payload_count) == (next_payload, len(expected["payloads"])),
                "canonical case source row")
        next_payload += payload_count
    require(next_payload == len(rows["payloads"]), "complete case payload partition")
    for raw in rows["payloads"]:
        _, owner, ordinal, type_id, start, length = struct.unpack("<6I", raw)
        require(owner < len(cases) and type_id < len(rows["types"]), "payload owner/type")
        sum_id = cases[owner][1]
        expected = declaration_syntax[sums[sum_id][1]]["cases"][cases[owner][2]]["payloads"]
        require(ordinal < len(expected) and (start, length) == expected[ordinal],
                "canonical payload source span")

    record_names = {
        sources[declaration_rows[declaration][4]][
            declaration_rows[declaration][6]:declaration_rows[declaration][6] + declaration_rows[declaration][7]
        ]: record_id
        for record_id, declaration in enumerate(expected_records)
    }
    next_machine_parameter = next_block = 0
    for machine_id, row in enumerate(machines):
        (_, declaration, owner, access, flags, reserved, result_type,
         parameter_start, parameter_count, block_start, block_count, entry) = row
        expected = declaration_syntax[expected_machines[machine_id]]
        source_id = declaration_rows[expected_machines[machine_id]][4]
        owner_name = sources[source_id][expected["owner"][0]:sum(expected["owner"])]
        require(declaration == expected_machines[machine_id]
                and owner == record_names.get(owner_name)
                and access == expected["access"] and flags == reserved == 0,
                "canonical machine identity")
        require((parameter_start, parameter_count)
                == (next_machine_parameter, len(expected["params"]))
                and (block_start, block_count) == (next_block, len(expected["blocks"]))
                and entry == block_start, "canonical machine partitions")
        require(result_type == 0xFFFF_FFFF or result_type < len(rows["types"]),
                "machine result type")
        next_machine_parameter += parameter_count; next_block += block_count
    require(next_machine_parameter == len(rows["machine_parameters"])
            and next_block == len(rows["blocks"]), "complete machine partitions")

    for raw in rows["machine_parameters"]:
        _, owner, ordinal, type_id, start, length = struct.unpack("<6I", raw)
        require(owner < len(machines) and type_id < len(rows["types"]),
                "machine parameter owner/type")
        expected = declaration_syntax[machines[owner][1]]["params"]
        require(ordinal < len(expected) and (start, length) == expected[ordinal],
                "canonical machine-parameter span")
        source_id = declaration_rows[machines[owner][1]][4]
        linked.setdefault(sources[source_id][start:start + length], set()).add(type_id)

    blocks = [struct.unpack("<3IBBH6I", raw) for raw in rows["blocks"]]
    next_block_parameter = 0
    for block_id, block in enumerate(blocks):
        (_, owner, ordinal, access, flags, reserved, body_start, body_end,
         name_start, name_length, parameter_start, parameter_count) = block
        require(owner < len(machines), "block owner")
        expected_blocks = declaration_syntax[machines[owner][1]]["blocks"]
        require(ordinal < len(expected_blocks), "block ordinal")
        expected = expected_blocks[ordinal]
        expected_name = ((0xFFFF_FFFF, 0) if expected["name"] is None else expected["name"])
        require(access == expected["access"] and flags == reserved == 0
                and (body_start, body_end) == expected["body"]
                and (name_start, name_length) == expected_name,
                "canonical block body/name span")
        require((parameter_start, parameter_count)
                == (next_block_parameter, len(expected["params"])),
                "canonical block-parameter partition")
        next_block_parameter += parameter_count
    require(next_block_parameter == len(rows["block_parameters"]),
            "complete block-parameter partition")
    for raw in rows["block_parameters"]:
        _, owner, ordinal, type_id, start, length = struct.unpack("<6I", raw)
        require(owner < len(blocks) and type_id < len(rows["types"]),
                "block parameter owner/type")
        machine = blocks[owner][1]
        expected = declaration_syntax[machines[machine][1]]["blocks"][blocks[owner][2]]["params"]
        require(ordinal < len(expected) and (start, length) == expected[ordinal],
                "canonical block-parameter span")
        source_id = declaration_rows[machines[machine][1]][4]
        linked.setdefault(sources[source_id][start:start + length], set()).add(type_id)
    return linked


def check_witness_relation(envelope: bytes, witness: bytes, program: Program) -> None:
    try:
        rows = decode_witness(witness)
    except Exception as error:
        raise RefinementError(f"OMGRSW7 framing: {error}") from error
    _dense_rows(rows)
    _canonical_order(rows)
    header = WITNESS_HEADER.unpack_from(witness)
    words = header[5:]
    selected_machine = words[-2]
    types = [struct.unpack("<IBBHIIII", row) for row in rows["types"]]
    require(len({row[1:] for row in types}) == len(types), "canonical type interning")
    full_ids = [row[0] for row in types if row[1:] == FULL_U32]
    u8_ids = [row[0] for row in types if row[1:] == FULL_U8]
    require(len(full_ids) == 1 and len(u8_ids) == 1, "canonical scalar interning")
    full_id, u8_id = full_ids[0], u8_ids[0]
    require(selected_machine < len(rows["machines"]), "selected machine ID")

    try:
        closure = compilation.decode(envelope)
    except Exception as error:
        raise RefinementError(f"OMGCOMP1 source closure: {error}") from error
    sources = tuple(closure.bundle_entries[row.bundle_entry_id].content
                    for row in closure.sources)
    require(len(rows["units"]) == len(sources), "one canonical unit per source")
    linked = _check_canonical_source_rows(closure, sources, rows)
    unit_source_ids = [struct.unpack_from("<I", row, 4)[0] for row in rows["units"]]
    require(unit_source_ids == list(range(len(sources))), "canonical unit/source order")
    for row in rows["units"]:
        _, source_id, _, start, length, *_ = struct.unpack("<9I", row)
        if length == 0:
            require(start in (0, 0xFFFF_FFFF), "empty module span")
        else:
            _identifier(_source_span(sources, source_id, start, length, "module"),
                        "module")

    declarations = [struct.unpack_from("<I", row, 8)[0] for row in rows["declarations"]]
    declaration_names: list[bytes] = []
    for row in rows["declarations"]:
        _, _, _, _, source_id, _, start, length, _ = struct.unpack("<IBBH5I", row)
        declaration_names.append(_identifier(
            _source_span(sources, source_id, start, length, "declaration"), "declaration"
        ))

    records = [struct.unpack_from("<5I", row) for row in rows["records"]]
    sums = [struct.unpack_from("<6I", row) for row in rows["sums"]]
    machines = [struct.unpack_from("<3I", row) for row in rows["machines"]]
    for _, declaration, *_ in records + sums:
        require(declaration < len(declarations), "nominal declaration")
    for _, declaration, owner in machines:
        require(declaration < len(declarations) and owner < len(records),
                "machine declaration/owner")

    selected = machines[selected_machine]
    selected_declaration = selected[1]
    selected_owner = selected[2]
    require(declaration_names[selected_declaration]
            == closure.strings[closure.root_machine_string_id].encode("utf-8"),
            "selected root machine name")
    owner_declaration = records[selected_owner][1]
    require(declaration_names[owner_declaration]
            == closure.strings[closure.root_owner_string_id].encode("utf-8"),
            "selected root owner name")

    field_rows = [struct.unpack("<6I", row) for row in rows["fields"]]
    for _, owner, _, type_id, start, length in field_rows:
        require(owner < len(records) and type_id < len(types), "field owner/type")
        declaration = records[owner][1]
        require(declaration < len(declarations), "field declaration")
        source_id = declarations[declaration]
        name = _identifier(_source_span(sources, source_id, start, length, "field"), "field")
        linked.setdefault(name, set()).add(type_id)

    block_rows = [struct.unpack_from("<3I", row) for row in rows["blocks"]]
    for table, owners, owner_name in (
        ("machine_parameters", machines, "machine"),
        ("block_parameters", block_rows, "block"),
    ):
        for row in rows[table]:
            _, owner, _, type_id, start, length = struct.unpack("<6I", row)
            require(owner < len(owners) and type_id < len(types), f"{table} owner/type")
            machine = owner if owner_name == "machine" else block_rows[owner][1]
            declaration = machines[machine][1]
            source_id = declarations[declaration]
            name = _identifier(_source_span(sources, source_id, start, length, table), table)
            linked.setdefault(name, set()).add(type_id)

    if program.view_literals:
        view_ids = [row[0] for row in types
                    if row[1:] == (7, 0, 0, u8_id, 0, 0, 0)]
        require(len(view_ids) == 1, "canonical shared-byte-view type")
        parameter_types = {
            struct.unpack_from("<I", row, 12)[0]
            for table in ("machine_parameters", "block_parameters")
            for row in rows[table]
        }
        require(view_ids[0] in parameter_types, "view parameter witness link")

    for expression in program.expressions:
        pending = [expression]
        while pending:
            node = pending.pop()
            pending.extend(child for child in (node.left, node.right) if child is not None)
            if node.kind != "leaf":
                continue
            name = bytes(node.value)
            leaf = name[5:] if name.startswith(b"self.") else name
            expected = u8_id if program.field_types.get(leaf) == "u8" else full_id
            require(expected in linked.get(leaf, set()), "named arithmetic leaf witness link")


def witness_leaf_names(envelope: bytes, witness: bytes) -> dict[str, dict[int, bytes]]:
    """Return independently source-checked witness names by canonical row ID."""
    try:
        rows = decode_witness(witness)
    except Exception as error:
        raise RefinementError(f"OMGRSW7 leaf-name framing: {error}") from error
    sources = source_contents(envelope)
    declarations = [struct.unpack_from("<I", row, 8)[0] for row in rows["declarations"]]
    records = [struct.unpack_from("<5I", row) for row in rows["records"]]
    machines = [struct.unpack_from("<3I", row) for row in rows["machines"]]
    blocks = [struct.unpack_from("<3I", row) for row in rows["blocks"]]
    result: dict[str, dict[int, bytes]] = {
        "fields": {}, "payloads": {},
        "machine_parameters": {}, "block_parameters": {},
    }
    for row in rows["fields"]:
        row_id, owner, _, _, start, length = struct.unpack("<6I", row)
        declaration = records[owner][1]
        source_id = declarations[declaration]
        result["fields"][row_id] = _identifier(
            _source_span(sources, source_id, start, length, "field"), "field"
        )
    sums = [struct.unpack_from("<2I", row) for row in rows["sums"]]
    cases = [struct.unpack_from("<2I", row) for row in rows["cases"]]
    for row in rows["payloads"]:
        row_id, owner, _, _, start, length = struct.unpack("<6I", row)
        sum_id = cases[owner][1]
        declaration = sums[sum_id][1]
        source_id = declarations[declaration]
        result["payloads"][row_id] = _identifier(
            _source_span(sources, source_id, start, length, "payload"), "payload"
        )
    for table, owners, block_owned in (
        ("machine_parameters", machines, False),
        ("block_parameters", blocks, True),
    ):
        for row in rows[table]:
            row_id, owner, _, _, start, length = struct.unpack("<6I", row)
            machine_id = blocks[owner][1] if block_owned else owner
            source_id = declarations[machines[machine_id][1]]
            result[table][row_id] = _identifier(
                _source_span(sources, source_id, start, length, table), table
            )
    return result

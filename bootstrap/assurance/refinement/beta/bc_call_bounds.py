#!/usr/bin/env python3
"""Produce an untrusted call/frame/SCC witness for the exact ``bc.beta``.

The eventual Alpha checker remains authoritative: it must reparse the source,
reconstruct the graph, validate the rank sites, and recompute both stack bounds.
This producer only supplies deterministic source-order hints so that checker does
not need a large closed-form table embedded in Alpha source.

Canonical ``BCS9`` layout is a four-byte magic followed by six split u32 tables:
expression explicit bytes, expression hidden returns, block explicit bytes,
block hidden returns, root explicit bytes, and root hidden returns.  Each depth
table has 64 x 70 cells in depth-major/procedure-minor order; each root table has
70 cells.  The graph, procedure, and SCC reconstruction remains available in
``--report`` but is not duplicated in the witness because the Alpha checker
reconstructs it independently from the already-owned source tables.

A summary cell is ``(explicit_r15_bytes, hidden_return_count)`` and includes the
current procedure's frame plus its incoming hidden return.  Phase zero has no
further admitted recursive entry; a recursive call there uses the separately
recorded rejected-probe summary.  Phase 63 admits the full 64-entry profile.
Only the ``gen_expr`` row of an expression phase and the ``gen_stmts`` row of a
block phase feed the root recurrence; the uniform 70-row shape keeps Alpha-side
indexing simple without requiring a 64 x 64 counter product.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from dataclasses import dataclass
from pathlib import Path

from bc_block_control_map import load_parser


MAGIC = b"BCS9"

RANK_SIGNED_DIV10 = 1
RANK_EXPR_COUNTER = 2
RANK_BLOCK_COUNTER = 3

EXPECTED_SOURCE_BYTES = 32_045
EXPECTED_SOURCE_SHA256 = bytes.fromhex(
    "8beb76feda2783e6597998e5e7b41889c049faa2f604e627aead0f9d2b0cdbc8"
)
EXPECTED_PROCS = 70
EXPECTED_CALLS = 309
EXPECTED_PARAMS = 27
EXPECTED_LETS = 51
EXPECTED_SLOTS = 78
EXPECTED_NONEMPTY_FRAMES = 47
EXPECTED_MAX_TEMP_WORDS = 2

EXPR_SCC = frozenset({
    "gen_expr", "gen_sum", "gen_term", "gen_factor", "gen_load",
    "gen_call", "gen_write_byte",
})
BLOCK_SCC = frozenset({"gen_stmts", "gen_stmt", "gen_state", "gen_block"})
DECIMAL_SCC = frozenset({"emit_dec"})

U32 = struct.Struct("<I")


@dataclass(frozen=True)
class Procedure:
    index: int
    name: str
    params: int
    lets: int
    local_temp_words: int = 0
    emits: int = 0

    @property
    def slots(self) -> int:
        return self.params + self.lets

    @property
    def frame_bytes(self) -> int:
        return 8 * (1 + self.slots)


@dataclass(frozen=True)
class Call:
    caller: int
    callee: int
    ambient_words: int
    caller_ordinal: int


@dataclass(frozen=True)
class RecursiveScc:
    index: int
    members: tuple[int, ...]
    rank_kind: int
    limit: int
    internal_edges: int


@dataclass(frozen=True)
class Summary:
    explicit_bytes: int
    hidden_returns: int


@dataclass(frozen=True)
class Analysis:
    source_size: int
    source_sha256: bytes
    procedures: tuple[Procedure, ...]
    calls: tuple[Call, ...]
    sccs: tuple[RecursiveScc, ...]
    max_temp_words: int
    expr_probe: Summary
    block_probe: Summary
    decimal_summary: Summary
    expr_phases: tuple[tuple[Summary, ...], ...]
    block_phases: tuple[tuple[Summary, ...], ...]
    root_summaries: tuple[Summary, ...]


def count_lets(statements: list) -> int:
    return sum(
        1 if stmt[0] == "let" else (
            count_lets(stmt[2]) if stmt[0] == "state" else 0
        )
        for stmt in statements
    )


def statement_expressions(stmt) -> tuple:
    kind = stmt[0]
    if kind in ("let", "assign"):
        return (stmt[2],)
    if kind == "return":
        return (stmt[1],)
    if kind == "goto" and stmt[2] is not None:
        return (stmt[2],)
    if kind == "memset":
        # The address is pushed while the value is evaluated.  The caller adds
        # that one-word ambient adjustment when walking the second expression.
        return (stmt[2], stmt[3])
    if kind == "callstmt":
        return (stmt[1],)
    return ()


def count_emits(statements: list) -> int:
    return sum(
        1 if stmt[0] == "emit" else (
            count_emits(stmt[2]) if stmt[0] == "state" else 0
        )
        for stmt in statements
    )


def strongly_connected_components(
    procedure_count: int, calls: list[Call]
) -> list[tuple[int, ...]]:
    adjacency = [set() for _ in range(procedure_count)]
    for call in calls:
        adjacency[call.caller].add(call.callee)

    next_index = 0
    indices = [-1] * procedure_count
    lowlinks = [0] * procedure_count
    stack: list[int] = []
    on_stack = [False] * procedure_count
    components: list[tuple[int, ...]] = []

    def visit(node: int) -> None:
        nonlocal next_index
        indices[node] = next_index
        lowlinks[node] = next_index
        next_index += 1
        stack.append(node)
        on_stack[node] = True
        for target in sorted(adjacency[node]):
            if indices[target] < 0:
                visit(target)
                lowlinks[node] = min(lowlinks[node], lowlinks[target])
            elif on_stack[target]:
                lowlinks[node] = min(lowlinks[node], indices[target])
        if lowlinks[node] != indices[node]:
            return
        members: list[int] = []
        while True:
            member = stack.pop()
            on_stack[member] = False
            members.append(member)
            if member == node:
                break
        components.append(tuple(sorted(members)))

    for node in range(procedure_count):
        if indices[node] < 0:
            visit(node)
    return sorted(components, key=lambda members: members[0])


def solve_summaries(
    procedures: tuple[Procedure, ...], calls: list[Call],
    special_callees: dict[int, Summary], live: frozenset[int],
) -> tuple[Summary, ...]:
    """Solve the now-acyclic weighted call graph for one counter phase.

    ``special_callees`` cuts the three recursive entry targets.  The recursive
    procedure itself is still solved normally; only an edge *to* a cut target
    consumes the supplied lower-phase summary.
    """
    outgoing: list[list[Call]] = [[] for _ in procedures]
    for call in calls:
        outgoing[call.caller].append(call)
    solved: list[Summary | None] = [None] * len(procedures)
    visiting = [False] * len(procedures)

    def solve(index: int) -> Summary:
        if index not in live:
            return Summary(0, 0)
        if solved[index] is not None:
            return solved[index]  # type: ignore[return-value]
        if visiting[index]:
            raise ValueError(
                f"bc call bounds: phase cuts leave a cycle at {procedures[index].name}"
            )
        visiting[index] = True
        proc = procedures[index]
        explicit = proc.frame_bytes + 8 * proc.local_temp_words
        hidden = 2 if proc.emits else 1
        for call in outgoing[index]:
            callee = special_callees.get(call.callee)
            if callee is None:
                if call.callee not in live:
                    raise ValueError(
                        "bc call bounds: live phase edge escapes closure: "
                        f"{proc.name} -> {procedures[call.callee].name}"
                    )
                callee = solve(call.callee)
            explicit = max(
                explicit,
                proc.frame_bytes + 8 * call.ambient_words
                + callee.explicit_bytes,
            )
            hidden = max(hidden, 1 + callee.hidden_returns)
        visiting[index] = False
        result = Summary(explicit, hidden)
        solved[index] = result
        return result

    return tuple(solve(index) for index in range(len(procedures)))


def stopped_reachable(
    procedure_count: int, calls: list[Call], start: int,
    stops: frozenset[int], reject: frozenset[int] = frozenset(),
) -> frozenset[int]:
    outgoing: list[set[int]] = [set() for _ in range(procedure_count)]
    for call in calls:
        outgoing[call.caller].add(call.callee)
    live = {start}
    work = [start]
    while work:
        caller = work.pop()
        for callee in sorted(outgoing[caller]):
            if callee in reject:
                raise ValueError("bc call bounds: forbidden phase dependency")
            if callee in stops or callee in live:
                continue
            live.add(callee)
            work.append(callee)
    return frozenset(live)


def build_phase_tables(
    procedures: tuple[Procedure, ...], calls: list[Call]
) -> tuple[Summary, Summary, Summary,
           tuple[tuple[Summary, ...], ...],
           tuple[tuple[Summary, ...], ...], tuple[Summary, ...]]:
    by_name = {proc.name: proc.index for proc in procedures}
    expr = by_name["gen_expr"]
    block = by_name["gen_stmts"]
    decimal = by_name["emit_dec"]
    expr_live = stopped_reachable(
        len(procedures), calls, expr,
        frozenset({expr, decimal}), frozenset({block}),
    )
    block_live = stopped_reachable(
        len(procedures), calls, block, frozenset({expr, block, decimal})
    )
    root_live = stopped_reachable(
        len(procedures), calls, by_name["main"],
        frozenset({expr, block, decimal}),
    )

    # A rejected depth-65 call has already pushed its hidden return and run its
    # prologue before the guard executes.  In both exact procedures the rejection
    # path evaluates one comparison (one pushed left operand), sets RESOURCE_FAIL,
    # and returns; later two-word paths are unreachable after this failed guard.
    expr_probe = Summary(procedures[expr].frame_bytes + 8, 1)
    block_probe = Summary(procedures[block].frame_bytes + 8, 1)
    # At each recursive edge emit_dec has no ambient word live.  Signed n/10
    # strictly decreases for the guarded positive domain; INT64_MAX has 19
    # digits.  Only the deepest activation's local temporary high-water matters.
    decimal_summary = Summary(
        19 * procedures[decimal].frame_bytes
        + 8 * procedures[decimal].local_temp_words,
        19,
    )

    expr_phases: list[tuple[Summary, ...]] = []
    for remaining in range(64):
        nested_expr = expr_probe if remaining == 0 else expr_phases[-1][expr]
        expr_phases.append(solve_summaries(
            procedures,
            calls,
            {expr: nested_expr, decimal: decimal_summary}, expr_live,
        ))

    block_phases: list[tuple[Summary, ...]] = []
    for remaining in range(64):
        nested_block = block_probe if remaining == 0 else block_phases[-1][block]
        block_phases.append(solve_summaries(
            procedures,
            calls,
            {
                expr: expr_phases[-1][expr],
                block: nested_block,
                decimal: decimal_summary,
            },
            block_live,
        ))

    root = solve_summaries(
        procedures,
        calls,
        {
            expr: expr_phases[-1][expr],
            block: block_phases[-1][block],
            decimal: decimal_summary,
        },
        root_live,
    )
    return (expr_probe, block_probe, decimal_summary,
            tuple(expr_phases), tuple(block_phases), root)


def analyze(repo: Path, source: bytes) -> Analysis:
    digest = hashlib.sha256(source).digest()
    if len(source) != EXPECTED_SOURCE_BYTES or digest != EXPECTED_SOURCE_SHA256:
        raise ValueError(
            "bc call bounds: source is not the exact frozen bc.beta "
            f"({len(source)} bytes, sha256 {digest.hex()})"
        )

    parser = load_parser(repo)
    ast = parser.Parser(parser.lex(source.decode("utf-8"))).parse()
    procedures = tuple(
        Procedure(index, proc[1], len(proc[2]), count_lets(proc[3]),
                  emits=count_emits(proc[3]))
        for index, proc in enumerate(ast)
    )
    by_name = {proc.name: proc.index for proc in procedures}
    if len(by_name) != len(procedures):
        raise ValueError("bc call bounds: duplicate procedure name")

    calls: list[Call] = []
    max_temp_words = 0
    proc_temp_words = [0] * len(procedures)

    def note_height(caller: int, words: int) -> None:
        nonlocal max_temp_words
        max_temp_words = max(max_temp_words, words)
        proc_temp_words[caller] = max(proc_temp_words[caller], words)

    def walk_expr(expr, caller: int, ambient: int, ordinals: list[int]) -> None:
        kind = expr[0]
        if kind == "bin":
            walk_expr(expr[2], caller, ambient, ordinals)
            note_height(caller, ambient + 1)
            walk_expr(expr[3], caller, ambient + 1, ordinals)
            return
        if kind == "mem":
            walk_expr(expr[2], caller, ambient, ordinals)
            return
        if kind != "call":
            return

        name, arguments = expr[1], expr[2]
        callee = by_name.get(name)
        if callee is not None:
            ordinal = ordinals[0]
            ordinals[0] += 1
            # The actual call follows reverse pops of all staged arguments, so
            # its ambient height is the height on entry to this call expression.
            calls.append(Call(caller, callee, ambient, ordinal))
            for argument_index, argument in enumerate(arguments):
                walk_expr(argument, caller, ambient + argument_index, ordinals)
                note_height(caller, ambient + argument_index + 1)
            return
        if name == "read_byte":
            if arguments:
                raise ValueError("bc call bounds: read_byte with arguments")
            return
        if name == "write_byte":
            if len(arguments) != 1:
                raise ValueError("bc call bounds: write_byte arity is not one")
            walk_expr(arguments[0], caller, ambient, ordinals)
            return
        raise ValueError(f"bc call bounds: unresolved call {name!r}")

    def walk_statements(statements: list, caller: int, ordinals: list[int]) -> None:
        nonlocal max_temp_words
        for stmt in statements:
            if stmt[0] == "state":
                walk_statements(stmt[2], caller, ordinals)
                continue
            expressions = statement_expressions(stmt)
            if stmt[0] == "memset":
                walk_expr(expressions[0], caller, 0, ordinals)
                note_height(caller, 1)
                walk_expr(expressions[1], caller, 1, ordinals)
            else:
                for expression in expressions:
                    walk_expr(expression, caller, 0, ordinals)

    for proc, ast_proc in zip(procedures, ast):
        walk_statements(ast_proc[3], proc.index, [0])

    procedures = tuple(
        Procedure(proc.index, proc.name, proc.params, proc.lets,
                  proc_temp_words[proc.index], proc.emits)
        for proc in procedures
    )

    components = strongly_connected_components(len(procedures), calls)
    edge_pairs = {(call.caller, call.callee) for call in calls}
    recursive_components = [
        members for members in components
        if len(members) > 1 or (members[0], members[0]) in edge_pairs
    ]

    expected_ranks = {
        DECIMAL_SCC: (RANK_SIGNED_DIV10, 19),
        EXPR_SCC: (RANK_EXPR_COUNTER, 64),
        BLOCK_SCC: (RANK_BLOCK_COUNTER, 64),
    }
    sccs: list[RecursiveScc] = []
    seen_names: set[frozenset[str]] = set()
    for index, members in enumerate(recursive_components):
        names = frozenset(procedures[member].name for member in members)
        if names not in expected_ranks:
            raise ValueError(f"bc call bounds: unexpected recursive SCC {sorted(names)}")
        seen_names.add(names)
        rank_kind, limit = expected_ranks[names]
        internal_edges = sum(
            call.caller in members and call.callee in members for call in calls
        )
        sccs.append(RecursiveScc(index, members, rank_kind, limit, internal_edges))
    if seen_names != set(expected_ranks):
        missing = [sorted(names) for names in set(expected_ranks) - seen_names]
        raise ValueError(f"bc call bounds: missing recursive SCCs {missing}")

    recursive_members = {member for scc in sccs for member in scc.members}
    for call in calls:
        if (call.caller in recursive_members and call.callee in recursive_members
                and any(call.caller in scc.members and call.callee in scc.members
                        for scc in sccs)
                and call.ambient_words != 0):
            raise ValueError("bc call bounds: recursive edge carries temporaries")

    if len(procedures) != EXPECTED_PROCS:
        raise ValueError(f"bc call bounds: {len(procedures)} procedures")
    if len(calls) != EXPECTED_CALLS:
        raise ValueError(f"bc call bounds: {len(calls)} ordinary calls")
    if sum(proc.params for proc in procedures) != EXPECTED_PARAMS:
        raise ValueError("bc call bounds: parameter total changed")
    if sum(proc.lets for proc in procedures) != EXPECTED_LETS:
        raise ValueError("bc call bounds: let total changed")
    if sum(proc.slots for proc in procedures) != EXPECTED_SLOTS:
        raise ValueError("bc call bounds: slot total changed")
    if sum(proc.slots > 0 for proc in procedures) != EXPECTED_NONEMPTY_FRAMES:
        raise ValueError("bc call bounds: nonempty-frame total changed")
    if max_temp_words != EXPECTED_MAX_TEMP_WORDS:
        raise ValueError(f"bc call bounds: max temporary words is {max_temp_words}")

    (expr_probe, block_probe, decimal_summary, expr_phases, block_phases,
     root_summaries) = build_phase_tables(procedures, calls)
    main_summary = root_summaries[by_name["main"]]
    if main_summary != Summary(12_720, 662):
        raise ValueError(
            "bc call bounds: root summary changed: "
            f"explicit={main_summary.explicit_bytes} "
            f"hidden={main_summary.hidden_returns}"
        )

    return Analysis(
        len(source), digest, procedures, tuple(calls), tuple(sccs), max_temp_words,
        expr_probe, block_probe, decimal_summary,
        expr_phases, block_phases, root_summaries,
    )


def encode(
    analysis: Analysis, *, underreport_probe: bool = False,
    underreport_root: bool = False,
) -> bytes:
    expr_phases = [list(phase) for phase in analysis.expr_phases]
    root_summaries = list(analysis.root_summaries)
    expr_index = next(
        proc.index for proc in analysis.procedures if proc.name == "gen_expr"
    )
    main_index = next(
        proc.index for proc in analysis.procedures if proc.name == "main"
    )
    if underreport_probe:
        summary = expr_phases[0][expr_index]
        expr_phases[0][expr_index] = Summary(
            summary.explicit_bytes - 8, summary.hidden_returns
        )
    if underreport_root:
        summary = root_summaries[main_index]
        root_summaries[main_index] = Summary(
            summary.explicit_bytes - 8, summary.hidden_returns - 1
        )

    out = bytearray(MAGIC)
    for phases, field in (
        (expr_phases, "explicit_bytes"),
        (expr_phases, "hidden_returns"),
        (analysis.block_phases, "explicit_bytes"),
        (analysis.block_phases, "hidden_returns"),
    ):
        for phase in phases:
            for summary in phase:
                out.extend(U32.pack(getattr(summary, field)))
    for field in ("explicit_bytes", "hidden_returns"):
        for summary in root_summaries:
            out.extend(U32.pack(getattr(summary, field)))
    return bytes(out)


def report(analysis: Analysis) -> str:
    names = [proc.name for proc in analysis.procedures]
    main_index = names.index("main")
    outgoing: list[list[Call]] = [[] for _ in analysis.procedures]
    for call in analysis.calls:
        outgoing[call.caller].append(call)
    lines = [
        "bc call bounds report",
        f"source {analysis.source_size} bytes sha256 {analysis.source_sha256.hex()}",
        f"procedures {len(analysis.procedures)} calls {len(analysis.calls)} "
        f"slots {sum(proc.slots for proc in analysis.procedures)} "
        f"nonempty_frames {sum(proc.slots > 0 for proc in analysis.procedures)} "
        f"max_temp_words {analysis.max_temp_words}",
        f"phase tables expr={len(analysis.expr_phases)}x{len(analysis.procedures)} "
        f"block={len(analysis.block_phases)}x{len(analysis.procedures)} "
        f"root={len(analysis.root_summaries)}",
        f"probes expr={analysis.expr_probe.explicit_bytes}/"
        f"{analysis.expr_probe.hidden_returns} "
        f"block={analysis.block_probe.explicit_bytes}/"
        f"{analysis.block_probe.hidden_returns} "
        f"emit_dec={analysis.decimal_summary.explicit_bytes}/"
        f"{analysis.decimal_summary.hidden_returns}",
        f"root main conservative bound="
        f"{analysis.root_summaries[main_index].explicit_bytes} explicit bytes / "
        f"{analysis.root_summaries[main_index].hidden_returns} hidden returns",
        "procedures:",
    ]
    for proc in analysis.procedures:
        edges: dict[tuple[str, int], int] = {}
        for call in outgoing[proc.index]:
            key = (names[call.callee], call.ambient_words)
            edges[key] = edges.get(key, 0) + 1
        rendered = ", ".join(
            f"{callee}@{ambient}" + (f"x{count}" if count != 1 else "")
            for (callee, ambient), count in edges.items()
        ) or "-"
        lines.append(
            f"  {proc.index:02d} {proc.name}: params={proc.params} "
            f"lets={proc.lets} slots={proc.slots} frame={proc.frame_bytes} "
            f"temps={proc.local_temp_words} emits={proc.emits} "
            f"calls=[{rendered}]"
        )
    lines.append("recursive SCCs:")
    rank_names = {
        RANK_SIGNED_DIV10: "signed-div10",
        RANK_EXPR_COUNTER: "expr-counter",
        RANK_BLOCK_COUNTER: "block-counter",
    }
    for scc in analysis.sccs:
        lines.append(
            f"  {scc.index}: {','.join(names[index] for index in scc.members)} "
            f"rank={rank_names[scc.rank_kind]} limit={scc.limit} "
            f"internal_edges={scc.internal_edges}"
        )
    lines.append("call ambient depths are words live at the actual Alpha call")
    return "\n".join(lines) + "\n"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", type=Path, required=True)
    ap.add_argument("--source", type=Path, required=True)
    ap.add_argument("--output", type=Path)
    ap.add_argument("--underreport-probe-output", type=Path)
    ap.add_argument("--underreport-root-output", type=Path)
    ap.add_argument("--report", action="store_true")
    args = ap.parse_args()
    if (args.output is None and args.underreport_probe_output is None
            and args.underreport_root_output is None and not args.report):
        ap.error("an output or --report is required")

    analysis = analyze(args.repo.resolve(), args.source.read_bytes())
    if args.output is not None:
        args.output.write_bytes(encode(analysis))
    if args.underreport_probe_output is not None:
        args.underreport_probe_output.write_bytes(
            encode(analysis, underreport_probe=True)
        )
    if args.underreport_root_output is not None:
        args.underreport_root_output.write_bytes(
            encode(analysis, underreport_root=True)
        )
    if args.report:
        print(report(analysis), end="")


if __name__ == "__main__":
    main()

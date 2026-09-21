"""Full-subject mutation controls for the Beta encoding certificate.

Builds the complete owner-fixed derivation exactly as --full-subject does,
then emits mutated checker requests covering the acceptance axes in
bootstrap/proofs/beta_encoding/ACCEPTANCE.md: source, tape, assertions,
theory identity, rules, clauses, substitution, premises, sorts, arities,
partition joints, endpoints, final-root selection, and rejection after a
valid prefix.  Every mutation carries the exact checker diagnostic the
formed request must publish, derived from the coordinate and code
conventions documented in bootstrap/proofs/checker/CHECKING.md and
FORMAT.md:

- code 6  physical decode failure (unknown tag, mismatched counts,
          extent escapes, trailing section bytes),
- code 10 unfolding clause/head failure at the clause field,
- code 12 derivation failure: invalid references/premises, sort
          mismatches at the right field, and structural-compare misses at
          the field owning the requirement,
- tag 7   Checked, payload (proof count, work).

The checker inside the evaluator is the only rejection authority, so the
verdict leg runs only on hosts with an evaluator seed (macOS arm64,
Windows x64), like the other evaluator gates:

    tests/gamma/beta-encoding-theory/run.sh --mutations

On any host with python3, `--self-test` reproduces the certificate,
constructs every mutated request, and verifies each patch lands at the
claimed field — without asserting any checker verdict:

    python3 tests/gamma/beta-encoding-theory/mutations.py --self-test
"""

import hashlib
import struct
import subprocess
import sys
import threading
import time
from pathlib import Path

GATE_DIR = Path(__file__).resolve().parent
sys.path.append(str(GATE_DIR))
sys.path.append(str(GATE_DIR.parent / "derivation-layout"))
sys.path.append(str(GATE_DIR.parent / "derivation-checking"))

import full_subject  # noqa: E402
import identity  # noqa: E402
from proof_wire import failure  # noqa: E402
from wire import envelope, words  # noqa: E402

PHYSICAL = 6
CLAUSE = 10
DERIVATION = 12

# Word-field offsets inside a proof record: extent@0, rule@+4, left@+8,
# right@+12, then rule fields at +16, +20, +24.
RULE = 4
LEFT = 8
RIGHT = 12
FIELD = 16
FIELD2 = 20

# Term record: extent@0, tag@+4, symbol@+8, argc@+12, children@+16..
SYMBOL = 8
ARGC = 12

# Scan bounds: mutations pin the earliest rows so the checker's in-order
# row scan rejects quickly; full-table cases inherently scan everything.
UNFOLD_SCAN_LIMIT = 300_000


def record_offsets(rows, start):
    """Absolute byte offset of each record's extent word in a table."""
    offsets = []
    offset = start
    for row in rows:
        offsets.append(offset)
        offset += 4 + 4 * len(row)
    return offsets, offset


def layout(ctx):
    """Absolute request offsets for every field a mutation touches."""
    theory_off = 24
    owner_off = theory_off + len(ctx["theory_bytes"])
    certificate_off = owner_off + len(ctx["owner_section"])
    owner_records, roots_off = record_offsets(ctx["owners"], owner_off + 8)
    if roots_off + 8 != owner_off + len(ctx["owner_section"]):
        raise SystemExit("mutations: owner section layout mismatch")
    witness_records, proof_count_off = record_offsets(
        ctx["witnesses"], certificate_off + 8)
    proof_records, end = record_offsets(ctx["proofs"], proof_count_off + 4)
    if end != certificate_off + len(ctx["certificate_section"]):
        raise SystemExit("mutations: certificate section layout mismatch")
    return {
        "theory_off": theory_off,
        "certificate_off": certificate_off,
        "owner_records": owner_records,
        "roots_off": roots_off,
        "witness_records": witness_records,
        "proof_count_off": proof_count_off,
        "proof_records": proof_records,
        "proof_table_end": end,
    }


def term_record(ctx, ref):
    owners = ctx["owners"]
    if ref <= len(owners):
        return owners[ref - 1]
    return ctx["witnesses"][ref - len(owners) - 1]


def term_record_off(ctx, offsets, ref):
    owners = ctx["owners"]
    if ref <= len(owners):
        return offsets["owner_records"][ref - 1]
    return offsets["witness_records"][ref - len(owners) - 1]


def term_sort(ctx, ref):
    """Inferred result sort of a ground term's root symbol."""
    tag, symbol = term_record(ctx, ref)[:2]
    if tag == 1:
        return ctx["theory"].constructors[symbol - 1][0]
    return ctx["theory"].functions[symbol - 1][0]


def alternative_symbol(ctx, record):
    """A different same-arity symbol in the term's own namespace,
    preferring a different result sort so the sort check also fires."""
    tag, symbol, argc = record[0], record[1], record[2]
    table = (ctx["theory"].constructors if tag == 1
             else [(f[0], len(f[1])) for f in ctx["theory"].functions])
    fallback = None
    for identity_, candidate in enumerate(table, 1):
        if identity_ == symbol or candidate[1] != argc:
            continue
        fallback = identity_
        if candidate[0] != table[symbol - 1][0]:
            return identity_
    return fallback


def patch_word(request, offset, value):
    """The request with one 32-bit word replaced."""
    return request[:offset] + words(value) + request[offset + 4:]


def first_citing_row(proofs, ref):
    """First row citing a term as an endpoint, with the owning field."""
    for index, row in enumerate(proofs):
        if row[2] == ref:
            return index, RIGHT
        if row[1] == ref:
            return index, LEFT
    raise SystemExit("mutations: term is never a row endpoint")


def unfold_pairs(ctx, proofs, limit=UNFOLD_SCAN_LIMIT):
    """(row_index, function_symbol, clause_ordinal) for early unfolds."""
    for index, row in enumerate(proofs[:limit]):
        if row[0] == 5:
            yield index, term_record(ctx, row[1])[1], row[3]


def theory_function_offsets(theory_bytes):
    """Per-function clause-body word offsets inside the GTH1 section.

    Returns a list indexed by function identity minus one of
    [body_word_offset per clause].
    """
    offset = 8
    (ctor_count,) = struct.unpack_from("<I", theory_bytes, offset)
    offset += 4
    for _ in range(ctor_count):
        (length,) = struct.unpack_from("<I", theory_bytes, offset)
        offset += 4 + 4 * length
    (fn_count,) = struct.unpack_from("<I", theory_bytes, offset)
    offset += 4
    functions = []
    for _ in range(fn_count):
        record_off = offset
        (length,) = struct.unpack_from("<I", theory_bytes, offset)
        argc = struct.unpack_from("<I", theory_bytes, offset + 8)[0]
        clause_count = struct.unpack_from(
            "<I", theory_bytes, offset + 4 + 4 * (4 + argc))[0]
        cursor = offset + 4 + 4 * (5 + argc)
        bodies = []
        for _ in range(clause_count):
            (clause_len,) = struct.unpack_from("<I", theory_bytes, cursor)
            bodies.append(cursor + 4 * clause_len)
            cursor += 4 + 4 * clause_len
        if cursor != record_off + 4 + 4 * length:
            raise SystemExit("mutations: function record walk mismatch")
        functions.append(bodies)
        offset = cursor
    return functions


def ground_tuple(ctx, ref, memo):
    """Structural form of a ground term as nested tuples, iteratively."""
    cached = memo.get(ref)
    if cached is not None:
        return cached
    memo[ref] = None  # cycle sentinel is never hit on ground terms
    stack = [ref]
    while stack:
        current = stack[-1]
        if memo.get(current):
            stack.pop()
            continue
        record = term_record(ctx, current)
        children = record[3:]
        if all(memo.get(child) for child in children):
            memo[current] = (record[0], record[1],
                             tuple(memo[child] for child in children))
            stack.pop()
        else:
            stack.extend(child for child in children
                         if not memo.get(child))
    return memo[ref]


def clause_environment(ctx, function, clause, arguments):
    """Binding environment for a clause application per FORMAT.md:
    mode-0 binds all arguments; mode-1 leaves the selected slot unbound
    and binds the matched constructor's children after the arguments."""
    result, signature, mode, selected, _ = function
    arity = len(signature)
    if mode == 0:
        return list(arguments), arity
    env = list(arguments)
    env[selected] = None
    selected_child = term_record(ctx, arguments[selected])
    env.extend(selected_child[3:])
    return env, arity


def instantiate(clause, env, ctx, memo):
    """Instantiate a clause template under the derived environment,
    producing a structural tuple comparable to a ground term."""
    templates = clause[1]
    cache = {}

    def row(index):
        if index in cache:
            return cache[index]
        template = templates[index - 1]
        tag = template[0]
        if tag == 0:
            bound = env[template[1]]
            if bound is None:
                raise SystemExit(
                    "mutations: template binds the unbound selected slot")
            result = ground_tuple(ctx, bound, memo)
        else:
            result = (tag, template[1],
                      tuple(row(child) for child in template[3:]))
        cache[index] = result
        return result

    return row(clause[2])


def clause_body_mutation(ctx, proofs, function_bodies):
    """Earliest unfold citing a clause with a spare template that
    instantiates differently under that row's own environment; retargeting
    the body changes the derived instance, not just the syntax."""
    memo = {}
    for index, symbol, ordinal in unfold_pairs(ctx, proofs):
        function = ctx["theory"].functions[symbol - 1]
        clause = function[4][ordinal - 1]
        templates, body = clause[1], clause[2]
        arguments = term_record(ctx, proofs[index][1])[3:]
        env, _ = clause_environment(ctx, function, clause, arguments)
        original = instantiate(clause, env, ctx, memo)
        for replacement in range(1, len(templates) + 1):
            if replacement == body:
                continue
            replaced = (*clause[:2], replacement)
            if instantiate(replaced, env, ctx, memo) != original:
                body_off = function_bodies[symbol - 1][ordinal - 1]
                return index, body_off, replacement
    return None


def substitution_mutation(ctx, proofs):
    """An unfold row plus a sibling application of the same function that
    still satisfies the stated clause but derives a different instance:
    mode-0 binds every argument; mode-1 requires the selected argument to
    keep the clause's constructor."""
    memo = {}
    pairs = list(unfold_pairs(ctx, proofs))
    for index, symbol, ordinal in pairs:
        function = ctx["theory"].functions[symbol - 1]
        mode, selected = function[2], function[3]
        clause = function[4][ordinal - 1]
        clause_ctor = clause[0]
        arguments = term_record(ctx, proofs[index][1])[3:]
        env, _ = clause_environment(ctx, function, clause, arguments)
        original_instance = instantiate(clause, env, ctx, memo)
        for other, _, _ in pairs[:400]:
            if other == index:
                continue
            candidate = proofs[other][1]
            record = term_record(ctx, candidate)
            if record[0] != 2 or record[1] != symbol:
                continue
            if mode == 1:
                selected_child = term_record(ctx, record[3 + selected])
                if selected_child[1] != clause_ctor:
                    continue
            env, _ = clause_environment(
                ctx, function, clause, record[3:])
            if instantiate(clause, env, ctx, memo) != original_instance:
                return index, candidate
    return None


def extend_certificate(ctx, extra_proof_bytes, count_delta):
    """The request with the proof table's count bumped and extra record
    bytes appended inside the certificate section."""
    certificate = ctx["certificate_section"]
    proof_count_in_section = offsets_certificate_count(ctx)
    head = certificate[:proof_count_in_section]
    proofs_bytes = certificate[proof_count_in_section + 4:]
    total = len(ctx["proofs"])
    extended = (head + words(total + count_delta)
                + proofs_bytes + extra_proof_bytes)
    return envelope((ctx["theory_bytes"], ctx["owner_section"], extended))


def offsets_certificate_count(ctx):
    """The witness table's byte extent inside the certificate section."""
    return 4 + 4 + sum(4 + 4 * len(row) for row in ctx["witnesses"])


def build_cases(ctx):
    """(name, request, expectation) vectors over the live certificate."""
    proofs = ctx["proofs"]
    request = ctx["request"]
    offsets = layout(ctx)
    owner_count = len(ctx["owners"])
    proof_offs = offsets["proof_records"]
    total = len(proofs)
    last = total - 1
    last_end = proof_offs[last] + 4 + 4 * len(proofs[last])

    cases = []
    # Positive control: the unmutated certificate must check.
    cases.append(("unmutated_certificate_checks", request,
                  ("checked", total)))

    # Theory identity: retarget a cited clause's body to a different
    # template row; the first citing unfold derives a different instance
    # and misses at its claimed-right field.
    found = clause_body_mutation(ctx, proofs,
                                 theory_function_offsets(
                                     ctx["theory_bytes"]))
    if found is None:
        raise SystemExit("mutations: no multi-template cited clause found")
    cite_row, body_off, replacement = found
    mutated = patch_word(request, offsets["theory_off"] + body_off,
                         replacement)
    cases.append(("theory_identity_clause_body", mutated,
                  ("failure", proof_offs[cite_row] + RIGHT, DERIVATION)))

    # Source and tape sides: corrupt the root symbol of an early and the
    # last unfolding's claimed-right instance.  The first row citing the
    # term as an endpoint rejects at its owning field.
    first_unfold = next(i for i, row in enumerate(proofs) if row[0] == 5)
    last_unfold = next(i for i in range(total - 1, -1, -1)
                       if proofs[i][0] == 5)
    for axis, row_index in (("source_stream", first_unfold),
                            ("tape_stream", last_unfold)):
        right_ref = proofs[row_index][2]
        record = term_record(ctx, right_ref)
        replacement = alternative_symbol(ctx, record)
        if replacement is None:
            raise SystemExit(
                f"mutations: no alternative symbol for {axis}")
        record_off = term_record_off(ctx, offsets, right_ref)
        mutated = patch_word(request, record_off + SYMBOL, replacement)
        cite, field = first_citing_row(proofs, right_ref)
        cases.append((f"{axis}_instance_root", mutated,
                      ("failure", proof_offs[cite] + field, DERIVATION)))

    # Rules: an unknown rule tag is a physical decode failure at the rule
    # field itself.
    row_index = next(i for i, row in enumerate(proofs) if row[0] == 3)
    mutated = patch_word(request, proof_offs[row_index] + RULE, 9)
    cases.append(("rule_tag_unknown", mutated,
                  ("failure", proof_offs[row_index] + RULE, PHYSICAL)))

    # Clauses: ordinal zero is out of range; a different declared clause
    # fails the stated-clause head check — both code 10 at the clause
    # field.
    mutated = patch_word(request, proof_offs[first_unfold] + FIELD, 0)
    cases.append(("clause_ordinal_zero", mutated,
                  ("failure", proof_offs[first_unfold] + FIELD, CLAUSE)))
    # A wrong ordinal must trip the clause head check, so pick a mode-1
    # row whose replacement clause heads a different constructor.  On a
    # mode-0 clause the retarget would only derive a different instance
    # (code 12 at the claimed right), not a clause failure.
    wrong = None
    for index, symbol, ordinal in unfold_pairs(ctx, proofs):
        function = ctx["theory"].functions[symbol - 1]
        declared = function[4]
        if function[2] != 1 or len(declared) <= 1:
            continue
        replacement_ordinal = ordinal % len(declared) + 1
        if declared[replacement_ordinal - 1][0] != declared[ordinal - 1][0]:
            wrong = (index, replacement_ordinal)
            break
    if wrong is None:
        raise SystemExit("mutations: no multi-clause mode-1 unfold row")
    index, replacement = wrong
    mutated = patch_word(request, proof_offs[index] + FIELD, replacement)
    cases.append(("clause_ordinal_wrong_case", mutated,
                  ("failure", proof_offs[index] + FIELD, CLAUSE)))

    # Substitution: same clause, sibling application; the derived
    # environment produces a different instance — compare misses at the
    # claimed-right field.
    found = substitution_mutation(ctx, proofs)
    if found is None:
        raise SystemExit("mutations: no sibling application found")
    index, candidate = found
    mutated = patch_word(request, proof_offs[index] + LEFT, candidate)
    cases.append(("substitution_sibling_environment", mutated,
                  ("failure", proof_offs[index] + RIGHT, DERIVATION)))

    # Premises: repointing a transitivity premise to an earlier row whose
    # left differs misses the first structural compare at the claimed
    # left field; a self-premise is invalid at its own field.
    transitivity = next(i for i, row in enumerate(proofs) if row[0] == 3)
    claimed_left = proofs[transitivity][1]
    replacement = next((candidate for candidate in range(1, transitivity + 1)
                        if proofs[candidate - 1][1] != claimed_left), None)
    if replacement is None:
        raise SystemExit("mutations: no earlier row with a different left")
    mutated = patch_word(request, proof_offs[transitivity] + FIELD,
                         replacement)
    cases.append(("premise_wrong_row", mutated,
                  ("failure", proof_offs[transitivity] + LEFT, DERIVATION)))
    mutated = patch_word(request, proof_offs[transitivity] + FIELD2,
                         transitivity + 1)
    cases.append(("premise_self_reference", mutated,
                  ("failure", proof_offs[transitivity] + FIELD2,
                       DERIVATION)))

    # Arity: an owner term's declared child count disagrees with the
    # record's fields — a physical count mismatch at the count word.
    owner_off = offsets["owner_records"][0]
    mutated = patch_word(request, owner_off + ARGC,
                         ctx["owners"][0][2] + 1)
    cases.append(("term_arity_count", mutated,
                  ("failure", owner_off + ARGC, PHYSICAL)))

    # Partition joint: the last row's right points across the
    # owner/witness boundary at the first witness term.
    mutated = patch_word(request, proof_offs[last] + RIGHT, owner_count + 1)
    cases.append(("partition_joint_witness_boundary", mutated,
                  ("failure", proof_offs[last] + RIGHT, DERIVATION)))

    # Endpoints: repoint a transitivity's claimed left to a same-sort
    # different term — first compare misses at +8 — and its claimed
    # right — the third compare misses at +12.
    left_sort = term_sort(ctx, claimed_left)
    wrong_left = None
    for ref in range(1, owner_count + len(ctx["witnesses"])):
        if ref != claimed_left and term_sort(ctx, ref) == left_sort:
            wrong_left = ref
            break
    if wrong_left is None:
        raise SystemExit("mutations: no same-sort term for claimed left")
    mutated = patch_word(request, proof_offs[transitivity] + LEFT,
                         wrong_left)
    cases.append(("endpoint_claimed_left", mutated,
                  ("failure", proof_offs[transitivity] + LEFT, DERIVATION)))
    second = next(i for i in range(transitivity + 1, total)
                  if proofs[i][0] == 3)
    right_sort = term_sort(ctx, proofs[second][2])
    wrong_right = None
    for ref in range(1, owner_count + len(ctx["witnesses"])):
        if ref != proofs[second][2] and term_sort(ctx, ref) == right_sort:
            wrong_right = ref
            break
    if wrong_right is None:
        raise SystemExit("mutations: no same-sort term for claimed right")
    mutated = patch_word(request, proof_offs[second] + RIGHT, wrong_right)
    cases.append(("endpoint_claimed_right", mutated,
                  ("failure", proof_offs[second] + RIGHT, DERIVATION)))

    # Final-root selection: the owner roots compare against the last
    # row's endpoints; each requirement belongs to that row's field.
    mutated = patch_word(request, offsets["roots_off"], ctx["right_ref"])
    cases.append(("final_root_left_selection", mutated,
                  ("failure", proof_offs[last] + LEFT, DERIVATION)))
    mutated = patch_word(request, offsets["roots_off"] + 4,
                         ctx["left_ref"])
    cases.append(("final_root_right_selection", mutated,
                  ("failure", proof_offs[last] + RIGHT, DERIVATION)))

    # A valid prefix does not excuse a following row: append a
    # reflexivity over distinct terms (compare miss at its right field)
    # and a record whose extent escapes the section (physical reject at
    # the extent word).
    bad_row = words(3, 1, 1, 2)
    mutated = extend_certificate(ctx, bad_row, 1)
    cases.append(("valid_prefix_trailing_bad_row", mutated,
                  ("failure", last_end + RIGHT, DERIVATION)))
    escaped = words(99, 7)
    mutated = extend_certificate(ctx, escaped, 1)
    cases.append(("valid_prefix_extent_escape", mutated,
                  ("failure", last_end, PHYSICAL)))

    return cases


def invoke(evaluator, name, source, request, timeout=900):
    framed = struct.pack("<I", len(source)) + source + request
    if len(framed) > 137363456:
        raise SystemExit(
            f"mutations {name}: outside evaluator request")
    started = time.monotonic()
    try:
        result = subprocess.run(
            [str(evaluator)], input=framed, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise SystemExit(
            f"mutations {name}: host timeout {timeout}s; no checker result")
    return result, time.monotonic() - started


def check(name, result, expected, elapsed):
    kind = expected[0]
    if kind == "checked":
        _, count = expected
        head = b"\x07" + struct.pack("<Q", count)
        if result.returncode != 0 or result.stderr or \
                not result.stdout.startswith(head) or \
                len(result.stdout) != 17:
            raise SystemExit(
                f"mutations {name}: expected Checked({count}), got "
                f"{result.returncode}/{result.stdout.hex()} "
                f"stderr={result.stderr!r}")
        steps = struct.unpack_from("<Q", result.stdout, 9)[0]
        print(f"mutations {name}: Checked rows={count} work={steps} "
              f"({elapsed:.3f}s)", flush=True)
        return
    _, coordinate, code = expected
    want = failure(coordinate, code)
    if (result.returncode, result.stdout, result.stderr) != \
            (0, want, b""):
        raise SystemExit(
            f"mutations {name}: expected 0/{want.hex()}, got "
            f"{result.returncode}/{result.stdout.hex()} "
            f"stderr={result.stderr!r}, elapsed={elapsed:.3f}s")
    print(f"mutations {name}: exact {want.hex()} ({elapsed:.3f}s)",
          flush=True)


def main():
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        self_test()
        return
    if len(sys.argv) != 2:
        raise SystemExit(
            "usage: mutations.py PREPARED_DIRECTORY|--self-test")
    prepared = Path(sys.argv[1]).resolve()
    evaluator = prepared / "evaluator"
    checker = (prepared / "checker.gamma").read_bytes()
    producer = (prepared / "producer.gamma").read_bytes()

    emitted, _ = invoke(evaluator, "emit", producer, b"", timeout=600)
    if emitted.returncode != 0 or emitted.stderr:
        raise SystemExit("mutations: producer emission failed")
    theory_bytes = emitted.stdout
    expected = identity.fixed_identity()
    actual = (len(theory_bytes), hashlib.sha256(theory_bytes).hexdigest())
    if actual != expected:
        raise SystemExit(f"mutations: theory identity {actual}")
    print(f"mutations: theory bytes={actual[0]} sha256={actual[1]}",
          flush=True)

    ctx = full_subject.build()
    cases = build_cases(ctx)
    print(f"mutations: {len(cases)} vectors over "
          f"{len(ctx['request'])}-byte request", flush=True)
    for name, request, expectation in cases:
        result, elapsed = invoke(evaluator, name, checker, request)
        check(name, result, expectation, elapsed)
    print(f"mutations: {len(cases)} vectors passed; checker verdicts "
          "only", flush=True)


def self_test():
    """Host-side coverage of production and vector construction."""
    ctx = full_subject.build()
    cases = build_cases(ctx)
    names = set()
    for name, request, expectation in cases:
        if name in names:
            raise SystemExit(f"mutations: duplicate vector {name}")
        names.add(name)
        if request == ctx["request"] and \
                name != "unmutated_certificate_checks":
            raise SystemExit(f"mutations {name}: no mutation applied")
        if request[:8] != b"GDREQ\x01\x00\x00":
            raise SystemExit(f"mutations {name}: envelope magic lost")
        if expectation[0] == "failure":
            _, coordinate, code = expectation
            if coordinate <= 0 or coordinate >= len(request):
                raise SystemExit(
                    f"mutations {name}: coordinate {coordinate} escapes")
            if len(failure(coordinate, code)) != 33:
                raise SystemExit(
                    f"mutations {name}: bad diagnostic shape")
    print(f"mutations self-test: {len(cases)} vectors constructed over "
          f"{len(ctx['request'])}-byte request; checker verdicts require "
          "the evaluator hosts", flush=True)


if __name__ == "__main__":
    sys.setrecursionlimit(1_000_000)
    try:
        threading.stack_size(512 * 1024 * 1024)
    except (ValueError, RuntimeError):
        pass
    # The worker's outcome has to reach the main thread: `threading`
    # discards a `SystemExit` raised in a non-main thread and exits 0, so
    # joining alone turns every refusal this gate makes into a silent pass.
    # Same shape `full_subject.py` already uses for its deep-fold worker.
    # Not named `failure`: that is this module's diagnostic helper, and
    # rebinding it at module scope breaks `self_test`.
    worker_outcome = []

    def run():
        try:
            main()
        except BaseException as error:  # noqa: BLE001 - re-raised below
            worker_outcome.append(error)

    worker = threading.Thread(target=run)
    worker.start()
    worker.join()
    if worker_outcome:
        raise worker_outcome[0]

#!/usr/bin/env python3
"""Authoring generator for the encoder definition section of the Beta
encoding theory.

Reads the independent declarations in ``encoding.py`` and writes the Gamma
sources under ``bootstrap/proofs/beta_encoding/theory/definitions/encoding/``
plus the ``encoding.gamma`` dispatcher. The generated text only calls the
checked-in fixed emitters (``beta_theory_word``, ``beta_theory_fields3``,
``beta_theory_repeated_word``, and the template helpers in
``theory/encoding/``); every emitted word is a literal, so the result is
reviewable against ``encoding.py`` row by row.

Gamma requires active ``let`` binders to be unique, and the evaluator caps
nested expression lists at 255, so each function emits one grouped ``let``
whose binding rows write the record in order at constant syntax depth.
Binder names carry the function identity: ``lenNN`` for the record length,
``wNN_k`` signature words, ``cNN_k`` clause headers, ``rNN_k_j`` template
rows, ``bNN_k`` clause bodies, and ``emittedNN`` in the dispatcher.

Run from the repository root:

    python3 tests/gamma/beta-encoding-theory/emit_gamma.py
"""

import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.append(str(ROOT / "tests/gamma/derivation-layout"))
sys.path.append(str(ROOT / "tests/gamma/beta-encoding-theory"))

import encoding  # noqa: E402

OUT_DIR = (ROOT / "bootstrap/proofs/beta_encoding/theory/definitions/encoding")

GROUPS = (
    ("selectors", range(58, 69),
     "Ordering predicates, byte equality, result and admission selectors."),
    ("lists", range(69, 72),
     "ByteList reversal and append, Fragment flattening."),
    ("admission", range(72, 74),
     "Per-byte and whole-source admission folds."),
    ("hexadecimal", range(74, 80),
     "Word shift-in and the register/word hexadecimal token helpers."),
    ("automaton", range(80, 85),
     "The token automaton step, fold, end-projection, and classifiers."),
    ("emitting", range(85, 92),
     "Comment marking, checked successors, and counted byte/word emits."),
    ("dispatch", range(92, 104),
     "Status and expectation selectors, scanning, flush, and dispatch."),
    ("results", range(104, 108),
     "EOF finish projections and the error-valued encode entrypoints."),
)

DESCRIPTIONS = {
    58: "ordering_is_equal(Ordering): only Equal is True",
    59: "ordering_is_less(Ordering): only Less is True",
    60: "byte_equal(Byte, Byte): ordering_is_equal(byte_compare)",
    61: "is_nil(ByteList)",
    62: "choose(Bool, DState, DState)",
    63: "choose(Bool, ScanResult, ScanResult)",
    64: "choose(Ordering, on_equal, on_other) -> ScanResult",
    65: "choose(Ordering, on_greater, on_other) -> ScanResult",
    66: "choose(Ordering, on_greater, on_other) -> EncodeResult",
    67: "choose(Bool, Admission, Admission)",
    68: "adm_of_word(WordResult): WordValue admits, Overflow exhausts",
    69: "rev_onto(ByteList accumulator, ByteList): reverse onto",
    70: "append(ByteList, ByteList tail)",
    71: "flatten(Fragment, ByteList tail): source-order flattening",
    72: "admit_leaf(Byte, Admission): envelope byte plus checked count",
    73: "admit(Admission, Source): ordered envelope fold",
    74: "shift_in(Word, Nibble): value*16 + nibble modulo 2^64",
    75: "dr1_hex(HexResult): 'r' then digit opens DRd1",
    76: "dre_hex(HexResult): 're' then digit opens DReg2(0xe, n)",
    77: "drd1_hex(HexResult, Nibble): digit closes DReg2",
    78: "dzx_hex(HexResult): first 0x digit opens DZgo",
    79: "dzgo_hex(HexResult, Byte, Word, Byte): digit or ':' in 0x word",
    80: "dfa_step(DState, Byte): one token-automaton transition",
    81: "dfa_fold(DState, ByteList): run the automaton over the tail",
    82: "dfa_end(DState): completed-token class projection",
    83: "classify_first(Byte): first-byte state selection",
    84: "classify(ByteList): Nil is Empty, Cons runs the automaton",
    85: "state_comment_true(State): mark comment mode, keep fields",
    86: "sr_comment_true(ScanResult): mark comment mode on the state",
    87: "wr_succ(WordResult): chained checked successor",
    88: "emit_byte_count(WordResult, Byte, Expect, Bool, Expect, Word, "
        "Status, Word, Fragment)",
    89: "emit_byte(Byte, Expect next, State, Fragment)",
    90: "emit_word_count: counted emit of word_bytes(w)",
    91: "emit_word(Word, Expect next, State, Fragment)",
    92: "choose(Bool, Bool, Bool)",
    93: "status_ok(Status): only Ok continues",
    94: "expect_ready(Expect): only Ready publishes",
    95: "choose(Bool, EncodeResult, EncodeResult)",
    96: "enc_choose_status(Status, ok, invalid, exhausted)",
    97: "sr_state(ScanResult) -> State",
    98: "sr_fragment(ScanResult) -> Fragment",
    99: "sr_choose_expect(Expect, ScanResult x6)",
    100: "dispatch(Bool, Expect, Word, Status, Word, TokenClass)",
    101: "flush(State): emit a completed pending token",
    102: "scan_byte(State, Byte): one scanning transition",
    103: "scan(State, Source): byte-order traversal with fragments",
    104: "finish_state(State, Fragment): Ready+Ok publishes output",
    105: "finish(ScanResult): EOF flush then final projection",
    106: "encode_admitted(Admission, Source, source limit, output limit)",
    107: "encode(Source, source limit, output limit)",
}


def emit_words(seq, stem):
    """Binding rows emitting the signature word sequence in order."""
    rows = []
    i = 0
    index = 0
    while i + 3 <= len(seq):
        rows.append((f"{stem}_{index}",
                     f"(beta_theory_fields3 {seq[i]} {seq[i + 1]} "
                     f"{seq[i + 2]})", None))
        i += 3
        index += 1
    while i < len(seq):
        rows.append((f"{stem}_{index}", f"(beta_theory_word {seq[i]})",
                     None))
        i += 1
        index += 1
    return rows


def row_expression(row):
    """Gamma call emitting one template row, plus a decoding comment."""
    tag, symbol, children = row[0], row[1], row[2:]
    if tag == 0:
        return f"(beta_variable_template {symbol})", f"v{symbol}"
    if tag == 1:
        if not children:
            return (f"(beta_constant_template {symbol})", f"c{symbol}")
        helper = {1: "beta_unary_template", 2: "beta_template_ctor2",
                  6: "beta_template_ctor6",
                  8: "beta_template_ctor8"}[len(children)]
        args = " ".join(map(str, children))
        if helper == "beta_unary_template":
            return (f"({helper} 1 {symbol} {args})",
                    f"c{symbol}({args})")
        return (f"({helper} {symbol} {args})", f"c{symbol}({args})")
    helper = {1: "beta_unary_template", 2: "beta_binary_template",
              3: "beta_template_app3", 4: "beta_template_app4",
              6: "beta_template_app6", 7: "beta_template_app7",
              9: "beta_template_app9"}[len(children)]
    args = " ".join(map(str, children))
    if helper == "beta_unary_template":
        return (f"({helper} 2 {symbol} {args})", f"f{symbol}({args})")
    return (f"({helper} {symbol} {args})", f"f{symbol}({args})")


def function_source(identity, decl):
    """One grouped let emitting the whole function record in wire order."""
    result, arguments, mode, selected, clauses = decl
    signature = [result, len(arguments), *arguments, mode, selected,
                 len(clauses)]
    steps = []
    clause_words = 0
    for clause_index, (ctor, rows, body) in enumerate(clauses, 1):
        payload = sum(3 + len(r) - 1 if r[0] else 3
                      for r in rows) + 3
        clause_words += 1 + payload
        steps.append((f"c{identity}_{clause_index}",
                      f"(beta_theory_fields3 {payload} {ctor} {len(rows)})",
                      None))
        for row_index, row in enumerate(rows, 1):
            expr, decoded = row_expression(row)
            steps.append((f"r{identity}_{clause_index}_{row_index}", expr,
                          f"r{row_index}: {decoded}"))
        steps.append((f"b{identity}_{clause_index}",
                      f"(beta_theory_word {body})", None))
    total = len(signature) + clause_words
    real = struct.unpack("<I", encoding._emit_function(decl)[:4])[0]
    assert total == real, (identity, total, real)

    steps = ([(f"len{identity}", f"(beta_theory_word {total})", None)]
             + emit_words(signature, f"w{identity}") + steps)
    binding_lines = []
    for name, emitter, comment in steps:
        if comment:
            binding_lines.append(f"      ; {comment}")
        binding_lines.append(f"      ({name} Int {emitter})")
    lines = [f"; Function {identity}: {DESCRIPTIONS[identity]}",
             f"(def beta_encoding_fn{identity} () Int",
             "  (let ("] + binding_lines + ["      ) 0))"]
    return "\n".join(lines)


def main():
    decls = encoding.new_functions()
    by_identity = {58 + i: decl for i, decl in enumerate(decls)}
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for stem, identities, blurb in GROUPS:
        lines = [f"; Encoder definitions: {blurb}",
                 "; Generated by tests/gamma/beta-encoding-theory/"
                 "emit_gamma.py from the declarations in encoding.py.",
                 ""]
        for identity in identities:
            lines.append(function_source(identity, by_identity[identity]))
            lines.append("")
        path = OUT_DIR / f"{stem}.gamma"
        path.write_text("\n".join(lines))
        print(f"wrote {path.relative_to(ROOT)}")

    # Dispatcher: emit every record in exact wire order after function 57.
    rows = "\n".join(
        f"      (emitted{identity} Int (beta_encoding_fn{identity}))"
        for identity in range(58, 108))
    dispatcher = [
        "; Emit functions 58..107 in exact wire order; earlier identities",
        "; stay fixed and every new record follows function 57.",
        "(def beta_encoder_definitions () Int",
        "  (let (",
        rows,
        "      ) 0))",
        "",
    ]
    path = ROOT / "bootstrap/proofs/beta_encoding/theory/definitions/encoding.gamma"
    path.write_text("\n".join(dispatcher))
    print(f"wrote {path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()

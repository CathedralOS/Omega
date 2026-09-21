"""Independent reconstruction of the complete Beta encoder theory section.

These builders state the error-valued encoder definitions a second time,
against the literal grammar in bootstrap/1_beta/LANGUAGE.md and the paused
candidate architecture in ENCODER_CANDIDATE.md. No emitted Gamma text or
checker output is read here; identities are derived from the contract alone.

Every function is declared as plain data: ``(result, arguments, mode,
selected, clauses)`` where each clause is ``(constructor, rows, body)`` and
each row is ``(0, slot)``, ``(1, constructor, *children)``, or
``(2, function, *children)``. ``_emit_function`` serializes the
declarations into GTH1 records; the proof fixtures in encoder.py replay
those records' clause ordinals and template shapes through stepper.py.
"""

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent.parent / "derivation-layout"))
from wire import clause, function, record, theory


class Rows:
    """Append template rows and return their one-based identities."""

    def __init__(self):
        self.rows = []

    def var(self, slot):
        self.rows.append((0, slot))
        return len(self.rows)

    def capp(self, constructor, *children):
        self.rows.append((1, constructor) + tuple(children))
        return len(self.rows)

    def fapp(self, symbol, *children):
        self.rows.append((2, symbol) + tuple(children))
        return len(self.rows)

    def const(self, constructor):
        return self.capp(constructor)

    def byte(self, value):
        """Row for the nullary Byte constructor denoting this value."""
        row = (1, value + 1)
        if row in self.rows:
            return self.rows.index(row) + 1
        self.rows.append(row)
        return len(self.rows)


def _fn(result, arguments, clauses, mode=1, selected=0):
    return (result, tuple(arguments), mode, selected, tuple(clauses))


def _mode0(result, arguments, rows, body):
    return _fn(result, arguments, ((0, tuple(rows), body),), mode=0)


def _emit_row(row):
    if row[0] == 0:
        return record(0, row[1])
    return record(row[0], row[1], len(row) - 2, *row[2:])


def _emit_function(decl):
    result, arguments, mode, selected, clauses = decl
    emitted = [
        clause([_emit_row(row) for row in rows], constructor, body)
        for constructor, rows, body in clauses
    ]
    return function(arguments, emitted, mode=mode, selected=selected,
                    result=result)


# Existing vocabulary identities (bootstrap/proofs/beta_encoding/README.md).
BYTE, BOOL, NIBBLE, HEXRESULT, WORD, BYTELIST, WORDRESULT, ORDERING = range(1, 9)
FALSE_, TRUE_ = 257, 258
NO_HEX, HEX_ = 275, 276
WORD_C, NIL, CONS = 277, 278, 279
OVERFLOW, WORDVALUE = 280, 281
LESS, EQUAL, GREATER = 282, 283, 284

# Existing function identities called by the encoder definitions.
F_SOURCE_BYTE = 1
F_SEPARATOR = 2
F_COMMENT_END = 3
F_HEX_DIGIT = 4
F_JOIN_NIBBLES = 21
F_HIGH_NIBBLE = 22
F_LOW_NIBBLE = 23
F_WORD_BYTES = 24
F_BYTE_INCREMENT = 25
F_WORD_SUCCESSOR = 36
F_BYTE_COMPARE = 55
F_WORD_COMPARE = 57

# New sorts: Source=9, Expect=10, Status=11, State=12, Fragment=13,
# ScanResult=14, Admission=15, DState=16, TokenClass=17, EncodeResult=18.
SOURCE, EXPECT, STATUS, STATE, FRAGMENT, SCANRESULT = range(9, 15)
ADMISSION, DSTATE, TOKENCLASS, ENCODERESULT = range(15, 19)

S_EMPTY, S_LEAF, S_JOIN = 285, 286, 287
E_READY, E_R, E_X, E_RR, E_RX, E_RRX = range(288, 294)
ST_OK, ST_INVALID, ST_EXHAUSTED = 294, 295, 296
STATE_C = 297
F_EMPTY, F_CHUNK, F_JOIN = 298, 299, 300
SR_C = 301
A_REJECTED, A_EXHAUSTED, A_ADMITTED = 302, 303, 304

# Token automaton states in constructor order.
D_BAD = 305
D_DONE = 306          # DDone(TokenClass): a complete token
D_R1 = 307            # 'r'
D_RE = 308            # 're': register 0x0e on end, prefix of read/ret
D_REA = 309           # 'rea': register 0xea on end, 'd' completes read
D_RD1 = 310           # 'r' + one hex digit: DRd1(Nibble)
D_REG2 = 311          # 'r' + two hex digits: DReg2(Nibble, Nibble)
D_Z1 = 312            # '0'
D_ZX = 313            # '0x'
D_ZGO = 314           # '0x' + 1..16 digits: DZgo(Word, Byte count)
D_ZDONE = 315         # '0x' + digits + ':': DZdone(Word)
D_A1, D_A2 = 316, 317
D_C1, D_C2, D_C3 = 318, 319, 320
D_D1, D_DIV = 321, 322
D_H1, D_H2, D_H3 = 323, 324, 325
D_I1, D_I2 = 326, 327
D_J1, D_JMP, D_JNZ, D_JLT, D_JEQ = 328, 329, 330, 331, 332
D_L1, D_LO, D_LOA, D_LOD, D_LODB = 333, 334, 335, 336, 337
D_M1, D_MO, D_MU = 338, 339, 340
D_S1, D_SU, D_ST, D_STO, D_STR, D_STRE, D_STREB = range(341, 348)
D_W1, D_W2, D_W3, D_W4 = 348, 349, 350, 351

T_EMPTY, T_INVALID, T_DW, T_REGISTER, T_WORD, T_ASSERT, T_MNEMONIC = range(352,
                                                                         359)
R_INVALID, R_EXHAUSTED, R_SUCCESS = 359, 360, 361

EXPECT_CTORS = (E_READY, E_R, E_X, E_RR, E_RX, E_RRX)
STATUS_CTORS = (ST_OK, ST_INVALID, ST_EXHAUSTED)
DSTATE_CTORS = tuple(range(D_BAD, D_W4 + 1))

# Mnemonic spelling to (opcode byte value, next expectation constructor),
# transcribed from bootstrap/1_beta/LANGUAGE.md's 21-row table plus dw.
OPCODES = {b"halt": 0x00, b"imm": 0x01, b"mov": 0x02, b"add": 0x03,
           b"sub": 0x04, b"mul": 0x05, b"div": 0x06, b"mod": 0x07,
           b"loadb": 0x08, b"storeb": 0x09, b"load": 0x0A, b"store": 0x0B,
           b"jmp": 0x0C, b"jz": 0x0D, b"jnz": 0x0E, b"jlt": 0x0F,
           b"jeq": 0x10, b"read": 0x11, b"write": 0x12, b"call": 0x13,
           b"ret": 0x14}
EXPECT_AFTER = {b"halt": E_R, b"read": E_R, b"write": E_R, b"ret": E_READY,
                b"imm": E_RX, b"jz": E_RX, b"jnz": E_RX,
                b"jmp": E_X, b"call": E_X, b"mov": E_RR, b"add": E_RR,
                b"sub": E_RR, b"mul": E_RR, b"div": E_RR, b"mod": E_RR,
                b"loadb": E_RR, b"storeb": E_RR, b"load": E_RR,
                b"store": E_RR, b"jlt": E_RRX, b"jeq": E_RRX}

# Letter-prefix transitions for the keyword automaton.  Each entry maps a
# byte value to the next DState; unlisted bytes fail.  ("done", spelling)
# transitions to DDone(TMnemonic) or ("done", "dw") to DDone(TDw).
LETTER_STEPS = {
    D_A1: {ord("d"): D_A2},
    D_A2: {ord("d"): ("done", b"add")},
    D_C1: {ord("a"): D_C2},
    D_C2: {ord("l"): D_C3},
    D_C3: {ord("l"): ("done", b"call")},
    D_D1: {ord("i"): D_DIV, ord("w"): ("done", "dw")},
    D_DIV: {ord("v"): ("done", b"div")},
    D_H1: {ord("a"): D_H2},
    D_H2: {ord("l"): D_H3},
    D_H3: {ord("t"): ("done", b"halt")},
    D_I1: {ord("m"): D_I2},
    D_I2: {ord("m"): ("done", b"imm")},
    D_J1: {ord("m"): D_JMP, ord("n"): D_JNZ, ord("l"): D_JLT,
           ord("e"): D_JEQ, ord("z"): ("done", b"jz")},
    D_JMP: {ord("p"): ("done", b"jmp")},
    D_JNZ: {ord("z"): ("done", b"jnz")},
    D_JLT: {ord("t"): ("done", b"jlt")},
    D_JEQ: {ord("q"): ("done", b"jeq")},
    D_L1: {ord("o"): D_LO},
    D_LO: {ord("a"): D_LOA},
    D_LOA: {ord("d"): D_LOD},
    D_LOD: {ord("b"): D_LODB},
    D_LODB: {},
    D_M1: {ord("o"): D_MO, ord("u"): D_MU},
    D_MO: {ord("v"): ("done", b"mov"), ord("d"): ("done", b"mod")},
    D_MU: {ord("l"): ("done", b"mul")},
    D_S1: {ord("u"): D_SU, ord("t"): D_ST},
    D_SU: {ord("b"): ("done", b"sub")},
    D_ST: {ord("o"): D_STO},
    D_STO: {ord("r"): D_STR},
    D_STR: {ord("e"): D_STRE},
    D_STRE: {ord("b"): D_STREB},
    D_STREB: {},
    D_W1: {ord("r"): D_W2},
    D_W2: {ord("i"): D_W3},
    D_W3: {ord("t"): D_W4},
    D_W4: {ord("e"): ("done", b"write")},
}

# End-of-token classes for keyword states that are complete on end-of-token;
# every other state classifies Invalid.  DDone projects its TokenClass child.
END_MNEMONICS = {D_LOD: b"load", D_LODB: b"loadb",
                 D_STRE: b"store", D_STREB: b"storeb"}

# First-byte states: '0' starts a hexadecimal word, 'r' a register or the
# re*/ret/read family, and each listed letter starts its keyword prefix.
FIRST_STATES = ((ord("r"), D_R1), (ord("0"), D_Z1), (ord("a"), D_A1),
                (ord("c"), D_C1), (ord("d"), D_D1), (ord("h"), D_H1),
                (ord("i"), D_I1), (ord("j"), D_J1), (ord("l"), D_L1),
                (ord("m"), D_M1), (ord("s"), D_S1), (ord("w"), D_W1))


def new_constructors():
    """Constructor records for sorts 9..18, identities 285..361."""
    constructors = []
    constructors += [record(9, 0), record(9, 1, 1), record(9, 2, 9, 9)]
    constructors += [record(10, 0)] * 6
    constructors += [record(11, 0)] * 3
    constructors += [record(12, 6, 2, 6, 10, 5, 11, 5)]
    constructors += [record(13, 0), record(13, 1, 6), record(13, 2, 13, 13)]
    constructors += [record(14, 2, 12, 13)]
    constructors += [record(15, 0), record(15, 0), record(15, 1, 5)]
    constructors += [record(16, 0)]                                 # DBad
    constructors += [record(16, 1, 17)]                             # DDone
    constructors += [record(16, 0)] * 3                             # DR1 DRe DRea
    constructors += [record(16, 1, 3)]                              # DRd1
    constructors += [record(16, 2, 3, 3)]                           # DReg2
    constructors += [record(16, 0)] * 2                             # DZ1 DZx
    constructors += [record(16, 2, 5, 1)]                           # DZgo
    constructors += [record(16, 1, 5)]                              # DZdone
    constructors += [record(16, 0)] * (D_W4 - D_A1 + 1)             # DA1..DW4
    constructors += [record(17, 0)] * 3
    constructors += [record(17, 1, 1), record(17, 1, 5),
                     record(17, 1, 5), record(17, 2, 1, 10)]
    constructors += [record(18, 0), record(18, 0), record(18, 1, 6)]
    return constructors


def _ordering_predicate(match):
    clauses = [(order, ((1, 257 + int(order == match)),), 1)
               for order in (LESS, EQUAL, GREATER)]
    return _fn(BOOL, (ORDERING,), clauses)


def _choose_bool(result_sort):
    # (Bool, on_true, on_false): True selects argument one.
    clauses = [(FALSE_, ((0, 2),), 1), (TRUE_, ((0, 1),), 1)]
    return _fn(result_sort, (BOOL, result_sort, result_sort), clauses)


def _choose_order(result_sort, selected):
    # (Ordering, on_match, on_other): the matched ordering selects arg one.
    clauses = [(order, ((0, selected[order]),), 1)
               for order in (LESS, EQUAL, GREATER)]
    return _fn(result_sort, (ORDERING, result_sort, result_sort), clauses)


def _choose_expect(result_sort):
    # (Expect, on_Ready, on_R, on_X, on_RR, on_RX, on_RRX).
    clauses = [(ctor, ((0, position),), 1)
               for position, ctor in enumerate(EXPECT_CTORS, start=1)]
    return _fn(result_sort, (EXPECT,) + (result_sort,) * 6, clauses)


def _const(constructor):
    return (1, constructor)


def _const_clause(constructor, result_constructor):
    return (constructor, ((1, result_constructor),), 1)


def _mnemonic(r, spelling):
    opcode = r.byte(OPCODES[spelling])
    expect = r.const(EXPECT_AFTER[spelling])
    return r.capp(T_MNEMONIC, opcode, expect)


def _letter_chain(r, byte_row, steps):
    """Nest dstate_choose over equal-byte tests; the failure leaf is DBad."""
    node = r.const(D_BAD)
    for letter, target in reversed(list(steps.items())):
        if isinstance(target, tuple):
            if target[1] == "dw":
                inner = r.const(T_DW)
            else:
                inner = _mnemonic(r, target[1])
            target_row = r.capp(D_DONE, inner)
        else:
            target_row = r.const(target)
        cond = r.fapp(60, byte_row, r.byte(letter))
        node = r.fapp(62, cond, target_row, node)
    return node


def _zero_word(r):
    zero = r.byte(0)
    return r.capp(WORD_C, *([zero] * 8))


def _emit_count_body(r, chunk_fn):
    """Shared emit_*_count WORDVALUE body: capacity check, then emit."""
    emitted_value, nxt = r.var(1), r.var(2)
    comment, expect = r.var(3), r.var(4)
    count, status, limit = r.var(5), r.var(6), r.var(7)
    fragment = r.var(8)
    new_count = r.var(9)
    compared = r.fapp(F_WORD_COMPARE, new_count, limit)
    pending = r.const(NIL)
    failed = r.const(ST_EXHAUSTED)
    exhausted = r.capp(STATE_C, comment, pending, expect, count, failed,
                       limit)
    exhausted_result = r.capp(SR_C, exhausted, fragment)
    kept = r.capp(STATE_C, comment, pending, nxt, new_count, status, limit)
    grown = r.capp(F_JOIN, fragment, chunk_fn(r, emitted_value))
    kept_result = r.capp(SR_C, kept, grown)
    return r.fapp(65, compared, exhausted_result, kept_result)


def _emit_count_overflow(r):
    """Shared emit_*_count OVERFLOW body: sticky exhaustion, no growth."""
    comment, expect = r.var(3), r.var(4)
    count, limit = r.var(5), r.var(7)
    fragment = r.var(8)
    pending = r.const(NIL)
    failed = r.const(ST_EXHAUSTED)
    exhausted = r.capp(STATE_C, comment, pending, expect, count, failed,
                       limit)
    return r.capp(SR_C, exhausted, fragment)


def new_functions():
    """Function declarations 58..107 in exact identity order."""
    out = {}

    # 58 ordering_is_equal, 59 ordering_is_less: Ordering -> Bool.
    out[58] = _ordering_predicate(EQUAL)
    out[59] = _ordering_predicate(LESS)

    # 60 byte_equal(Byte, Byte) = ordering_is_equal(byte_compare(a, b)).
    r = Rows()
    left, right = r.var(0), r.var(1)
    compared = r.fapp(F_BYTE_COMPARE, left, right)
    body = r.fapp(58, compared)
    out[60] = _mode0(BOOL, (BYTE, BYTE), r.rows, body)

    # 61 is_nil(ByteList) -> Bool.
    out[61] = _fn(BOOL, (BYTELIST,),
                  (_const_clause(NIL, TRUE_), _const_clause(CONS, FALSE_)))

    # 62..67 result selection helpers: the condition is never evaluated.
    out[62] = _choose_bool(DSTATE)                                  # dstate
    out[63] = _choose_bool(SCANRESULT)                              # scan result
    out[64] = _choose_order(SCANRESULT, {LESS: 2, EQUAL: 1, GREATER: 2})
    out[65] = _choose_order(SCANRESULT, {LESS: 2, EQUAL: 2, GREATER: 1})
    out[66] = _choose_order(ENCODERESULT, {LESS: 2, EQUAL: 2, GREATER: 1})
    out[67] = _choose_bool(ADMISSION)                               # admission

    # 68 adm_of_word(WordResult) -> Admission: successor overflow exhausts.
    r = Rows()
    value = r.var(1)
    body = r.capp(A_ADMITTED, value)
    out[68] = _fn(ADMISSION, (WORDRESULT,),
                  (_const_clause(OVERFLOW, A_EXHAUSTED),
                   (WORDVALUE, tuple(r.rows), body)))

    # 69 rev_onto(accumulator, list): reverse the list onto the accumulator.
    r = Rows()
    acc = r.var(0)
    head, tail = r.var(2), r.var(3)
    pushed = r.capp(CONS, head, acc)
    body = r.fapp(69, pushed, tail)
    out[69] = _fn(BYTELIST, (BYTELIST, BYTELIST),
                  ((NIL, ((0, 0),), 1), (CONS, tuple(r.rows), body)),
                  selected=1)

    # 70 append(list, tail): copy the list ahead of the tail.
    r = Rows()
    tail = r.var(1)
    head, rest = r.var(2), r.var(3)
    appended = r.fapp(70, rest, tail)
    body = r.capp(CONS, head, appended)
    out[70] = _fn(BYTELIST, (BYTELIST, BYTELIST),
                  ((NIL, ((0, 1),), 1), (CONS, tuple(r.rows), body)))

    # 71 flatten(fragment, tail): right-first descent keeps source order.
    r = Rows()
    tail = r.var(1)
    chunk = r.var(2)
    body = r.fapp(70, chunk, tail)
    r2 = Rows()
    tail2 = r2.var(1)
    left, right = r2.var(2), r2.var(3)
    inner = r2.fapp(71, right, tail2)
    joined = r2.fapp(71, left, inner)
    out[71] = _fn(BYTELIST, (FRAGMENT, BYTELIST),
                  ((F_EMPTY, ((0, 1),), 1),
                   (F_CHUNK, tuple(r.rows), body),
                   (F_JOIN, tuple(r2.rows), joined)))

    # 72 admit_leaf(Byte, Admission): envelope byte plus checked count.
    r = Rows()
    byte = r.var(0)
    count = r.var(2)
    admitted_byte = r.fapp(F_SOURCE_BYTE, byte)
    successor = r.fapp(F_WORD_SUCCESSOR, count)
    counted = r.fapp(68, successor)
    rejected = r.const(A_REJECTED)
    body = r.fapp(67, admitted_byte, counted, rejected)
    out[72] = _fn(ADMISSION, (BYTE, ADMISSION),
                  ((A_REJECTED, ((1, A_REJECTED),), 1),
                   (A_EXHAUSTED, ((1, A_EXHAUSTED),), 1),
                   (A_ADMITTED, tuple(r.rows), body)),
                  selected=1)

    # 73 admit(Admission, Source): ordered fold over the raw-byte tree.
    r = Rows()
    acc = r.var(0)
    leaf_byte = r.var(2)
    leaf = r.fapp(72, leaf_byte, acc)
    r2 = Rows()
    acc2 = r2.var(0)
    l_left, l_right = r2.var(2), r2.var(3)
    first = r2.fapp(73, acc2, l_left)
    joined = r2.fapp(73, first, l_right)
    out[73] = _fn(ADMISSION, (ADMISSION, SOURCE),
                  ((S_EMPTY, ((0, 0),), 1),
                   (S_LEAF, tuple(r.rows), leaf),
                   (S_JOIN, tuple(r2.rows), joined)),
                  selected=1)

    # 74 shift_in(Word, Nibble): value*16 + nibble, dropping bit 64+.
    r = Rows()
    digits = [r.var(slot) for slot in range(2, 10)]
    nibble = r.var(1)
    lows = [r.fapp(F_LOW_NIBBLE, row) for row in digits]
    highs = [r.fapp(F_HIGH_NIBBLE, row) for row in digits[:7]]
    joins = [r.fapp(F_JOIN_NIBBLES, lows[0], nibble)]
    joins += [r.fapp(F_JOIN_NIBBLES, lows[i], highs[i - 1])
              for i in range(1, 8)]
    body = r.capp(WORD_C, *joins)
    out[74] = _fn(WORD, (WORD, NIBBLE), ((WORD_C, tuple(r.rows), body),))

    # 75 dr1_hex: 'r' then digit n -> DRd1(n).
    r = Rows()
    body = r.capp(D_RD1, r.var(1))
    out[75] = _fn(DSTATE, (HEXRESULT,),
                  (_const_clause(NO_HEX, D_BAD), (HEX_, tuple(r.rows), body)))
    # 76 dre_hex: 're' then digit n -> DReg2(0xe, n).
    r = Rows()
    first = r.const(259 + 14)
    body = r.capp(D_REG2, first, r.var(1))
    out[76] = _fn(DSTATE, (HEXRESULT,),
                  (_const_clause(NO_HEX, D_BAD), (HEX_, tuple(r.rows), body)))
    # 77 drd1_hex(HexResult, Nibble): digit n2 -> DReg2(n1, n2).
    r = Rows()
    first = r.var(1)
    body = r.capp(D_REG2, first, r.var(2))
    out[77] = _fn(DSTATE, (HEXRESULT, NIBBLE),
                  (_const_clause(NO_HEX, D_BAD), (HEX_, tuple(r.rows), body)))
    # 78 dzx_hex: first digit after '0x' opens DZgo(0 shifted in, count 1).
    r = Rows()
    shifted = r.fapp(74, _zero_word(r), r.var(1))
    body = r.capp(D_ZGO, shifted, r.byte(1))
    out[78] = _fn(DSTATE, (HEXRESULT,),
                  (_const_clause(NO_HEX, D_BAD), (HEX_, tuple(r.rows), body)))
    # 79 dzgo_hex(HexResult, scanned Byte, Word, Byte count): a digit while
    # count < 16 shifts in; ':' closes for the assertion state; else Bad.
    r = Rows()
    scanned, word, count = r.var(1), r.var(2), r.var(3)
    colon = r.fapp(60, scanned, r.byte(58))
    done = r.capp(D_ZDONE, word)
    bad = r.const(D_BAD)
    no_hex = r.fapp(62, colon, done, bad)
    r2 = Rows()
    word2, count2, nib2 = r2.var(2), r2.var(3), r2.var(4)
    below = r2.fapp(F_BYTE_COMPARE, count2, r2.byte(16))
    is_less = r2.fapp(59, below)
    shifted2 = r2.fapp(74, word2, nib2)
    bumped = r2.fapp(F_BYTE_INCREMENT, count2)
    again = r2.capp(D_ZGO, shifted2, bumped)
    hex_ = r2.fapp(62, is_less, again, r2.const(D_BAD))
    out[79] = _fn(DSTATE, (HEXRESULT, BYTE, WORD, BYTE),
                  ((NO_HEX, tuple(r.rows), no_hex),
                   (HEX_, tuple(r2.rows), hex_)))

    # 80 dfa_step(DState, Byte): one token-automaton transition.
    dfa_clauses = []
    for state in DSTATE_CTORS:
        r = Rows()
        byte = r.var(1)
        if state in (D_BAD, D_DONE, D_REG2, D_ZDONE, D_LODB, D_STREB):
            body = r.const(D_BAD)
        elif state == D_R1:
            re_state = r.const(D_RE)
            digits = r.fapp(F_HEX_DIGIT, byte)
            register = r.fapp(75, digits)
            is_e = r.fapp(60, byte, r.byte(ord("e")))
            body = r.fapp(62, is_e, re_state, register)
        elif state == D_RE:
            rea = r.const(D_REA)
            ret = r.capp(D_DONE, _mnemonic(r, b"ret"))
            digits = r.fapp(F_HEX_DIGIT, byte)
            register = r.fapp(76, digits)
            is_t = r.fapp(60, byte, r.byte(ord("t")))
            inner = r.fapp(62, is_t, ret, register)
            is_a = r.fapp(60, byte, r.byte(ord("a")))
            body = r.fapp(62, is_a, rea, inner)
        elif state == D_REA:
            read = r.capp(D_DONE, _mnemonic(r, b"read"))
            is_d = r.fapp(60, byte, r.byte(ord("d")))
            bad = r.const(D_BAD)
            body = r.fapp(62, is_d, read, bad)
        elif state == D_RD1:
            first = r.var(2)
            digits = r.fapp(F_HEX_DIGIT, byte)
            body = r.fapp(77, digits, first)
        elif state == D_Z1:
            zx = r.const(D_ZX)
            is_x = r.fapp(60, byte, r.byte(ord("x")))
            bad = r.const(D_BAD)
            body = r.fapp(62, is_x, zx, bad)
        elif state == D_ZX:
            digits = r.fapp(F_HEX_DIGIT, byte)
            body = r.fapp(78, digits)
        elif state == D_ZGO:
            word = r.var(2)
            count = r.var(3)
            digits = r.fapp(F_HEX_DIGIT, byte)
            body = r.fapp(79, digits, byte, word, count)
        else:
            body = _letter_chain(r, byte, LETTER_STEPS[state])
        dfa_clauses.append((state, tuple(r.rows), body))
    out[80] = _fn(DSTATE, (DSTATE, BYTE), dfa_clauses)

    # 81 dfa_fold(DState, ByteList): run the automaton over the token tail.
    r = Rows()
    state = r.var(0)
    head, tail = r.var(2), r.var(3)
    stepped = r.fapp(80, state, head)
    body = r.fapp(81, stepped, tail)
    out[81] = _fn(DSTATE, (DSTATE, BYTELIST),
                  ((NIL, ((0, 0),), 1), (CONS, tuple(r.rows), body)),
                  selected=1)

    # 82 dfa_end(DState): project the completed-token class.
    end_clauses = []
    for state in DSTATE_CTORS:
        r = Rows()
        if state == D_DONE:
            body = r.var(1)
        elif state == D_RE:
            body = r.capp(T_REGISTER, r.byte(0x0E))
        elif state == D_REA:
            body = r.capp(T_REGISTER, r.byte(0xEA))
        elif state == D_RD1:
            joined = r.fapp(F_JOIN_NIBBLES, r.const(259), r.var(1))
            body = r.capp(T_REGISTER, joined)
        elif state == D_REG2:
            joined = r.fapp(F_JOIN_NIBBLES, r.var(1), r.var(2))
            body = r.capp(T_REGISTER, joined)
        elif state == D_ZGO:
            body = r.capp(T_WORD, r.var(1))
        elif state == D_ZDONE:
            body = r.capp(T_ASSERT, r.var(1))
        elif state in END_MNEMONICS:
            body = _mnemonic(r, END_MNEMONICS[state])
        else:
            body = r.const(T_INVALID)
        end_clauses.append((state, tuple(r.rows), body))
    out[82] = _fn(TOKENCLASS, (DSTATE,), end_clauses)

    # 83 classify_first(Byte): the first token byte selects the state.
    r = Rows()
    byte = r.var(0)
    body = r.const(D_BAD)
    for letter, state in reversed(FIRST_STATES):
        target = r.const(state)
        cond = r.fapp(60, byte, r.byte(letter))
        body = r.fapp(62, cond, target, body)
    out[83] = _mode0(DSTATE, (BYTE,), r.rows, body)

    # 84 classify(ByteList): Nil is Empty; Cons runs the automaton.
    r = Rows()
    head, tail = r.var(1), r.var(2)
    started = r.fapp(83, head)
    folded = r.fapp(81, started, tail)
    body = r.fapp(82, folded)
    out[84] = _fn(TOKENCLASS, (BYTELIST,),
                  (_const_clause(NIL, T_EMPTY), (CONS, tuple(r.rows), body)))

    # 85 state_comment_true(State): mark comment mode, keep other fields.
    r = Rows()
    fields = [r.var(slot) for slot in range(1, 7)]
    body = r.capp(STATE_C, r.const(TRUE_), fields[1], fields[2], fields[3],
                  fields[4], fields[5])
    out[85] = _fn(STATE, (STATE,), ((STATE_C, tuple(r.rows), body),))

    # 86 sr_comment_true(ScanResult): mark comment mode on the state half.
    r = Rows()
    state, fragment = r.var(1), r.var(2)
    commented = r.fapp(85, state)
    body = r.capp(SR_C, commented, fragment)
    out[86] = _fn(SCANRESULT, (SCANRESULT,), ((SR_C, tuple(r.rows), body),))

    # 87 wr_succ(WordResult): chained checked successor.
    r = Rows()
    body = r.fapp(F_WORD_SUCCESSOR, r.var(1))
    out[87] = _fn(WORDRESULT, (WORDRESULT,),
                  (_const_clause(OVERFLOW, OVERFLOW),
                   (WORDVALUE, tuple(r.rows), body)))

    # 88 emit_byte_count(WordResult succ, Byte, Expect next, Bool comment,
    # Expect old, Word count, Status, Word limit, Fragment).
    r = Rows()
    over = _emit_count_overflow(r)
    r2 = Rows()
    kept = _emit_count_body(
        r2, lambda rr, value: rr.capp(
            F_CHUNK, rr.capp(CONS, value, rr.const(NIL))))
    out[88] = _fn(SCANRESULT,
                  (WORDRESULT, BYTE, EXPECT, BOOL, EXPECT, WORD, STATUS,
                   WORD, FRAGMENT),
                  ((OVERFLOW, tuple(r.rows), over),
                   (WORDVALUE, tuple(r2.rows), kept)))

    # 89 emit_byte(Byte, Expect next, State, Fragment).
    r = Rows()
    byte, nxt, fragment = r.var(0), r.var(1), r.var(3)
    fields = [r.var(slot) for slot in range(4, 10)]
    successor = r.fapp(F_WORD_SUCCESSOR, fields[3])
    body = r.fapp(88, successor, byte, nxt, fields[0], fields[2], fields[3],
                  fields[4], fields[5], fragment)
    out[89] = _fn(SCANRESULT, (BYTE, EXPECT, STATE, FRAGMENT),
                  ((STATE_C, tuple(r.rows), body),), selected=2)

    # 90 emit_word_count: same shape, emitting word_bytes(w).
    r = Rows()
    over = _emit_count_overflow(r)
    r2 = Rows()
    kept = _emit_count_body(
        r2, lambda rr, value: rr.capp(F_CHUNK, rr.fapp(F_WORD_BYTES, value)))
    out[90] = _fn(SCANRESULT,
                  (WORDRESULT, WORD, EXPECT, BOOL, EXPECT, WORD, STATUS,
                   WORD, FRAGMENT),
                  ((OVERFLOW, tuple(r.rows), over),
                   (WORDVALUE, tuple(r2.rows), kept)))

    # 91 emit_word(Word, Expect next, State, Fragment): eight checked
    # successors ahead of the single capacity comparison.
    r = Rows()
    word, nxt, fragment = r.var(0), r.var(1), r.var(3)
    fields = [r.var(slot) for slot in range(4, 10)]
    chain = r.fapp(F_WORD_SUCCESSOR, fields[3])
    for _ in range(7):
        chain = r.fapp(87, chain)
    body = r.fapp(90, chain, word, nxt, fields[0], fields[2], fields[3],
                  fields[4], fields[5], fragment)
    out[91] = _fn(SCANRESULT, (WORD, EXPECT, STATE, FRAGMENT),
                  ((STATE_C, tuple(r.rows), body),), selected=2)

    # 92 bool_choose(Bool, on_true, on_false) -> Bool.
    out[92] = _choose_bool(BOOL)
    # 93 status_ok(Status) -> Bool.
    out[93] = _fn(BOOL, (STATUS,),
                  (_const_clause(ST_OK, TRUE_),
                   _const_clause(ST_INVALID, FALSE_),
                   _const_clause(ST_EXHAUSTED, FALSE_)))
    # 94 expect_ready(Expect) -> Bool.
    out[94] = _fn(BOOL, (EXPECT,),
                  tuple(_const_clause(ctor, 257 + int(ctor == E_READY))
                        for ctor in EXPECT_CTORS))
    # 95 enc_choose(Bool, EncodeResult, EncodeResult) -> EncodeResult.
    out[95] = _choose_bool(ENCODERESULT)
    # 96 enc_choose_status(Status, ok, invalid, exhausted) -> EncodeResult.
    out[96] = _fn(ENCODERESULT,
                  (STATUS, ENCODERESULT, ENCODERESULT, ENCODERESULT),
                  ((ST_OK, ((0, 1),), 1), (ST_INVALID, ((0, 2),), 1),
                    (ST_EXHAUSTED, ((0, 3),), 1)))

    # 97 sr_state, 99 sr_fragment: ScanResult accessors.
    out[97] = _fn(STATE, (SCANRESULT,), ((SR_C, ((0, 1),), 1),))
    out[98] = _fn(FRAGMENT, (SCANRESULT,), ((SR_C, ((0, 2),), 1),))

    # 99 sr_choose_expect(Expect, SR x6): dispatch's expectation dispatch.
    out[99] = _choose_expect(SCANRESULT)

    # 100 dispatch(comment, expect, count, status, limit, TokenClass):
    # emit or reject the classified token. Declared before flush/scan_byte so
    # every function dependency points backward.
    out[100] = _dispatch()

    # 101 flush(State): emit a completed pending token, or nothing.
    r = Rows()
    comment, pending, expect = r.var(1), r.var(2), r.var(3)
    count, status, limit = r.var(4), r.var(5), r.var(6)
    empty = r.const(F_EMPTY)
    same = r.capp(STATE_C, comment, pending, expect, count, status, limit)
    nothing = r.capp(SR_C, same, empty)
    reversed_ = r.fapp(69, r.const(NIL), pending)
    classified = r.fapp(84, reversed_)
    dispatched = r.fapp(100, comment, expect, count, status, limit,
                      classified)
    is_empty = r.fapp(61, pending)
    body = r.fapp(63, is_empty, nothing, dispatched)
    out[101] = _fn(SCANRESULT, (STATE,), ((STATE_C, tuple(r.rows), body),))

    # 102 scan_byte(State, Byte): one scanning transition.
    r = Rows()
    byte = r.var(1)
    comment, pending, expect = r.var(2), r.var(3), r.var(4)
    count, status, limit = r.var(5), r.var(6), r.var(7)
    same = r.capp(STATE_C, comment, pending, expect, count, status, limit)
    empty = r.const(F_EMPTY)
    sticky = r.capp(SR_C, same, empty)
    ok = r.fapp(93, status)
    ended = r.fapp(F_COMMENT_END, byte)
    stays = r.fapp(92, ended, r.const(FALSE_), r.const(TRUE_))
    commented = r.capp(STATE_C, stays, pending, expect, count, status, limit)
    in_comment = r.capp(SR_C, commented, empty)
    pushed = r.capp(CONS, byte, pending)
    advanced = r.capp(STATE_C, r.const(FALSE_), pushed, expect, count,
                      status, limit)
    pushing = r.capp(SR_C, advanced, empty)
    separated = r.fapp(F_SEPARATOR, byte)
    flushed = r.fapp(101, same)
    flushed_or_pushed = r.fapp(63, separated, flushed, pushing)
    semicolon = r.fapp(60, byte, r.byte(ord(";")))
    flushed_commented = r.fapp(86, flushed)
    in_token = r.fapp(63, semicolon, flushed_commented, flushed_or_pushed)
    proceed = r.fapp(63, comment, in_comment, in_token)
    body = r.fapp(63, ok, proceed, sticky)
    out[102] = _fn(SCANRESULT, (STATE, BYTE),
                   ((STATE_C, tuple(r.rows), body),))

    # 103 scan(State, Source): byte-order traversal threading one state and
    # building ordered fragments.
    r = Rows()
    state0 = r.var(0)
    empty0 = r.const(F_EMPTY)
    nothing = r.capp(SR_C, state0, empty0)
    r2 = Rows()
    state = r2.var(0)
    leaf_byte = r2.var(2)
    leaf = r2.fapp(102, state, leaf_byte)
    r3 = Rows()
    state3 = r3.var(0)
    l_left, l_right = r3.var(2), r3.var(3)
    first = r3.fapp(103, state3, l_left)
    first_state = r3.fapp(97, first)
    first_fragment = r3.fapp(98, first)
    second = r3.fapp(103, first_state, l_right)
    second_state = r3.fapp(97, second)
    second_fragment = r3.fapp(98, second)
    combined = r3.capp(F_JOIN, first_fragment, second_fragment)
    joined = r3.capp(SR_C, second_state, combined)
    out[103] = _fn(SCANRESULT, (STATE, SOURCE),
                   ((S_EMPTY, tuple(r.rows), nothing),
                    (S_LEAF, tuple(r2.rows), leaf),
                    (S_JOIN, tuple(r3.rows), joined)),
                   selected=1)

    # 104 finish_state(State, Fragment): Ready plus Ok publishes the
    # flattened output; Invalid and Exhausted keep their results.
    r = Rows()
    fragment = r.var(1)
    expect, status = r.var(4), r.var(6)
    ready = r.fapp(94, expect)
    flattened = r.fapp(71, fragment, r.const(NIL))
    success = r.capp(R_SUCCESS, flattened)
    rejected = r.const(R_INVALID)
    exhausted = r.const(R_EXHAUSTED)
    decided = r.fapp(95, ready, success, rejected)
    body = r.fapp(96, status, decided, rejected, exhausted)
    out[104] = _fn(ENCODERESULT, (STATE, FRAGMENT),
                   ((STATE_C, tuple(r.rows), body),))

    # 105 finish(ScanResult): one EOF flush, then the final projection.
    r = Rows()
    state, fragment = r.var(1), r.var(2)
    flushed = r.fapp(101, state)
    flushed_state = r.fapp(97, flushed)
    flushed_fragment = r.fapp(98, flushed)
    combined = r.capp(F_JOIN, fragment, flushed_fragment)
    body = r.fapp(104, flushed_state, combined)
    out[105] = _fn(ENCODERESULT, (SCANRESULT,), ((SR_C, tuple(r.rows), body),))

    # 106 encode_admitted(Admission, Source, source limit, output limit).
    r2 = Rows()
    source = r2.var(1)
    source_limit, output_limit = r2.var(2), r2.var(3)
    count = r2.var(4)
    compared = r2.fapp(F_WORD_COMPARE, count, source_limit)
    over = r2.const(R_EXHAUSTED)
    initial = r2.capp(STATE_C, r2.const(FALSE_), r2.const(NIL),
                    r2.const(E_READY), _zero_word(r2), r2.const(ST_OK),
                    output_limit)
    scanned = r2.fapp(103, initial, source)
    finished = r2.fapp(105, scanned)
    body = r2.fapp(66, compared, over, finished)
    out[106] = _fn(ENCODERESULT, (ADMISSION, SOURCE, WORD, WORD),
                   ((A_REJECTED, ((1, R_INVALID),), 1),
                    (A_EXHAUSTED, ((1, R_EXHAUSTED),), 1),
                    (A_ADMITTED, tuple(r2.rows), body)))

    # 107 encode(Source, source limit, output limit): whole-source envelope
    # admission, then scanning and EOF projection.
    r = Rows()
    source, source_limit, output_limit = r.var(0), r.var(1), r.var(2)
    admitted = r.fapp(73, r.capp(A_ADMITTED, _zero_word(r)), source)
    body = r.fapp(106, admitted, source, source_limit, output_limit)
    out[107] = _mode0(ENCODERESULT, (SOURCE, WORD, WORD), r.rows, body)

    return [out[identity] for identity in range(58, 108)]


def _dispatch():
    """dispatch(comment, expect, count, status, limit, TokenClass)."""
    clauses = []

    def common(r):
        comment, expect, count = r.var(0), r.var(1), r.var(2)
        status, limit = r.var(3), r.var(4)
        empty = r.const(F_EMPTY)
        cleared = r.capp(STATE_C, comment, r.const(NIL), expect, count,
                         status, limit)
        failed = r.capp(STATE_C, comment, r.const(NIL), expect, count,
                        r.const(ST_INVALID), limit)
        invalid = r.capp(SR_C, failed, empty)
        return comment, expect, count, status, limit, empty, cleared, invalid

    # T_EMPTY and T_INVALID are malformed tokens.
    r = Rows()
    _, _, _, _, _, _, _, invalid = common(r)
    clauses.append((T_EMPTY, tuple(r.rows), invalid))
    r = Rows()
    _, _, _, _, _, _, _, invalid = common(r)
    clauses.append((T_INVALID, tuple(r.rows), invalid))

    # T_DW is legal only while Ready: it selects expectation X, no bytes.
    r = Rows()
    comment, expect, count, status, limit, empty, cleared, invalid = common(r)
    awaiting = r.capp(STATE_C, comment, r.const(NIL), r.const(E_X), count,
                      status, limit)
    dw_ok = r.capp(SR_C, awaiting, empty)
    body = r.fapp(99, expect, dw_ok, invalid, invalid, invalid, invalid,
                  invalid)
    clauses.append((T_DW, tuple(r.rows), body))

    # T_REGISTER(Byte) emits one byte at the operand expectations.
    r = Rows()
    comment, expect, count, status, limit, empty, cleared, invalid = common(r)
    reg = r.var(6)
    reg_ready = r.fapp(89, reg, r.const(E_READY), cleared, empty)
    reg_r = r.fapp(89, reg, r.const(E_R), cleared, empty)
    reg_x = r.fapp(89, reg, r.const(E_X), cleared, empty)
    reg_rx = r.fapp(89, reg, r.const(E_RX), cleared, empty)
    body = r.fapp(99, expect, invalid, reg_ready, invalid, reg_r, reg_x,
                  reg_rx)
    clauses.append((T_REGISTER, tuple(r.rows), body))

    # T_WORD(Word) emits eight bytes only at expectation X.
    r = Rows()
    comment, expect, count, status, limit, empty, cleared, invalid = common(r)
    word = r.var(6)
    word_ok = r.fapp(91, word, r.const(E_READY), cleared, empty)
    body = r.fapp(99, expect, invalid, invalid, word_ok, invalid, invalid,
                  invalid)
    clauses.append((T_WORD, tuple(r.rows), body))

    # T_ASSERT(Word) requires the word to equal the count, only in Ready.
    r = Rows()
    comment, expect, count, status, limit, empty, cleared, invalid = common(r)
    address = r.var(6)
    kept = r.capp(SR_C, cleared, empty)
    matched = r.fapp(F_WORD_COMPARE, address, count)
    checked = r.fapp(64, matched, kept, invalid)
    body = r.fapp(99, expect, checked, invalid, invalid, invalid, invalid,
                  invalid)
    clauses.append((T_ASSERT, tuple(r.rows), body))

    # T_MNEMONIC(Byte opcode, Expect next) emits the opcode in Ready.
    r = Rows()
    comment, expect, count, status, limit, empty, cleared, invalid = common(r)
    opcode, following = r.var(6), r.var(7)
    emitted = r.fapp(89, opcode, following, cleared, empty)
    body = r.fapp(99, expect, emitted, invalid, invalid, invalid, invalid,
                  invalid)
    clauses.append((T_MNEMONIC, tuple(r.rows), body))

    return _fn(SCANRESULT, (BOOL, EXPECT, WORD, STATUS, WORD, TOKENCLASS),
               clauses, selected=5)


def section_records(partial_constructors, partial_functions):
    """Complete GTH1 records over the restated partial theory."""
    constructors = list(partial_constructors) + new_constructors()
    functions = list(partial_functions) + [
        _emit_function(decl) for decl in new_functions()
    ]
    return constructors, functions


def section_bytes(partial_constructors, partial_functions):
    constructors, functions = section_records(partial_constructors,
                                              partial_functions)
    return theory(constructors, functions, sorts=18)

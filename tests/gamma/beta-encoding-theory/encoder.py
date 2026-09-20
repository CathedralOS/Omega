"""Authored encoder equations over functions 58..107.

Every equation is produced by stepping the parsed emitted theory, not by
reading checker output or a Beta assembler.  The stated right side is a
literal constructor term transcribed from LANGUAGE.md; ``prove`` computes
the value independently and the request is only well-formed when the two
agree.  Intermediate terms land in the certificate witness table; the owner
table carries only the closure of the stated root.
"""

import struct
import sys

sys.setrecursionlimit(1000000)

from literal_rows import LiteralRows
from lexical import certificate, checked, envelope, proposition, record, rejected
from stepper import Stepper, Theory


# Vocabulary identities shared with encoding.py's independent restatement.
FALSE, TRUE = 257, 258
NO_HEX, HEX = 275, 276
WORD_C, NIL, CONS = 277, 278, 279
OVERFLOW, WORDVALUE = 280, 281
LESS, EQUAL, GREATER = 282, 283, 284
S_EMPTY, S_LEAF, S_JOIN = 285, 286, 287
E_READY, E_R, E_X, E_RR, E_RX, E_RRX = range(288, 294)
ST_OK, ST_INVALID, ST_EXHAUSTED = 294, 295, 296
STATE_C = 297
F_EMPTY, F_CHUNK, F_JOIN = 298, 299, 300
SR_C = 301
A_REJECTED, A_EXHAUSTED, A_ADMITTED = 302, 303, 304
(D_BAD, D_DONE, D_R1, D_RE, D_REA, D_RD1, D_REG2, D_Z1, D_ZX, D_ZGO,
 D_ZDONE, D_A1, D_A2, D_C1, D_C2, D_C3, D_D1, D_DIV, D_H1, D_H2, D_H3,
 D_I1, D_I2, D_J1, D_JMP, D_JNZ, D_JLT, D_JEQ, D_L1, D_LO, D_LOA, D_LOD,
 D_LODB, D_M1, D_MO, D_MU, D_S1, D_SU, D_ST, D_STO, D_STR, D_STRE,
 D_STREB, D_W1, D_W2, D_W3, D_W4) = range(305, 352)
(T_EMPTY, T_INVALID, T_DW, T_REGISTER, T_WORD, T_ASSERT,
 T_MNEMONIC) = range(352, 359)
R_INVALID, R_EXHAUSTED, R_SUCCESS = 359, 360, 361


def C(s, constructor, *children):
    return s.intern(1, constructor, tuple(children))


def F(s, function, *arguments):
    return s.intern(2, function, tuple(arguments))


def nib(s, value):
    return s.intern(1, 259 + value)


def state(s, comment=False, pending=None, expect=E_READY, count=0,
          status=ST_OK, limit=0):
    return C(s, STATE_C, s.intern(1, TRUE if comment else FALSE),
             pending if pending is not None else s.intern(1, NIL),
             s.intern(1, expect), s.word(count), s.intern(1, status),
             s.word(limit))


def sr(s, state_node, fragment):
    return C(s, SR_C, state_node, fragment)


def mnemonic(s, opcode, expect):
    return C(s, T_MNEMONIC, s.byte(opcode), s.intern(1, expect))


class Vector:
    """One batched request: ordered (left, literal right) equation pairs."""

    def __init__(self, theory):
        self.s = Stepper(theory)
        self.equations = []

    def equation(self, left, right):
        self.equations.append((left, right))
        return left

    def build(self, definitions):
        for index, (left, right) in enumerate(self.equations):
            value, _ = self.s.prove(left)
            if value != right:
                raise ValueError(f"equation {index} proved a different value")
        owner_terms, left_ref, right_ref, witnesses, proofs = \
            self.s.encode(*self.equations[-1])
        owner = proposition([record(*t) for t in owner_terms],
                            left_ref, right_ref)
        witness_records = [record(*t) for t in witnesses]
        proof_records = [record(*p) for p in proofs]
        cert = certificate(terms=witness_records, proofs=proof_records)
        return (envelope((definitions, owner, cert)), len(proofs),
                owner, witness_records, proof_records)


def _predicates(v):
    s = v.s
    v.equation(F(s, 58, C(s, EQUAL)), C(s, TRUE))
    v.equation(F(s, 58, C(s, LESS)), C(s, FALSE))
    v.equation(F(s, 59, C(s, LESS)), C(s, TRUE))
    v.equation(F(s, 59, C(s, GREATER)), C(s, FALSE))
    v.equation(F(s, 60, s.byte(0x61), s.byte(0x61)), C(s, TRUE))
    v.equation(F(s, 60, s.byte(0x61), s.byte(0x62)), C(s, FALSE))


def _choosers(v):
    s = v.s
    t_dw, t_word = C(s, T_DW), C(s, T_WORD, s.word(0))
    v.equation(F(s, 62, C(s, TRUE), C(s, D_R1), C(s, D_BAD)), C(s, D_R1))
    v.equation(F(s, 92, C(s, FALSE), C(s, TRUE), C(s, FALSE)),
               C(s, FALSE))
    v.equation(F(s, 95, C(s, TRUE), C(s, R_INVALID), C(s, R_EXHAUSTED)),
               C(s, R_INVALID))
    v.equation(F(s, 96, C(s, ST_OK), C(s, R_INVALID), C(s, R_EXHAUSTED),
                 C(s, R_SUCCESS, s.intern(1, NIL))), C(s, R_INVALID))
    v.equation(F(s, 96, C(s, ST_INVALID), C(s, R_INVALID),
                 C(s, R_EXHAUSTED), C(s, R_SUCCESS, s.intern(1, NIL))),
               C(s, R_EXHAUSTED))
    v.equation(F(s, 96, C(s, ST_EXHAUSTED), C(s, R_INVALID),
                 C(s, R_EXHAUSTED), C(s, R_SUCCESS, s.intern(1, NIL))),
               C(s, R_SUCCESS, s.intern(1, NIL)))
    a, b = sr(s, state(s), C(s, F_EMPTY)), sr(s, state(s, comment=True),
                                            C(s, F_EMPTY))
    v.equation(F(s, 63, C(s, TRUE), a, b), a)
    v.equation(F(s, 63, C(s, FALSE), a, b), b)
    v.equation(F(s, 64, C(s, EQUAL), a, b), a)
    v.equation(F(s, 64, C(s, LESS), a, b), b)
    v.equation(F(s, 65, C(s, GREATER), a, b), a)
    v.equation(F(s, 65, C(s, EQUAL), a, b), b)
    v.equation(F(s, 66, C(s, GREATER), C(s, R_EXHAUSTED),
                 C(s, R_SUCCESS, s.intern(1, NIL))), C(s, R_EXHAUSTED))
    v.equation(F(s, 66, C(s, LESS), C(s, R_EXHAUSTED),
                 C(s, R_SUCCESS, s.intern(1, NIL))),
               C(s, R_SUCCESS, s.intern(1, NIL)))
    admitted, refused = C(s, A_ADMITTED, s.word(7)), C(s, A_REJECTED)
    v.equation(F(s, 67, C(s, TRUE), admitted, refused), admitted)
    v.equation(F(s, 67, C(s, FALSE), admitted, refused), refused)
    six = [sr(s, state(s, expect=e), C(s, F_EMPTY))
           for e in (E_READY, E_R, E_X, E_RR, E_RX, E_RRX)]
    v.equation(F(s, 99, s.intern(1, E_RX), *six), six[4])


def _accessors(v):
    s = v.s
    v.equation(F(s, 61, s.intern(1, NIL)), C(s, TRUE))
    v.equation(F(s, 61, s.byte_list([5])), C(s, FALSE))
    v.equation(F(s, 68, C(s, WORDVALUE, s.word(9))),
               C(s, A_ADMITTED, s.word(9)))
    v.equation(F(s, 68, C(s, OVERFLOW)), C(s, A_EXHAUSTED))
    st = state(s, comment=True, expect=E_RRX, count=5, status=ST_INVALID,
               limit=99)
    v.equation(F(s, 85, st), state(s, comment=True, expect=E_RRX, count=5,
                                  status=ST_INVALID, limit=99))
    st2 = state(s, expect=E_X, count=2, limit=8)
    v.equation(F(s, 85, st2), state(s, comment=True, expect=E_X, count=2,
                                   limit=8))
    v.equation(F(s, 86, sr(s, st2, C(s, F_CHUNK, s.byte_list([1])))),
               sr(s, state(s, comment=True, expect=E_X, count=2, limit=8),
                  C(s, F_CHUNK, s.byte_list([1]))))
    v.equation(F(s, 87, C(s, WORDVALUE, s.word(1))),
               C(s, WORDVALUE, s.word(2)))
    v.equation(F(s, 87, C(s, OVERFLOW)), C(s, OVERFLOW))
    v.equation(F(s, 93, C(s, ST_OK)), C(s, TRUE))
    v.equation(F(s, 93, C(s, ST_INVALID)), C(s, FALSE))
    v.equation(F(s, 93, C(s, ST_EXHAUSTED)), C(s, FALSE))
    v.equation(F(s, 94, s.intern(1, E_READY)), C(s, TRUE))
    v.equation(F(s, 94, s.intern(1, E_RX)), C(s, FALSE))
    pair = sr(s, st, C(s, F_CHUNK, s.byte_list([7])))
    v.equation(F(s, 97, pair), st)
    v.equation(F(s, 98, pair), C(s, F_CHUNK, s.byte_list([7])))


def _lists(v):
    s = v.s
    v.equation(F(s, 69, s.intern(1, NIL), s.byte_list([1, 2, 3])),
               s.byte_list([3, 2, 1]))
    v.equation(F(s, 69, s.byte_list([9]), s.byte_list([1])),
               s.byte_list([1, 9]))
    v.equation(F(s, 69, s.intern(1, NIL), s.intern(1, NIL)),
               s.intern(1, NIL))
    v.equation(F(s, 70, s.byte_list([1, 2]), s.byte_list([9])),
               s.byte_list([1, 2, 9]))
    v.equation(F(s, 70, s.intern(1, NIL), s.byte_list([7])),
               s.byte_list([7]))
    frag = C(s, F_JOIN, C(s, F_CHUNK, s.byte_list([1])),
             C(s, F_CHUNK, s.byte_list([2])))
    v.equation(F(s, 71, frag, s.intern(1, NIL)), s.byte_list([1, 2]))
    v.equation(F(s, 71, C(s, F_EMPTY), s.byte_list([9])), s.byte_list([9]))
    v.equation(F(s, 71, C(s, F_CHUNK, s.byte_list([1, 2])),
                 s.byte_list([3])), s.byte_list([1, 2, 3]))


def _admission(v):
    s = v.s
    admitted = C(s, A_ADMITTED, s.word(0))
    v.equation(F(s, 72, s.byte(32), admitted), C(s, A_ADMITTED, s.word(1)))
    v.equation(F(s, 72, s.byte(0), admitted), C(s, A_REJECTED))
    v.equation(F(s, 72, s.byte(32), C(s, A_REJECTED)), C(s, A_REJECTED))
    v.equation(F(s, 72, s.byte(32), C(s, A_EXHAUSTED)), C(s, A_EXHAUSTED))
    v.equation(F(s, 72, s.byte(127), admitted), C(s, A_REJECTED))
    v.equation(F(s, 73, admitted, s.intern(1, S_EMPTY)), admitted)
    pair = s.source(b" !")
    v.equation(F(s, 73, admitted, pair), C(s, A_ADMITTED, s.word(2)))
    v.equation(F(s, 73, admitted, s.source(bytes([0]))),
               C(s, A_REJECTED))


def _hexadecimal(v):
    s = v.s
    v.equation(F(s, 74, s.word(0), nib(s, 5)), s.word(5))
    v.equation(F(s, 74, s.word(0x0123456789ABCDEF), nib(s, 0)),
               s.word(0x123456789ABCDEF0))
    v.equation(F(s, 74, s.word(0xF123456789ABCDEF), nib(s, 5)),
               s.word(0x123456789ABCDEF5))
    v.equation(F(s, 75, C(s, HEX, nib(s, 3))), C(s, D_RD1, nib(s, 3)))
    v.equation(F(s, 75, C(s, NO_HEX)), C(s, D_BAD))
    v.equation(F(s, 76, C(s, HEX, nib(s, 9))),
               C(s, D_REG2, nib(s, 0xE), nib(s, 9)))
    v.equation(F(s, 76, C(s, NO_HEX)), C(s, D_BAD))
    v.equation(F(s, 77, C(s, HEX, nib(s, 5)), nib(s, 1)),
               C(s, D_REG2, nib(s, 1), nib(s, 5)))
    v.equation(F(s, 77, C(s, NO_HEX), nib(s, 1)), C(s, D_BAD))
    v.equation(F(s, 78, C(s, HEX, nib(s, 0))),
               C(s, D_ZGO, s.word(0), s.byte(1)))
    v.equation(F(s, 78, C(s, NO_HEX)), C(s, D_BAD))
    v.equation(F(s, 79, C(s, HEX, nib(s, 2)), s.byte(ord('2')),
                 s.word(1), s.byte(1)),
               C(s, D_ZGO, s.word(0x12), s.byte(2)))
    v.equation(F(s, 79, C(s, HEX, nib(s, 2)), s.byte(ord('2')),
                 s.word(1), s.byte(16)), C(s, D_BAD))
    v.equation(F(s, 79, C(s, NO_HEX), s.byte(ord(':')), s.word(7),
                 s.byte(3)), C(s, D_ZDONE, s.word(7)))
    v.equation(F(s, 79, C(s, NO_HEX), s.byte(ord('z')), s.word(7),
                 s.byte(3)), C(s, D_BAD))


def _dfa_step(v):
    s = v.s
    cases = (
        (D_R1, 'e', C(s, D_RE)),
        (D_R1, '5', C(s, D_RD1, nib(s, 5))),
        (D_R1, 'x', C(s, D_BAD)),
        (D_RE, 't', C(s, D_DONE, mnemonic(s, 0x14, E_READY))),
        (D_RE, 'a', C(s, D_REA)),
        (D_RE, 'b', C(s, D_REG2, nib(s, 0xE), nib(s, 0xB))),
        (D_REA, 'd', C(s, D_DONE, mnemonic(s, 0x11, E_R))),
        (D_REA, 'f', C(s, D_BAD)),
        (D_Z1, 'x', C(s, D_ZX)),
        (D_Z1, '0', C(s, D_BAD)),
        (D_ZX, 'a', C(s, D_ZGO, s.word(0xA), s.byte(1))),
        (D_ZX, ':', C(s, D_BAD)),
        (D_LODB, 'x', C(s, D_BAD)),
        (D_A1, 'd', C(s, D_A2)),
        (D_A2, 'd', C(s, D_DONE, mnemonic(s, 0x03, E_RR))),
        (D_I2, 'm', C(s, D_DONE, mnemonic(s, 0x01, E_RX))),
        (D_I2, 'p', C(s, D_BAD)),
        (D_J1, 'z', C(s, D_DONE, mnemonic(s, 0x0D, E_RX))),
        (D_M1, 'o', C(s, D_MO)),
        (D_MO, 'd', C(s, D_DONE, mnemonic(s, 0x07, E_RR))),
        (D_MO, 'v', C(s, D_DONE, mnemonic(s, 0x02, E_RR))),
        (D_S1, 't', C(s, D_ST)),
        (D_STREB, 'x', C(s, D_BAD)),
        (D_W4, 'e', C(s, D_DONE, mnemonic(s, 0x12, E_R))),
    )
    for state_ctor, char, expected in cases:
        v.equation(F(s, 80, s.intern(1, state_ctor), s.byte(ord(char))),
                   expected)
    v.equation(F(s, 80, C(s, D_DONE, C(s, T_DW)), s.byte(ord('a'))),
               C(s, D_BAD))
    v.equation(F(s, 80, C(s, D_RD1, nib(s, 7)), s.byte(ord('a'))),
               C(s, D_REG2, nib(s, 7), nib(s, 0xA)))
    v.equation(F(s, 80, C(s, D_RD1, nib(s, 7)), s.byte(ord('z'))),
               C(s, D_BAD))
    v.equation(F(s, 80, C(s, D_ZGO, s.word(0xF), s.byte(1)),
                 s.byte(ord('5'))),
               C(s, D_ZGO, s.word(0xF5), s.byte(2)))
    v.equation(F(s, 80, C(s, D_ZGO, s.word(0xF), s.byte(1)),
                 s.byte(ord(':'))), C(s, D_ZDONE, s.word(0xF)))


def _dfa_fold_end(v):
    s = v.s
    v.equation(F(s, 81, s.intern(1, D_I1), s.byte_list(b'mm')),
               C(s, D_DONE, mnemonic(s, 0x01, E_RX)))
    v.equation(F(s, 81, s.intern(1, D_R1), s.byte_list(b'et')),
               C(s, D_DONE, mnemonic(s, 0x14, E_READY)))
    v.equation(F(s, 81, s.intern(1, D_LOD), s.byte_list(b'b')),
               C(s, D_LODB))
    v.equation(F(s, 81, C(s, D_ZGO, s.word(9), s.byte(1)),
                 s.intern(1, NIL)), C(s, D_ZGO, s.word(9), s.byte(1)))
    for state_ctor, expected in (
        (D_DONE, None),
        (D_RE, C(s, T_REGISTER, s.byte(0x0E))),
        (D_REA, C(s, T_REGISTER, s.byte(0xEA))),
        (D_RD1, None),
        (D_REG2, None),
        (D_ZGO, None),
        (D_ZDONE, None),
        (D_LOD, mnemonic(s, 0x0A, E_RR)),
        (D_LODB, mnemonic(s, 0x08, E_RR)),
        (D_STRE, mnemonic(s, 0x0B, E_RR)),
        (D_STREB, mnemonic(s, 0x09, E_RR)),
        (D_BAD, C(s, T_INVALID)),
        (D_R1, C(s, T_INVALID)),
    ):
        if state_ctor == D_DONE:
            arg = C(s, D_DONE, C(s, T_DW))
            expected = C(s, T_DW)
        elif state_ctor == D_RD1:
            arg = C(s, D_RD1, nib(s, 7))
            expected = C(s, T_REGISTER, s.byte(7))
        elif state_ctor == D_REG2:
            arg = C(s, D_REG2, nib(s, 0xA), nib(s, 0xB))
            expected = C(s, T_REGISTER, s.byte(0xAB))
        elif state_ctor == D_ZGO:
            arg = C(s, D_ZGO, s.word(0x1F), s.byte(2))
            expected = C(s, T_WORD, s.word(0x1F))
        elif state_ctor == D_ZDONE:
            arg = C(s, D_ZDONE, s.word(0x40))
            expected = C(s, T_ASSERT, s.word(0x40))
        else:
            arg = s.intern(1, state_ctor)
        v.equation(F(s, 82, arg), expected)


def _classify(v):
    s = v.s
    for char, expected in ((ord('r'), D_R1), (ord('0'), D_Z1),
                           (ord('a'), D_A1), (ord('z'), D_BAD),
                           (ord(';'), D_BAD), (ord(' '), D_BAD),
                           (ord('w'), D_W1), (ord('j'), D_J1)):
        v.equation(F(s, 83, s.byte(char)), s.intern(1, expected))
    v.equation(F(s, 84, s.intern(1, NIL)), C(s, T_EMPTY))
    tokens = (
        (b'r5', C(s, T_REGISTER, s.byte(5))),
        (b'rff', C(s, T_REGISTER, s.byte(0xFF))),
        (b'0x1f', C(s, T_WORD, s.word(0x1F))),
        (b'0x1f:', C(s, T_ASSERT, s.word(0x1F))),
        (b'dw', C(s, T_DW)),
        (b'halt', mnemonic(s, 0x00, E_R)),
        (b'imm', mnemonic(s, 0x01, E_RX)),
        (b'imp', C(s, T_INVALID)),
        (b'loadb', mnemonic(s, 0x08, E_RR)),
        (b'store', mnemonic(s, 0x0B, E_RR)),
        (b'zz', C(s, T_INVALID)),
        (b'rabc', C(s, T_INVALID)),
        (b'0x', C(s, T_INVALID)),
        (b'ret', mnemonic(s, 0x14, E_READY)),
        (b're', C(s, T_REGISTER, s.byte(0x0E))),
        (b'rea', C(s, T_REGISTER, s.byte(0xEA))),
        (b'read', mnemonic(s, 0x11, E_R)),
    )
    for spelling, expected in tokens:
        v.equation(F(s, 84, s.byte_list(spelling)), expected)


def _emitting(v):
    s = v.s
    frag = C(s, F_CHUNK, s.byte_list([0xAA]))
    # emit_byte_count(WordResult, Byte, next, comment, old, count, status,
    #                 limit, fragment)
    kept = sr(s, state(s, expect=E_R, count=1, limit=8),
              C(s, F_JOIN, frag, C(s, F_CHUNK, s.byte_list([7]))))
    v.equation(F(s, 88, C(s, WORDVALUE, s.word(1)), s.byte(7),
                 s.intern(1, E_R), s.intern(1, FALSE),
                 s.intern(1, E_READY), s.word(0), s.intern(1, ST_OK),
                 s.word(8), frag), kept)
    over_limit = sr(s, state(s, count=0, status=ST_EXHAUSTED,
                           limit=0), frag)
    v.equation(F(s, 88, C(s, WORDVALUE, s.word(1)), s.byte(7),
                 s.intern(1, E_R), s.intern(1, FALSE),
                 s.intern(1, E_READY), s.word(0), s.intern(1, ST_OK),
                 s.word(0), frag), over_limit)
    stuck = sr(s, state(s, expect=E_READY, count=0, status=ST_EXHAUSTED,
                        limit=8), frag)
    v.equation(F(s, 88, C(s, OVERFLOW), s.byte(7), s.intern(1, E_R),
                 s.intern(1, FALSE), s.intern(1, E_READY), s.word(0),
                 s.intern(1, ST_EXHAUSTED), s.word(8), frag), stuck)
    # emit_byte(7, E_R, state, fragment) delegates to emit_byte_count.
    st = state(s, expect=E_X, count=0, limit=8)
    v.equation(F(s, 89, s.byte(7), s.intern(1, E_R), st, frag), kept)
    # emit_word_count and emit_word emit the eight little-endian bytes.
    word_frag = C(s, F_JOIN, frag,
                  C(s, F_CHUNK,
                    s.byte_list([0x22, 0x11, 0, 0, 0, 0, 0, 0])))
    v.equation(F(s, 90, C(s, WORDVALUE, s.word(8)), s.word(0x1122),
                 s.intern(1, E_READY), s.intern(1, FALSE),
                 s.intern(1, E_X), s.word(0), s.intern(1, ST_OK),
                 s.word(8), frag),
               sr(s, state(s, count=8, limit=8), word_frag))
    v.equation(F(s, 90, C(s, WORDVALUE, s.word(9)), s.word(0x1122),
                 s.intern(1, E_READY), s.intern(1, FALSE),
                 s.intern(1, E_X), s.word(0), s.intern(1, ST_OK),
                 s.word(8), frag),
               sr(s, state(s, expect=E_X, count=0, status=ST_EXHAUSTED,
                           limit=8), frag))
    v.equation(F(s, 91, s.word(0x1122), s.intern(1, E_READY), st, frag),
               sr(s, state(s, count=8, limit=8), word_frag))


def _dispatch(v):
    s = v.s
    # dispatch(comment, expect, count, status, limit, TokenClass)
    def disp(expect, count, token, status=ST_OK, limit=8):
        return F(s, 100, s.intern(1, FALSE), s.intern(1, expect),
                 s.word(count), s.intern(1, status), s.word(limit), token)
    cleared = lambda **kw: state(s, limit=8, **kw)
    # dw only at Ready; it awaits a word and emits nothing itself.
    v.equation(disp(E_READY, 0, C(s, T_DW)),
               sr(s, cleared(expect=E_X), C(s, F_EMPTY)))
    v.equation(disp(E_X, 0, C(s, T_DW)),
               sr(s, cleared(expect=E_X, status=ST_INVALID),
                  C(s, F_EMPTY)))
    # A register byte at E_RX: emit it and await the word.
    v.equation(disp(E_RX, 0, C(s, T_REGISTER, s.byte(5))),
               sr(s, cleared(expect=E_X, count=1),
                  C(s, F_JOIN, C(s, F_EMPTY),
                    C(s, F_CHUNK, s.byte_list([5])))))
    # A register at Ready is a misplaced operand.
    v.equation(disp(E_READY, 0, C(s, T_REGISTER, s.byte(5))),
               sr(s, cleared(status=ST_INVALID), C(s, F_EMPTY)))
    # A word at E_X emits eight bytes and returns to Ready.
    frag8 = C(s, F_JOIN, C(s, F_EMPTY),
              C(s, F_CHUNK, s.byte_list([0x2A, 0, 0, 0, 0, 0, 0, 0])))
    v.equation(disp(E_X, 0, C(s, T_WORD, s.word(0x2A))),
               sr(s, cleared(count=8), frag8))
    v.equation(disp(E_READY, 0, C(s, T_WORD, s.word(0x2A))),
               sr(s, cleared(status=ST_INVALID), C(s, F_EMPTY)))
    # Assertions emit nothing and require address == count in Ready.
    v.equation(disp(E_READY, 0, C(s, T_ASSERT, s.word(0))),
               sr(s, cleared(), C(s, F_EMPTY)))
    v.equation(disp(E_READY, 1, C(s, T_ASSERT, s.word(0))),
               sr(s, cleared(count=1, status=ST_INVALID), C(s, F_EMPTY)))
    v.equation(disp(E_X, 0, C(s, T_ASSERT, s.word(0))),
               sr(s, cleared(expect=E_X, status=ST_INVALID),
                  C(s, F_EMPTY)))
    # Mnemonics emit their opcode byte only in Ready.
    v.equation(disp(E_READY, 0, mnemonic(s, 0x00, E_R)),
               sr(s, cleared(expect=E_R, count=1),
                  C(s, F_JOIN, C(s, F_EMPTY),
                    C(s, F_CHUNK, s.byte_list([0])))))
    v.equation(disp(E_R, 0, mnemonic(s, 0x14, E_READY)),
               sr(s, cleared(expect=E_R, status=ST_INVALID),
                  C(s, F_EMPTY)))
    v.equation(disp(E_READY, 0, C(s, T_INVALID)),
               sr(s, cleared(status=ST_INVALID), C(s, F_EMPTY)))
    v.equation(disp(E_READY, 0, C(s, T_EMPTY)),
               sr(s, cleared(status=ST_INVALID), C(s, F_EMPTY)))


def _flush_scan(v):
    s = v.s
    # flush on an empty pending token returns the state with no fragment.
    st = state(s, count=3, limit=8)
    v.equation(F(s, 101, st), sr(s, st, C(s, F_EMPTY)))
    # flush of a pending 'ret' in Ready emits its opcode byte.
    pending = s.byte_list(b'ter')  # stored reversed
    st2 = state(s, pending=pending, count=0, limit=8)
    v.equation(F(s, 101, st2),
               sr(s, state(s, count=1, limit=8),
                  C(s, F_JOIN, C(s, F_EMPTY),
                    C(s, F_CHUNK, s.byte_list([0x14])))))
    # scan_byte: separator flushes; comment swallows; others push.
    v.equation(F(s, 102, st, s.byte(ord(' '))), sr(s, st, C(s, F_EMPTY)))
    st_tok = state(s, pending=s.byte_list(b'r'), count=3, limit=8)
    v.equation(F(s, 102, st_tok, s.byte(ord('5'))),
               sr(s, state(s, pending=s.byte_list(b'5r'), count=3,
                           limit=8), C(s, F_EMPTY)))
    st_ret = state(s, pending=s.byte_list(b'ter'), count=3, limit=8)
    v.equation(F(s, 102, st_ret, s.byte(ord(';'))),
               sr(s, state(s, comment=True, count=4, limit=8),
                  C(s, F_JOIN, C(s, F_EMPTY),
                    C(s, F_CHUNK, s.byte_list([0x14])))))
    st_c = state(s, comment=True, count=4, limit=8)
    v.equation(F(s, 102, st_c, s.byte(ord('x'))),
               sr(s, st_c, C(s, F_EMPTY)))
    v.equation(F(s, 102, st_c, s.byte(10)),
               sr(s, state(s, count=4, limit=8), C(s, F_EMPTY)))
    st_bad = state(s, status=ST_INVALID, count=4, limit=8)
    v.equation(F(s, 102, st_bad, s.byte(ord(' '))),
               sr(s, st_bad, C(s, F_EMPTY)))
    # scan threads one state across a Join in source order.
    two = C(s, S_JOIN, C(s, S_LEAF, s.byte(ord('r'))),
            C(s, S_LEAF, s.byte(ord('5'))))
    v.equation(F(s, 103, state(s, limit=8), two),
               sr(s, state(s, pending=s.byte_list(b'5r'), limit=8),
                  C(s, F_JOIN, C(s, F_EMPTY), C(s, F_EMPTY))))


def _finish(v):
    s = v.s
    empty = C(s, F_EMPTY)
    v.equation(F(s, 104, state(s, count=1, limit=8),
                 C(s, F_CHUNK, s.byte_list([0x14]))),
               C(s, R_SUCCESS, s.byte_list([0x14])))
    v.equation(F(s, 104, state(s, expect=E_RX, count=1, limit=8),
                 C(s, F_CHUNK, s.byte_list([0x14]))), C(s, R_INVALID))
    v.equation(F(s, 104, state(s, count=1, status=ST_INVALID, limit=8),
                 empty), C(s, R_INVALID))
    v.equation(F(s, 104, state(s, count=1, status=ST_EXHAUSTED, limit=8),
                 empty), C(s, R_EXHAUSTED))
    scanned = sr(s, state(s, pending=s.byte_list(b'ter'), limit=8),
                 C(s, F_EMPTY))
    v.equation(F(s, 105, scanned),
               C(s, R_SUCCESS, s.byte_list([0x14])))
    admitted0 = C(s, A_ADMITTED, s.word(0))
    v.equation(F(s, 106, C(s, A_REJECTED), s.intern(1, S_EMPTY),
                 s.word(0), s.word(0)), C(s, R_INVALID))
    v.equation(F(s, 106, C(s, A_EXHAUSTED), s.intern(1, S_EMPTY),
                 s.word(0), s.word(0)), C(s, R_EXHAUSTED))
    v.equation(F(s, 106, admitted0, s.intern(1, S_EMPTY), s.word(0),
                 s.word(0)), C(s, R_SUCCESS, s.intern(1, NIL)))
    v.equation(F(s, 106, C(s, A_ADMITTED, s.word(1)),
                 s.intern(1, S_EMPTY), s.word(0), s.word(0)),
               C(s, R_EXHAUSTED))


def _encode_positive(v):
    s = v.s
    encode = lambda src, sl, ol: F(s, 107, s.source(src), s.word(sl),
                                   s.word(ol))
    success = lambda data: C(s, R_SUCCESS, s.byte_list(data))
    v.equation(encode(b'', 0, 0), C(s, R_SUCCESS, s.intern(1, NIL)))
    v.equation(encode(b' ', 1, 0), C(s, R_SUCCESS, s.intern(1, NIL)))
    v.equation(encode(b';c', 2, 0), C(s, R_SUCCESS, s.intern(1, NIL)))
    v.equation(encode(b'ret', 3, 1), success([0x14]))
    v.equation(encode(b'halt r9', 7, 2), success([0x00, 0x09]))
    v.equation(encode(b'0x0:', 4, 0),
               C(s, R_SUCCESS, s.intern(1, NIL)))
    v.equation(encode(b'dw 0x0', 6, 8), success([0] * 8))
    v.equation(encode(b'imm r5 0x10', 11, 10),
               success([0x01, 0x05, 0x10, 0, 0, 0, 0, 0, 0, 0]))
    v.equation(encode(b'ret 0x1: ret', 13, 2), success([0x14, 0x14]))


def _encode_negative(v):
    s = v.s
    encode = lambda src, sl, ol: F(s, 107, s.source(src), s.word(sl),
                                   s.word(ol))
    v.equation(encode(bytes([0]), 1, 0), C(s, R_INVALID))
    v.equation(encode(b'zz', 2, 0), C(s, R_INVALID))
    v.equation(encode(b'imp', 3, 0), C(s, R_INVALID))
    v.equation(encode(b'ret 0x2: ret', 13, 2), C(s, R_INVALID))
    v.equation(encode(b'r5', 2, 0), C(s, R_INVALID))
    v.equation(encode(b'imm r5', 6, 2), C(s, R_INVALID))
    v.equation(encode(b'ret', 2, 1), C(s, R_EXHAUSTED))
    v.equation(encode(b'dw 0x0', 6, 7), C(s, R_EXHAUSTED))
    v.equation(encode(b'ret', 3, 0), C(s, R_EXHAUSTED))


GROUPS = (
    ("encoder_predicates", _predicates),
    ("encoder_choosers", _choosers),
    ("encoder_accessors", _accessors),
    ("encoder_lists", _lists),
    ("encoder_admission", _admission),
    ("encoder_hexadecimal", _hexadecimal),
    ("encoder_dfa_step", _dfa_step),
    ("encoder_dfa_fold_end", _dfa_fold_end),
    ("encoder_classify", _classify),
    ("encoder_emitting", _emitting),
    ("encoder_dispatch", _dispatch),
    ("encoder_flush_scan", _flush_scan),
    ("encoder_finish", _finish),
    ("encoder_encode_positive", _encode_positive),
    ("encoder_encode_negative", _encode_negative),
)


# Exact checker work observations, pinned by running each request once;
# `python3 worksim.py` re-derives each figure from the checker source's
# charging model and fails on any divergence.
WORK = {
    "encoder_predicates": 913,
    "encoder_choosers": 175,
    "encoder_accessors": 1696,
    "encoder_lists": 643,
    "encoder_admission": 2551,
    "encoder_hexadecimal": 5888,
    "encoder_dfa_step": 15240,
    "encoder_dfa_fold_end": 3892,
    "encoder_classify": 60538,
    "encoder_emitting": 7297,
    "encoder_dispatch": 10978,
    "encoder_flush_scan": 20511,
    "encoder_finish": 13725,
    "encoder_encode_positive": 123466,
    "encoder_encode_negative": 100183,
}


def cases(definitions):
    theory = Theory(definitions)
    for name, fill in GROUPS:
        vector = Vector(theory)
        fill(vector)
        request, count, owner, witnesses, proofs = vector.build(definitions)
        yield name, request, checked(count, WORK.get(name, 0))
    yield from mutations(definitions)


def _mutated(definitions, theory, fill, index, replacement, field, code=12):
    """Rebuild one group's request with a single corrupted proof row."""
    vector = Vector(theory)
    fill(vector)
    request, count, owner, witnesses, proofs = vector.build(definitions)
    changed = list(proofs)
    changed[index - 1] = replacement
    start = (24 + len(definitions) + len(owner) + 12
             + sum(map(len, witnesses))
             + sum(map(len, proofs[:index - 1])))
    coordinate = start + field
    cert = certificate(terms=witnesses, proofs=changed)
    return envelope((definitions, owner, cert)), rejected(coordinate, code)


def mutations(definitions):
    theory = Theory(definitions)

    # encoder_predicates row 7 unfolds f22's 256-clause table on byte 0x61;
    # clause 97 names the adjacent byte's constructor and cannot match.
    yield ("encoder_unfold_wrong_clause",
           *_mutated(definitions, theory, _predicates, 7,
                     record(5, 16, 21, 97), 16, 10))

    # Row 1 unfolds f58(EQUAL) to TRUE; claiming FALSE keeps the clause but
    # lands on a different same-sort term.
    yield ("encoder_unfold_wrong_result",
           *_mutated(definitions, theory, _predicates, 1,
                     record(5, 6, 1, 2), 12))

    # Row 11 is a transitivity row; moving its second premise past the end
    # is a missing/forward premise.
    yield ("encoder_forward_premise",
           *_mutated(definitions, theory, _predicates, 11,
                     record(3, 22, 5, 9, 99), 20))

    # The same row with swapped premises concludes nothing: claimed left no
    # longer matches the first premise's left.
    yield ("encoder_swapped_transitivity",
           *_mutated(definitions, theory, _predicates, 11,
                     record(3, 22, 5, 10, 9), 8))

    # Row 8 is congruence with two premises; pointing the second at a
    # forward row is an invalid (missing) premise.
    yield ("encoder_congruence_forward_premise",
           *_mutated(definitions, theory, _predicates, 8,
                     record(4, 17, 22, 2, 7, 40), 24))

    # The last row must conclude the owner root; replaying an earlier row
    # leaves the root unproved.
    vector = Vector(theory)
    _predicates(vector)
    request, count, owner, witnesses, proofs = vector.build(definitions)
    changed = proofs[:-1] + [proofs[10]]
    start = (24 + len(definitions) + len(owner) + 12
             + sum(map(len, witnesses))
             + sum(map(len, changed[:-1])))
    cert = certificate(terms=witnesses, proofs=changed)
    yield ("encoder_root_not_concluded",
           envelope((definitions, owner, cert)),
           rejected(start + 8))

    # A certificate with no proof rows rejects at the proof count field.
    cert = certificate(terms=witnesses, proofs=())
    yield ("encoder_zero_proof_rows",
           envelope((definitions, owner, cert)),
           rejected(24 + len(definitions) + len(owner) + 8
                    + sum(map(len, witnesses))))

"""Independent fixed wire identity; its bytes never become checker input."""

import hashlib

from lexical import BOOLEAN_TABLES, HEX_BYTES
from wire import clause, function, record, theory


def fixed_identity():
    constructors = [record(1, 0)] * 256 + [record(2, 0)] * 2
    constructors += [record(3, 0)] * 16 + [record(4, 0), record(4, 1, 3)]
    constructors += [record(5, 8, *([1] * 8)), record(6, 0), record(6, 2, 1, 6)]
    functions = []
    for admitted in BOOLEAN_TABLES:
        functions.append(function((1,), [clause((record(1, 257 + int(byte in admitted), 0),), byte + 1, 1) for byte in range(256)], mode=1, result=2))
    clauses = []
    for byte in range(256):
        templates = ((record(1, 259 + HEX_BYTES.index(byte), 0), record(1, 276, 1, 1))
                     if byte in HEX_BYTES else (record(1, 275, 0),))
        clauses.append(clause(templates, byte + 1, len(templates)))
    functions.append(function((1,), clauses, mode=1, result=4))
    for high in range(16):
        functions.append(function((3,), [clause((record(1, 16 * high + low + 1, 0),), 259 + low, 1) for low in range(16)], mode=1))
    functions.append(function((3, 3), [clause((record(0, 1), record(2, 5 + high, 1, 1)), 259 + high, 2) for high in range(16)], mode=1))
    for high in (True, False):
        functions.append(function((1,), [clause((record(1, 259 + (byte // 16 if high else byte % 16), 0),), byte + 1, 1) for byte in range(256)], mode=1, result=3))
    templates = [record(0, slot) for slot in range(1, 9)] + [record(1, 278, 0)]
    templates.extend(record(1, 279, 2, 8 - position, 9 + position) for position in range(8))
    functions.append(function((5,), (clause(templates, 277, 17),), mode=1, result=6))
    package = theory(constructors, functions, sorts=6)
    return len(package), hashlib.sha256(package).hexdigest()

#!/usr/bin/env python3
"""Development Gamma compiler: produce an independent Alpha tape.

Python is an additional trusted semantic producer in this experimental variant.
The output runs on the existing Alpha seed without a Gamma evaluator. This is
not yet a predecessor-language implementation or a source-only bootstrap.
"""

import argparse
import os
from pathlib import Path
import re
import struct
import sys
import tempfile


MASK = (1 << 64) - 1
STACK, STACK_END = 0x02000000, 0x08000000
INPUT, INPUT_END = 0x10000000, 0x18300000
OUTPUT, OUTPUT_END = 0x20000000, 0x28000000
HEAP, HEAP_END = 0x30000000, 0x2000000000
SP, FP, HP, IN_LEN, OUT_LEN, DEPTH = 246, 247, 248, 249, 250, 243
OPS = {
    "halt": (0, "r"), "imm": (1, "ri"), "mov": (2, "rr"),
    "add": (3, "rr"), "sub": (4, "rr"), "mul": (5, "rr"),
    "div": (6, "rr"), "mod": (7, "rr"), "loadb": (8, "rr"),
    "storeb": (9, "rr"), "load": (10, "rr"), "store": (11, "rr"),
    "jmp": (12, "i"), "jz": (13, "ri"), "jnz": (14, "ri"),
    "jlt": (15, "rri"), "jeq": (16, "rri"), "read": (17, "r"),
    "write": (18, "r"), "call": (19, "i"), "ret": (20, ""),
}
BINARY = {"+": "add", "-": "sub", "*": "mul", "/": "div", "%": "mod"}
ARITY = dict.fromkeys([*BINARY, "eq", "lt", "pair"], 2)
ARITY.update(dict.fromkeys(["read", "write", "first", "second"], 1))
ARITY["input"] = 0


class Invalid(ValueError):
    pass


def parse(source):
    """Parse authored lists; comments and character literals are lexical forms."""
    if any(b not in (9, 10, 13) and not 32 <= b <= 126 for b in source):
        raise Invalid("Gamma source contains a forbidden byte")
    text = source.decode("ascii")
    roots, stack, at = [], [], 0
    while at < len(text):
        c = text[at]
        if c.isspace():
            at += 1
            continue
        if c == ";":
            while at < len(text) and text[at] not in "\r\n":
                at += 1
            continue
        if c == "(":
            node = []
            (stack[-1] if stack else roots).append(node)
            stack.append(node)
            if len(stack) > 255:
                raise Invalid("authored list nesting exceeds 255")
            at += 1
            continue
        if c == ")":
            if not stack:
                raise Invalid("unmatched closing parenthesis")
            stack.pop()
            at += 1
            continue
        if c == "'":
            if text[at:at + 4] in ("'\\n'", "'\\s'"):
                token = 10 if text[at + 2] == "n" else 32
                at += 4
            elif (at + 2 < len(text) and text[at + 2] == "'"
                  and 32 <= ord(text[at + 1]) <= 126 and text[at + 1] not in "'\\"):
                token = ord(text[at + 1])
                at += 3
            else:
                raise Invalid("malformed character literal")
        else:
            end = at
            while end < len(text) and not text[end].isspace() and text[end] not in "();":
                end += 1
            token = text[at:end]
            if re.fullmatch(r"-?[0-9]+", token):
                value = 0
                for digit in token.lstrip("-"):
                    value = (value * 10 + ord(digit) - 48) & MASK
                token = (-value if token.startswith("-") else value) & MASK
            at = end
        (stack[-1] if stack else roots).append(token)
    if stack:
        raise Invalid("unclosed list")
    return roots


def name(value):
    return isinstance(value, str)


def declarations(roots):
    functions = {}
    application = bool(roots and roots[0] == ["def", "$application", [], "Int", 1])
    for root in roots:
        if not isinstance(root, list) or len(root) != 5 or root[0] != "def" or root[3] != "Int":
            raise Invalid("expected (def NAME ((PARAM Int)...) Int BODY)")
        _, ident, parameters, _, body = root
        if not name(ident) or ident in functions or not isinstance(parameters, list):
            raise Invalid("invalid or duplicate function")
        if ident == "$application" and (not application or root is not roots[0]):
            raise Invalid("invalid application marker")
        params = []
        for parameter in parameters:
            if not isinstance(parameter, list) or len(parameter) != 2 or parameter[1] != "Int" or not name(parameter[0]):
                raise Invalid("invalid parameter")
            if parameter[0] in params:
                raise Invalid("duplicate parameter")
            params.append(parameter[0])
        functions[ident] = (params, body)
    if "main" not in functions or functions["main"][0]:
        raise Invalid("exactly one nullary main is required")
    return functions, application


class Tape:
    def __init__(self):
        self.data = bytearray()
        self.labels = {}
        self.fixups = []
        self.serial = 0

    def fresh(self):
        self.serial += 1
        return f"L{self.serial}"

    def mark(self, label):
        if label in self.labels:
            raise Invalid("duplicate machine label")
        self.labels[label] = len(self.data)

    def emit(self, op, *args):
        opcode, widths = OPS[op]
        assert len(args) == len(widths)
        self.data.append(opcode)
        for width, arg in zip(widths, args):
            if width == "r":
                self.data.append(arg)
            else:
                if isinstance(arg, str):
                    self.fixups.append((len(self.data), arg))
                    arg = 0
                self.data.extend(struct.pack("<Q", arg & MASK))

    def add(self, register, value):
        self.emit("imm", 12, value)
        self.emit("add", register, 12)

    def bound(self, register, end, error):
        self.emit("imm", 13, end)
        self.emit("jlt", 13, register, error)

    def push(self):
        self.emit("mov", 10, SP)
        self.add(10, 16)
        self.bound(10, STACK_END, "stack_error")
        self.emit("store", SP, 0)
        self.add(SP, 8)
        self.emit("store", SP, 1)
        self.add(SP, 8)

    def pop(self, value=0, tag=1):
        self.add(SP, -8)
        self.emit("load", tag, SP)
        self.add(SP, -8)
        self.emit("load", value, SP)

    def slot(self, offset, store=False):
        self.emit("mov", 10, FP)
        self.add(10, offset * 16)
        self.emit("store" if store else "load", 10 if store else 0, 0 if store else 10)
        self.add(10, 8)
        self.emit("store" if store else "load", 10 if store else 1, 1 if store else 10)

    def finish(self):
        if len(self.data) > 16777212:
            raise Invalid("compiled tape exceeds the Alpha seed hole")
        for offset, label in self.fixups:
            if label not in self.labels:
                raise Invalid(f"unresolved label {label}")
            struct.pack_into("<Q", self.data, offset, self.labels[label])
        return bytes(self.data)


class Compiler:
    def __init__(self, source):
        self.functions, self.application = declarations(parse(source))
        self.t = Tape()

    def expr(self, expr, env, tail=False):
        t = self.t
        if isinstance(expr, int):
            t.emit("imm", 0, expr)
            t.emit("imm", 1, 0)
            t.push()
            return
        if isinstance(expr, str):
            if expr not in env:
                raise Invalid(f"unbound name {expr}")
            t.slot(env[expr])
            t.push()
            return
        if not expr or not isinstance(expr[0], str):
            raise Invalid("expression requires an operator")
        op, *args = expr
        if op == "if":
            if len(args) != 3:
                raise Invalid("if requires three operands")
            other, done = t.fresh(), t.fresh()
            self.expr(args[0], env)
            t.pop()
            t.emit("jnz", 1, "trap")
            t.emit("jz", 0, other)
            self.expr(args[1], env, tail)
            t.emit("jmp", done)
            t.mark(other)
            self.expr(args[2], env, tail)
            t.mark(done)
            return
        if op == "let":
            if len(args) == 4 and args[1] == "Int":
                bindings, body = [[args[0], args[1], args[2]]], args[3]
            elif len(args) == 2 and isinstance(args[0], list):
                bindings, body = args
            else:
                raise Invalid("malformed let")
            nested = dict(env)
            for binding in bindings:
                if not isinstance(binding, list) or len(binding) != 3 or binding[1] != "Int" or not name(binding[0]) or binding[0] in nested:
                    raise Invalid("invalid or shadowed let binder")
                self.expr(binding[2], nested)
                t.pop()
                slot = self.locals
                self.locals += 1
                t.slot(slot, store=True)
                nested[binding[0]] = slot
            self.expr(body, nested, tail)
            return
        if op in self.functions and op not in ARITY:
            if op == "$application" or len(args) != len(self.functions[op][0]):
                raise Invalid("invalid function arity or marker call")
            for arg in args:
                self.expr(arg, env)
            if tail:
                # Arguments have been evaluated before any old binding is overwritten.
                t.slot(self.parameter_count)
                t.emit("mov", 14, 0)
                t.emit("mov", 8, SP)
                t.add(8, -16 * len(args))
                t.emit("mov", 9, FP)
                for _ in args:
                    for _ in range(2):
                        t.emit("load", 0, 8)
                        t.emit("store", 9, 0)
                        t.add(8, 8)
                        t.add(9, 8)
                t.emit("mov", SP, 9)
                t.emit("mov", FP, 14)
                t.emit("jmp", f"fn:{op}")
            else:
                t.add(DEPTH, 1)
                t.bound(DEPTH, 131072, "stack_error")
                t.emit("call", f"fn:{op}")
                t.push()
            return
        if op not in ARITY or len(args) != ARITY[op]:
            raise Invalid(f"unknown operator or wrong arity: {op}")
        for arg in args:
            self.expr(arg, env)
        if len(args) == 2:
            t.pop(2, 3)
        if args:
            t.pop()
        if op == "pair":
            t.emit("mov", 10, HP)
            t.add(10, 32)
            t.bound(10, HEAP_END, "heap_error")
            t.emit("mov", 4, HP)
            for register in (0, 1, 2, 3):
                t.emit("store", HP, register)
                t.add(HP, 8)
            t.emit("mov", 0, 4)
            t.emit("imm", 1, 1)
        elif op in ("first", "second"):
            t.emit("jz", 1, "trap")
            t.emit("mov", 10, 0)
            if op == "second":
                t.add(10, 16)
            t.emit("load", 0, 10)
            t.add(10, 8)
            t.emit("load", 1, 10)
        else:
            if args:
                t.emit("jnz", 1, "trap")
            if len(args) == 2:
                t.emit("jnz", 3, "trap")
            if op in BINARY:
                if op in ("/", "%"):
                    good = t.fresh()
                    t.emit("jz", 2, "trap")
                    t.emit("imm", 4, 1 << 63)
                    t.emit("jeq", 0, 4, good)
                    after = t.fresh()
                    t.emit("jmp", after)
                    t.mark(good)
                    t.emit("imm", 4, MASK)
                    t.emit("jeq", 2, 4, "trap")
                    t.mark(after)
                t.emit(BINARY[op], 0, 2)
            elif op in ("eq", "lt"):
                yes, done = t.fresh(), t.fresh()
                t.emit("jeq" if op == "eq" else "jlt", 0, 2, yes)
                t.emit("imm", 0, 0)
                t.emit("jmp", done)
                t.mark(yes)
                t.emit("imm", 0, 1)
                t.mark(done)
            elif op == "input":
                t.emit("mov", 0, IN_LEN)
            elif op == "read":
                t.emit("imm", 4, 0)
                t.emit("jlt", 0, 4, "trap")
                good = t.fresh()
                t.emit("jlt", 0, IN_LEN, good)
                t.emit("jmp", "trap")
                t.mark(good)
                t.add(0, INPUT)
                t.emit("loadb", 0, 0)
            elif op == "write":
                t.emit("call", "buffer_byte")
            t.emit("imm", 1, 0)
        t.push()

    def compile(self):
        t = self.t
        for reg, value in ((SP, STACK), (FP, 0), (HP, HEAP), (IN_LEN, 0), (OUT_LEN, 0), (DEPTH, 1)):
            t.emit("imm", reg, value)
        read_loop, read_done = t.fresh(), t.fresh()
        t.mark(read_loop)
        t.emit("read", 0)
        t.emit("imm", 1, MASK)
        t.emit("jeq", 0, 1, read_done)
        t.emit("mov", 10, IN_LEN)
        t.add(10, 1)
        t.bound(10, INPUT_END - INPUT, "input_error")
        t.emit("mov", 10, IN_LEN)
        t.add(10, INPUT)
        t.emit("storeb", 10, 0)
        t.add(IN_LEN, 1)
        t.emit("jmp", read_loop)
        t.mark(read_done)
        t.emit("call", "fn:main")
        if self.application:
            t.emit("jz", 1, "trap")
            t.emit("mov", 10, 0)
            t.emit("load", 15, 10)
            t.add(10, 8)
            t.emit("load", 1, 10)
            t.emit("jnz", 1, "trap")
            t.add(10, 8)
            t.emit("load", 2, 10)
            t.add(10, 8)
            t.emit("load", 3, 10)
            t.emit("jnz", 3, "trap")
            t.emit("imm", 4, 0)
            t.emit("jlt", 15, 4, "trap")
            t.emit("imm", 4, 254)
            t.emit("jlt", 4, 15, "trap")
            discard, publish = t.fresh(), t.fresh()
            t.emit("jz", 2, discard)
            t.emit("imm", 4, 1)
            t.emit("jeq", 2, 4, publish)
            t.emit("jmp", "trap")
            t.mark(discard)
            t.emit("jz", 15, "trap")
            t.emit("halt", 15)
            t.mark(publish)
            valid = t.fresh()
            t.emit("jz", 15, valid)
            t.emit("jz", OUT_LEN, "trap")
            t.mark(valid)
        else:
            t.emit("jnz", 1, "trap")
            t.emit("call", "buffer_byte")
            t.emit("imm", 15, 0)
        t.emit("imm", 8, 0)
        flush, flushed = t.fresh(), t.fresh()
        t.mark(flush)
        t.emit("jeq", 8, OUT_LEN, flushed)
        t.emit("mov", 10, 8)
        t.add(10, OUTPUT)
        t.emit("loadb", 0, 10)
        t.emit("write", 0)
        t.add(8, 1)
        t.emit("jmp", flush)
        t.mark(flushed)
        t.emit("halt", 15)
        t.mark("buffer_byte")
        t.emit("imm", 4, 0)
        t.emit("jlt", 0, 4, "trap")
        t.emit("imm", 4, 255)
        t.emit("jlt", 4, 0, "trap")
        t.emit("mov", 10, OUT_LEN)
        t.add(10, 1)
        t.bound(10, OUTPUT_END - OUTPUT, "output_error")
        t.emit("mov", 10, OUT_LEN)
        t.add(10, OUTPUT)
        t.emit("storeb", 10, 0)
        t.add(OUT_LEN, 1)
        t.emit("ret")
        for label, status in (("trap", 249), ("stack_error", 250), ("heap_error", 252), ("input_error", 253), ("output_error", 254)):
            t.mark(label)
            t.emit("imm", 0, status if self.application else (2 if label == "trap" else 3))
            t.emit("halt", 0)
        for ident, (params, body) in self.functions.items():
            self.parameter_count = len(params)
            self.locals = len(params) + 1  # Saved caller frame occupies one cell.
            t.mark(f"fn:{ident}")
            t.emit("mov", 0, FP)
            t.emit("imm", 1, 0)
            t.emit("mov", FP, SP)
            t.add(FP, -16 * len(params))
            t.push()
            # Patch the reservation after assigning every lexical binding a home.
            t.emit("imm", 12, 0)
            reservation = len(t.data) - 8
            t.emit("add", SP, 12)
            t.bound(SP, STACK_END, "stack_error")
            self.expr(body, {p: i for i, p in enumerate(params)}, tail=True)
            struct.pack_into("<Q", t.data, reservation, 16 * (self.locals - len(params) - 1))
            t.pop()
            t.emit("mov", 6, 0)
            t.emit("mov", 7, 1)
            t.slot(len(params))
            t.emit("mov", SP, FP)
            t.emit("mov", FP, 0)
            t.emit("mov", 0, 6)
            t.emit("mov", 1, 7)
            t.add(DEPTH, -1)
            t.emit("ret")
        return t.finish()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        tape = Compiler(args.source.read_bytes()).compile()
    except (Invalid, RecursionError) as error:
        print(f"Gamma compilation refused: {error}", file=sys.stderr)
        return 1
    args.output.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(dir=args.output.parent)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(tape)
        os.replace(temporary, args.output)
    finally:
        Path(temporary).unlink(missing_ok=True)
    print(f"compiled {len(tape)} Alpha bytes; producer=host-python", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.setrecursionlimit(4096)
    raise SystemExit(main())

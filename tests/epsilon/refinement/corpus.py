"""Refinement corpus: programs, stdin variants, and mutations.

Every expectation in this gate is derived by the independent model
(model.py), never asserted as a fixture: a mutation entry re-derives the
canonical observation for the mutated source or stdin, and the evaluator's
published observation must equal it byte-for-byte. That is what makes the
check a refinement check rather than a table of pinned answers — the model
reconstructs Epsilon meaning independently, and a mutation only passes when
the evaluator follows the same contract on the mutated input.

Entry shape: (name, source, stdin, source_mutations, stdin_mutations).
A mutation is a function applied to the source bytes (or a literal stdin
bytes replacement). `swap(old, new)` requires `old` to occur exactly once.
"""

CONSOLE = b"""boundary trait Console {
  machine exit_process(return_code: i32) -> never;
  machine write_byte(value: i32);
  machine read_byte() -> i32;
  machine write_line(text: &[u8]);
}

"""

MAIN_ONLY = CONSOLE + b"""data Main {
  console: Console;
}

"""


def swap(old, new):
    def apply(source):
        count = source.count(old)
        if count != 1:
            raise ValueError(f"{old!r} occurs {count} times")
        return source.replace(old, new)
    apply.__name__ = f"{old!r} -> {new!r}"
    return apply


def insert_at(offset, data):
    def apply(source):
        return source[:offset] + data + source[offset:]
    apply.__name__ = f"insert {data!r} at {offset}"
    return apply


def drop_last(n):
    def apply(source):
        return source[:-n]
    apply.__name__ = f"drop last {n} bytes"
    return apply


def chain(*mutations):
    def apply(source):
        for mutation in mutations:
            source = mutation(source)
        return source
    apply.__name__ = " -> ".join(m.__name__ for m in mutations)
    return apply


def append(data):
    def apply(source):
        return source + data
    apply.__name__ = f"append {data!r}"
    return apply


# --- Programs -----------------------------------------------------------------

ECHO = MAIN_ONLY + b"""machine Main::main(&mut self) {
  let value: i32 = self.console.read_byte();
  self.console.write_byte(value);
  self.console.exit_process(value);
}
"""

SCALARS = MAIN_ONLY + b"""machine Main::main(&mut self) {
  let a: i32 = 6 * 7;
  let b: i32 = a - 10 / 3;
  let c: i32 = (b % 5) << 2;
  let d: i32 = (c >> 1) & 15;
  let e: i32 = d | 32;
  let f: i32 = e ^ 7;
  assert f == 47 && a == 42;
  self.console.write_byte(f);
  self.console.exit_process(f - 47);
}
"""

RECURSION = MAIN_ONLY + b"""machine Main::main(&mut self) {
  let total: i32 = nest(6);
  self.console.write_byte(total);
  self.console.exit_process(total);
}

machine nest(depth: i32) -> i32 {
  transition depth {
    0 -> return 0
    _ -> recurse()
  }
  state recurse() {
    let subtotal: i32 = nest(depth - 1);
    return subtotal + 2;
  }
}
"""

STATES_FIELDS = CONSOLE + b"""data Main {
  console: Console;
  count: i32;
  values: [u8; 2];
}

machine Main::main(&mut self) {
  transition 0 { 0 -> increment() }
  state increment() {
    self.count = self.count + 1;
    self.values[0] = self.values[0] + 1;
    transition self.count {
      3 -> done()
      _ -> again()
    }
  }
  state again() {
    self.values[1] = self.values[1] + 1;
    transition 0 { 0 -> increment() }
  }
  state done() {
    assert self.count == 3;
    assert self.values[0] == 3;
    assert self.values[1] == 2;
    self.console.write_byte(self.count + 62);
    return;
  }
}
"""

SUM_BINDERS = CONSOLE + b"""data Choice { case Full(value: i32); case Empty; }
data Main { console: Console; choice: Choice; }

machine Main::main(&mut self) {
  self.choice = Choice::Full(66);
  transition self.choice {
    Choice::Full{value} -> self.done(value)
    Choice::Empty -> return
  }
}

machine Main::done(&mut self, value: i32) {
  self.console.write_byte(value);
  self.console.exit_process(value - 66);
}
"""

RECORDS_NESTED = CONSOLE + b"""data Cell { value: i32; }
data Group { cell: Cell; cells: [Cell; 2]; }
data Main { console: Console; first: Cell; group: Group; }

machine Main::main(&mut self) {
  self.first.value = 65;
  self.group.cell.value = 66;
  self.group.cells[0].value = 67;
  self.group.cells[1].value = 68;
  self.group.cell = self.first;
  assert self.group.cell.value == 65;
  self.console.write_byte(self.group.cells[1].value);
  self.console.exit_process(self.group.cell.value - 65);
}
"""

DIV_ZERO = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_byte(66);
  let value: i32 = 1 / 0;
  self.console.write_byte(67);
}
"""

OVERFLOW = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_byte(66);
  let value: i32 = 2147483647 + 1;
  self.console.write_byte(67);
}
"""

SHIFT_COUNT = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_byte(66);
  let value: i32 = 1 << 32;
  self.console.write_byte(67);
}
"""

BYTE_RANGE = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_byte(66);
  self.console.write_byte(256);
  self.console.write_byte(67);
}
"""

BOUNDS_READ = CONSOLE + b"""data Main {
  console: Console;
  values: [i32; 4];
}

machine Main::main(&mut self) {
  self.console.write_byte(66);
  let value: i32 = self.values[9];
  self.console.write_byte(67);
}
"""

NONBOOLEAN = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_byte(66);
  assert 2;
  self.console.write_byte(67);
}
"""

ASSERTION = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_byte(66);
  assert 1 == 2;
  self.console.write_byte(67);
}
"""

NONEXHAUSTIVE = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_byte(66);
  transition 7 {
    1 -> return
  }
}
"""

DIV_OVERFLOW = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_byte(66);
  let value: i32 = -2147483648 / -1;
  self.console.write_byte(67);
}
"""

REM_ZERO = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_byte(66);
  let value: i32 = 5 % 0;
  self.console.write_byte(67);
}
"""

STRINGS_LINES = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_line("hi");
  self.console.write_byte('!');
  self.console.exit_process(5);
}
"""

ECHO_LOOP = MAIN_ONLY + b"""machine Main::main(&mut self) {
  transition 0 { 0 -> self.drain() }
}

machine Main::drain(&mut self) {
  let next: i32 = self.console.read_byte();
  transition next < 0 {
    1 -> return
    _ -> again(next)
  }
  state again(byte: i32) {
    self.console.write_byte(byte);
    transition 0 { 0 -> self.drain() }
  }
}
"""

PARAM_STATES = MAIN_ONLY + b"""machine Main::main(&mut self) {
  let result: i32 = self.advance(60);
  self.console.write_byte(result);
  self.console.exit_process(result - 61);
}

machine Main::advance(&mut self, value: i32) -> i32 {
  value = value + 1;
  transition 0 { 0 -> first(4) }
  state first(offset: i32) {
    value = value + offset;
    transition 0 { 0 -> second() }
  }
  state second() { return value; }
}
"""

VIEWS = CONSOLE + b"""data Main {
  console: Console;
  bytes: [u8; 4];
}

machine Main::main(&mut self) {
  self.bytes[0] = 104;
  self.bytes[1] = 105;
  self.bytes[2] = 33;
  self.bytes[3] = 0;
  let view: &[u8] = self.bytes[0..3];
  self.console.write_line(view);
  self.console.write_line(self.bytes.as_slice);
  self.console.exit_process(view.len);
}
"""

CONSTRUCTOR_ORDER = CONSOLE + b"""data Token {
  case Item(marker: i32, byte: u8, position: i32);
  case None;
}
data Main { console: Console; }

machine Main::main(&mut self) {
  let token: Token = Token::Item(self.mark(), self.byte(), self.pos());
  transition token {
    Token::Item{m, b, p} -> self.report(m + b + p)
    Token::None -> return
  }
}

machine Main::report(&mut self, total: i32) {
  self.console.write_byte(total);
}

machine Main::mark(&mut self) -> i32 {
  self.console.write_byte(65);
  return 5;
}

machine Main::byte(&mut self) -> i32 {
  self.console.write_byte(66);
  return 256;
}

machine Main::pos(&mut self) -> i32 {
  self.console.write_byte(67);
  return 9;
}
"""

NEGATIVE_EXIT = MAIN_ONLY + b"""machine Main::main(&mut self) {
  self.console.write_byte(65);
  self.console.exit_process(-1);
}
"""

# --- Corpus table --------------------------------------------------------------

# Each mutation is applied to the source; the model derives the mutated
# program's observation. Stdin mutations replace the sealed stdin section.

CASES = [
    ("echo", ECHO, b"Z",
     [swap(b"exit_process(value)", b"exit_process(value + 1)"),
      swap(b"write_byte(value)", b"write_byte(value + 2)")],
     [b"Y", b"", b"\x00", b"\xff"]),
    ("scalars", SCALARS, b"",
     [swap(b"6 * 7", b"6 * 8"),
      swap(b"10 / 3", b"10 / 4"),
      swap(b"% 5", b"% 6"),
      swap(b"<< 2", b"<< 3"),
      swap(b">> 1", b">> 2"),
      swap(b"| 32", b"| 64"),
      swap(b"^ 7", b"^ 9"),
      swap(b"f == 47", b"f == 46"),
      swap(b"&&", b"||"),
      # The one admitted magnitude under unary minus: a = -2147483648,
      # then a - 3 traps Overflow.
      swap(b"6 * 7", b"-2147483648")],
     []),
    ("recursion", RECURSION, b"",
     [swap(b"nest(6)", b"nest(4)"),
      swap(b"subtotal + 2", b"subtotal + 3"),
      swap(b"depth - 1", b"depth - 2"),
      swap(b"0 -> return 0", b"0 -> return 1"),
      swap(b"let subtotal: i32 = nest", b"let subtotal: i32 = mist")],
     []),
    ("states_fields", STATES_FIELDS, b"",
     [swap(b"3 -> done()", b"4 -> done()"),
      swap(b"self.count + 62", b"self.count + 63"),
      swap(b"self.values[1] == 2", b"self.values[1] == 3"),
      swap(b"self.count == 3", b"self.count == 2")],
     []),
    ("sum_binders", SUM_BINDERS, b"",
     [swap(b"Choice::Full(66)", b"Choice::Full(67)"),
      swap(b"Choice::Full(66)", b"Choice::Empty"),
      swap(b"Choice::Full{value} -> self.done(value)",
          b"Choice::Full{item} -> self.done(item)"),
      swap(b"    Choice::Empty -> return\n", b""),
      swap(b"Choice::Full{value}", b"Choice::Full{value, extra}")],
     []),
    ("records_nested", RECORDS_NESTED, b"",
     [swap(b"self.group.cells[1].value = 68", b"self.group.cells[0].value = 68"),
      swap(b"self.group.cell.value = 66", b"self.group.cell.value = 69"),
      swap(b"cells[1].value);", b"cells[0].value);"),
      swap(b"self.group.cell = self.first",
           b"self.group.cell = self.group.cells[1]")],
     []),
    ("div_zero", DIV_ZERO, b"",
     [swap(b"1 / 0", b"4 / 2"),
      swap(b"1 / 0", b"1 % 0")],
     []),
    ("overflow", OVERFLOW, b"",
     [swap(b"2147483647 + 1", b"2147483647 + 0"),
      swap(b"2147483647 + 1", b"-2147483648 - 1")],
     []),
    ("shift_count", SHIFT_COUNT, b"",
     [swap(b"1 << 32", b"1 << 31"),
      swap(b"1 << 32", b"1 >> 32"),
      swap(b"1 << 32", b"1 << -1")],
     []),
    ("byte_range", BYTE_RANGE, b"",
     [swap(b"write_byte(256)", b"write_byte(255)"),
      swap(b"write_byte(256)", b"write_byte(-1)")],
     []),
    ("bounds_read", BOUNDS_READ, b"",
     [swap(b"self.values[9]", b"self.values[3]"),
      swap(b"self.values[9]", b"self.values[4]"),
      swap(b"self.values[9]", b"self.values[-1]")],
     []),
    ("nonboolean", NONBOOLEAN, b"",
     [swap(b"assert 2", b"assert 1"),
      swap(b"assert 2", b"assert -1")],
     []),
    ("assertion", ASSERTION, b"",
     [swap(b"assert 1 == 2", b"assert 1 == 1"),
      swap(b"assert 1 == 2", b"assert 0")],
     []),
    ("nonexhaustive", NONEXHAUSTIVE, b"",
     [swap(b"transition 7", b"transition 1"),
      swap(b"1 -> return", b"7 -> return"),
      swap(b"1 -> return", b"1 -> return\n    _ -> return")],
     []),
    ("div_overflow", DIV_OVERFLOW, b"",
     [swap(b"-2147483648 / -1", b"-2147483647 / -1"),
      swap(b"-2147483648 / -1", b"-2147483648 % -1"),
      swap(b"-2147483648 / -1", b"-2147483648 / 1")],
     []),
    ("rem_zero", REM_ZERO, b"",
     [swap(b"5 % 0", b"5 % 2"),
      swap(b"5 % 0", b"-2147483648 % -1")],
     []),
    ("strings_lines", STRINGS_LINES, b"",
     [swap(b'"hi"', b'"yo"'),
      swap(b"'!'", b"'?'"),
      swap(b'"hi"', b'"h\\ni"'),
      swap(b"exit_process(5)", b"exit_process(6)")],
     []),
    ("echo_loop", ECHO_LOOP, b"ab",
     [swap(b"self.console.write_byte(byte)",
           b"self.console.write_byte(byte + 1)")],
     [b"", b"a", b"\x00\xff", b"xyzzy"]),
    ("param_states", PARAM_STATES, b"",
     [swap(b"self.advance(60)", b"self.advance(58)"),
      swap(b"first(4)", b"first(2)"),
      swap(b"value = value + 1", b"value = value + 10"),
      swap(b"return value", b"return value - 2")],
     []),
    ("views", VIEWS, b"",
     [swap(b"self.bytes[0..3]", b"self.bytes[1..3]"),
      swap(b"self.bytes[0..3]", b"self.bytes[0..4]"),
      swap(b"self.bytes[0..3]", b"self.bytes[0..5]"),
      swap(b"exit_process(view.len)", b"exit_process(view.len + 1)")],
     []),
    ("constructor_order", CONSTRUCTOR_ORDER, b"",
     [swap(b"return 256;", b"return 255;"),
      swap(b"Token::Item{m, b, p} -> self.report(m + b + p)",
           b"Token::Item{m, b, p} -> self.report(m)")],
     []),
    ("negative_exit", NEGATIVE_EXIT, b"",
     [swap(b"exit_process(-1)", b"exit_process(-2)"),
      swap(b"exit_process(-1)", b"exit_process(0)")],
     []),
]

# Rejection mutations: each maps a well-formed program to one the model
# rejects with an exact closed reason and coordinate. These are checking-
# phase refusals; the evaluator must publish the same Reject observation.
REJECTIONS = [
    ("invalid source byte", ECHO, swap(b"let value", b"let\x01value")),
    ("invalid token", ECHO,
     swap(b"self.console.read_byte()", b"self.console.read_byte()?")),
    ("unterminated string", STRINGS_LINES,
     swap(b'self.console.write_line("hi");',
          b'self.console.write_line("hi);')),
    ("invalid escape", STRINGS_LINES,
     swap(b'"hi"', b'"h\\qi"')),
    ("integer out of range", SCALARS, swap(b"6 * 7", b"2147483648")),
    ("integer out of range past the unary-minus magnitude", SCALARS,
     swap(b"6 * 7", b"2147483649")),
    ("unexpected end", ECHO, drop_last(20)),
    ("unexpected token after wildcard", NONEXHAUSTIVE,
     swap(b"1 -> return", b"_ -> return\n    1 -> return")),
    ("unknown name", ECHO,
     swap(b"console.exit_process", b"console.exit_procesz")),
    ("duplicate pattern", NONEXHAUSTIVE,
     swap(b"1 -> return", b"1 -> return\n    1 -> return")),
    ("missing entry", ECHO,
     swap(b"machine Main::main", b"machine Main::mains")),
    ("arity mismatch", RECURSION, swap(b"nest(6)", b"nest(6, 6)")),
    ("duplicate local", SCALARS,
     swap(b"let b: i32", b"let a: i32")),
    # Block-exit relations: a `return;` is ReturnNone and is admitted only
    # by a resultless machine, anchoring TypeMismatch at `return`; an
    # authored value is ReturnValue under strict type equality, anchoring
    # TypeMismatch at the expression; an absent terminal is Falloff and
    # anchors at the body's closing `}`; a resultless machine call used as
    # a continuation becomes Falloff on return; an executable construct
    # after a `never` statement anchors InvalidTerminal at its own `;`.
    ("return without a value in a value machine", RECURSION,
     swap(b"return subtotal + 2;", b"return;")),
    ("return value in a resultless machine", ECHO,
     swap(b"self.console.exit_process(value);", b"return value;")),
    ("u8 let initializer is strict equality", RECURSION,
     chain(swap(b"let subtotal: i32 = nest(depth - 1);",
                b"let subtotal: i32 = nest(depth - 1);\n    let low: u8 = 1;"),
           swap(b"return subtotal + 2;", b"return low;"))),
    ("falloff in a value machine", RECURSION,
     swap(b"return subtotal + 2;", b"")),
    ("resultless continuation in a value machine", RECURSION,
     chain(swap(b"_ -> recurse()", b"_ -> drain()"),
           append(b"\nmachine drain() {\n  return;\n}\n"))),
    ("statement after a never call", ECHO,
     swap(b"self.console.exit_process(value);",
          b"self.console.exit_process(value);\n  self.console.write_byte(0);")),
    # Type formation by placement: `u8` is storage-only, so a `let`,
    # parameter, or return `u8` is TypeMismatch at the type token; `never`
    # admits only the return placement; `Console` names no declared type
    # (InvalidEntry); an unknown owner is UnknownType; a zero array length
    # anchors InvalidArrayLength at the length literal; a stored or nested
    # view is EscapingView.
    ("u8 parameter", RECURSION, swap(b"depth: i32", b"depth: u8")),
    ("never local", SCALARS, swap(b"let a: i32", b"let a: never")),
    ("u8 return type", RECURSION,
     swap(b"machine nest(depth: i32) -> i32",
          b"machine nest(depth: i32) -> u8")),
    ("console as a local type", SCALARS,
     swap(b"let a: i32", b"let a: Console")),
    ("unknown type", SCALARS, swap(b"let a: i32", b"let a: Bogus")),
    ("zero array length", STATES_FIELDS,
     swap(b"values: [u8; 2]", b"values: [u8; 0]")),
    ("view field escapes", VIEWS,
     swap(b"bytes: [u8; 4];", b"bytes: &[u8];")),
]

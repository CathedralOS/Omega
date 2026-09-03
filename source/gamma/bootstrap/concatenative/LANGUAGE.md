# Gamma language

Gamma is a bounded concatenative compiler machine. Its only customers are the
Delta compiler and named bootstrap tools. It exposes an explicit value stack,
fixed cells, sealed input, append-only output, ordinary word calls, and tail
control-flow transfers. General-purpose language features have no standing.

## Source form

Source bytes are HT, LF, CR, and printable ASCII. Every other byte rejects
before tokenization at its exact byte offset. Space, tab, CR, and LF separate
tokens. `#` begins a comment through the next CR, LF, or source end.

Names match `[a-z_][a-z0-9_]*`. A hexadecimal word is `0x` followed by one to
sixteen lowercase hexadecimal digits. It denotes one 64-bit bit pattern.
Leading zeroes are permitted. Decimal words, uppercase digits, bare `0x`, and
wider words are not literals.

```text
program     := definition+
definition  := ':' NAME TOKEN* ';'
HEXWORD     := '0x' [0-9a-f]{1,16}
NAME        := [a-z_] [a-z0-9_]*
```

`:` and `;` must be separate tokens. Definitions are mutually visible and
unique. A source name may not equal a builtin. Exactly one definition named
`main` is required. The complete definition census occurs before execution;
body tokens are otherwise opaque until reached. A reached token is classified
as a `HEXWORD`, builtin, or exact user-word name. Every other reached token is
an authored trap. `jump` consumes one following token as its target name;
`branch` consumes two. A missing target token is malformed source.

## Machine state

Gamma execution has:

- an immutable sealed input byte sequence;
- an append-only output byte sequence;
- a stack of 64-bit words;
- zero-initialized fixed 64-bit cells addressed by nonnegative index;
- a bounded stack of ordinary word continuations; and
- the current word and source-token position.

Returning from `main` completes successfully. Returning from any other word
resumes immediately after its call token. `jump target` transfers to `target`
without retaining the current word. `branch yes no` pops one condition,
consumes both target-name tokens, and tail-transfers to `yes` when nonzero or
`no` when zero. Thus source CFG loops use constant continuation storage.

An ordinary user name calls that word and retains one continuation. An unknown
reached name, stack underflow, invalid cell/input access, invalid output byte,
failed assertion, or selected unknown transfer target is an authored trap.
An unselected branch target has no runtime name-resolution effect.

## Builtins

The stack notation `( before -- after )` writes the top at the right.

| Builtin | Stack effect | Meaning |
| --- | --- | --- |
| `input-length` | `( -- n )` | Push sealed input length. |
| `input-get` | `( index -- byte )` | Push the indexed input byte; bounds-check first. |
| `output-byte` | `( value -- )` | Append one byte, requiring `0 <= value <= 255`. |
| `output-word` | `( value -- )` | Append eight little-endian bytes. |
| `output-position` | `( -- n )` | Push current output length. |
| `assert-equal` | `( left right -- )` | Trap unless both words are identical. |
| `cell-get` | `( index -- value )` | Load one checked cell. |
| `cell-set` | `( value index -- )` | Store one checked cell. |
| `dup` | `( a -- a a )` | Duplicate top. |
| `swap` | `( a b -- b a )` | Exchange top two. |
| `over` | `( a b -- a b a )` | Copy the second word. |
| `drop` | `( a -- )` | Discard top. |
| `+` | `( a b -- a+b )` | Wrapping addition modulo $2^{64}$. |
| `-` | `( a b -- a-b )` | Wrapping subtraction modulo $2^{64}$. |
| `*` | `( a b -- a*b )` | Wrapping multiplication modulo $2^{64}$. |
| `/` | `( a b -- a/b )` | Signed division truncating toward zero. |
| `%` | `( a b -- a%b )` | Signed remainder matching `/`. |
| `<` | `( a b -- flag )` | Signed comparison, producing zero or one. |
| `=` | `( a b -- flag )` | Bit-pattern equality, producing zero or one. |

Division and remainder trap for a zero divisor and `INT64_MIN / -1`.
Arithmetic otherwise has Alpha's exact two's-complement meaning.

## Boundaries and exclusions

[`EVALUATOR_PROFILE.md`](EVALUATOR_PROFILE.md) fixes request framing, capacities,
terminal statuses, publication, and private representation. Capacity exhaustion
produces `Incomplete`; it is not a Gamma value, authored trap, or source
rejection. A program may diverge through actual infinite control transfer.

Gamma has no strings, local names, lexical environments, pairs, algebraic data,
pattern matching, function parameters, function values, closures, heap,
garbage collector, mutation outside fixed cells, raw addresses, computed jumps,
exceptions, modules, packages, concurrency, ambient input/output, or host calls.
A builtin is admitted only when a named compiler/checker customer lowers the
complete audited chain cost.
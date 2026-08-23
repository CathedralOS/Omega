# Beta — small-step operational semantics

This document is the written meaning of the Beta language used by the bootstrap
lattice. [`LANGUAGE.md`](LANGUAGE.md) owns the source grammar and rung boundary;
this document fixes evaluation order, state transitions, memory, I/O, and
terminal observations. It formalizes the existing compiler/interpreter contract
and does not add language features.

`reference/beta_interp.py` is an untrusted executable regression oracle. Its
sparse memory and finite step cap are implementation conveniences, so it is not
the authority for finite-memory exhaustion or divergence. The whole-compiler
correspondence edge reconstructs obligations from this document instead.

## 1. Values and program formation

The only runtime value is a 64-bit word `Word = 0 .. 2^64-1`. Interpret a word
as signed two's-complement only for signed comparisons and division/remainder.
All source identifiers in a well-formed program resolve within their procedure;
procedure names and procedure-scoped state names are unique; calls have at most
four arguments and match the callee's parameter count.

A parsed procedure contains:

1. an ordered parameter list;
2. an entry block containing statements before its first `state`;
3. ordered, procedure-scoped state blocks; and
4. the source-order fallthrough edge from each block to the next block.

The fixed compiler subject `bc.beta` is well formed. Handling arbitrary bytes
fed *to* that compiler is ordinary Beta byte-I/O behavior; the quantified input
stream is not reparsed as the definition of Beta itself.

## 2. Configurations

For a parsed program `P` and resource profile `B`, a configuration is:

```text
BetaConfig = {
  control: EvalExpr(e, k) | ExecBlock(proc, block, stmt, k) | Returning(v, k),
  calls: sequence<Continuation>,
  frame: map<LocalName, Word>,
  memory: byte array of B.memory_bytes,
  input: sequence<Byte>,
  input_cursor: Nat,
  output: finite sequence<Byte>,
  resources: counters fixed by B
}
```

Memory begins as all zero. A procedure call creates a fresh frame, binds
parameters left-to-right to the already evaluated argument words, and leaves no
other locals initialized. `let` creates its function-scoped local; assignment
updates the resolved local. A well-formed program never reads an uninitialized
or unresolved local.

`B_bc1` is fixed in [`BOOTSTRAP_OBSERVABLE.md`](BOOTSTRAP_OBSERVABLE.md). A
resource admission that would exceed a declared checked ceiling transitions to
`Exhaust(kind, limit, requested)` before the overlapping write or recursive
activation. Out-of-range raw memory remains a stuck configuration until Alpha's
corresponding edge is hardened or an independent proof shows it unreachable;
`B_bc1` requires the latter proof for `bc.beta`.

## 3. Expression order and arithmetic

Expressions evaluate left-to-right. Call arguments evaluate left-to-right
before the callee begins. Memory-store addresses evaluate before their values.
The binary rules are:

```text
a + b = (a + b) mod 2^64
a - b = (a - b) mod 2^64
a * b = (a * b) mod 2^64
```

`/` is signed division truncated toward zero. `%` is the matching signed
remainder `a - (a / b) * b`. Division and remainder transition to
`Trap(DivisionByZero)` when the divisor is zero and to
`Trap(SignedDivisionOverflow)` for `INT64_MIN / -1`.

`<`, `>`, `<=`, and `>=` compare signed words. `==` and `!=` compare all 64
bits. Every comparison produces exactly word `0` or word `1`.

Parentheses have no runtime effect beyond determining the parsed expression.
Character literals are their decoded byte values.

## 4. Memory

For an admitted address `a`:

```text
byte[a]       reads memory[a] and zero-extends it
byte[a] = v   writes v mod 2^8 to memory[a]
word[a]       reads memory[a .. a+8] as a little-endian Word
word[a] = v   writes v as eight little-endian bytes
```

Byte and word accesses alias through the same byte array. Address arithmetic is
ordinary wrapping Word arithmetic performed before the access. An independent
bounds obligation must establish `a < B.memory_bytes` for a byte and
`a <= B.memory_bytes - 8` for a word before transferring the run to Alpha.

## 5. Statements and control flow

Statements step in source order within the current block:

- `let x = e` evaluates `e`, binds `x`, then advances;
- `x = e` evaluates `e`, updates `x`, then advances;
- a byte/word store evaluates address then value, performs the store, then
  advances;
- a call statement evaluates the call, discards its result, then advances;
- `emit("...")` appends the decoded literal bytes in order, then advances;
- `return e` evaluates `e` and returns it immediately;
- `to S` selects state `S` immediately;
- `to S when e` evaluates `e`; a nonzero result selects `S`, while zero advances
  to the following statement.

Reaching the end of a block falls through to the next state block in source
order. Falling past the final block returns word zero. A selected state belongs
to the current procedure; state selection never crosses a procedure boundary.

## 6. Calls and host boundary

An ordinary call evaluates its arguments left-to-right, creates the callee frame,
executes the callee's entry block, and yields the returned word to its caller.
Recursive calls use the same rule and consume the explicit call resource in
`B`.

The two intrinsic calls are:

```text
read_byte()
  if input_cursor < input.length:
      consume and return the next byte, zero-extended
  else:
      return 0xFFFFFFFFFFFFFFFF without advancing

write_byte(v)
  append v mod 2^8 to output
  return v
```

The return value of `write_byte` matches Alpha lowering: the argument remains in
`r0` after `write`.

`emit` is syntax sugar for a fixed sequence of byte writes. Its escapes are
exactly `\n`, `\t`, `\r`, `\0`, `\\`, and `\"`.

## 7. Maximal observations

Evaluation produces the maximal ordered output trace and exactly one terminal
classification:

```text
BetaObservation = {
  stdout: finite or infinite sequence<Byte>,
  terminal: Halt(u32) | Trap(TrapKind)
          | Exhaust(ResourceKind, limit, requested) | Diverge
}
```

Returning from `main` yields `Halt(result mod 2^32)`. A Unix process wrapper may
expose only the low byte, but that projection is not the language observation.
A trap or checked exhaustion retains every byte emitted before it. `Diverge`
means an infinite small-step run; a fuel limit or wall-clock timeout cannot be
relabelled as a trap.

The `bc.beta` theorem required by `BOOTSTRAP_OBSERVABLE.md` compares this maximal
Beta observation with the exact Alpha tape observation for every finite input
stream admitted by `B_bc1`.

## 8. Executable evidence

- `reference/beta_interp.py` exercises the rules over finite test runs;
- `reference/beta-correctness-fuzz.sh` compares interpreted and compiled runs;
- `reference/beta-io-exhaust.sh` exhausts all 256 single-byte inputs for its
  admitted programs;
- `source-exhaustion.sh` pins `B_bc1` resource boundaries;
- `bootstrap/assurance/refinement/beta/bc-artifact-structure.sh` checks the
  exact artifact's reachable instruction framing and direct targets below `bc`.
- `bootstrap/assurance/refinement/beta/bc-block-control.sh` checks exact source
  control successors and static custody for every call/return/I/O/emit site,
  including the fixed-literal output macro, source-derived frame allocations,
  parameter stores, callee arities, and pre-call argument pops, in one Alpha
  process.

These gates are evidence. Whole-compiler closure still requires a checked
forward simulation from these source transitions to Alpha small steps, including
memory bounds, call/return discipline, complete output traces, traps,
exhaustion, and cyclic/divergent control.

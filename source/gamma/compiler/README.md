# Gamma compiler owner

The canonical compiler owned here accepts Gamma, is implemented in Beta, and
emits platform-independent Alpha tape:

```text
gamma_compiler.beta → gamma_compiler_bytecode.tape
```

The source now exists as an incomplete implementation; the tape does not. Its
retained frontend is the former standalone checker, moved rather than copied,
and its direct Alpha payload/label/fixup substrate is final compiler material.
The adjacent gate compiles this one source with temporary fixed test entries,
runs all 78 frontend discriminators, checks exact emitter bytes plus sticky
capacity/fixup failures, executes six generated runtime-containment programs,
exercises 16 checked-`Int` paths, and runs 12 source-to-code lowering cases. It
publishes no compiler artifact.

The retained compiler source declares 84 procedures. With the fixed frontend
gate entry, the compiled gate uses 85 of Beta's 128 procedure slots and compiles
to 165,571 bytes. The remaining 96,569 bytes under
Alpha's runnable payload ceiling are a measured implementation budget, not a
Gamma language limit.

`../interp.beta` remains a bounded execution oracle. It may contribute an
isolated lowering/runtime algorithm where economical, but no interpreter loop,
serialized Gamma AST, or second frontend enters the canonical compiler.

The compiler uses a private arbitrary-arity frame ABI and preserves proper tail
calls. Its emitted compiler-application adapter alone supplies sealed input as
Gamma `Bytes`, invokes the typed `main`, and serializes exact success or the
accepted-language edge's failure frame. Fuel and private storage ceilings yield
outer resource outcomes; they never change Gamma meaning.

## Implementation shape

The compiler source is growing one pipeline inside `gamma_compiler.beta`:

```text
sealed source
  -> strict parse and declaration collection
  -> type resolution and checked typed IR
  -> direct Alpha lowering and fixups
  -> complete private payload validation
  -> one publication
```

The compile-time AST/IR is never serialized into the emitted program. No Gamma
evaluator loop, syntax-tag dispatcher, textual Alpha stream, or host-side
assembler participates in the edge.

The Beta executable has exactly 32 MiB of source-visible logical raw memory;
physical memory above it is Alpha's hidden-return-stack allowance. The frontend
keeps sealed source in `[2 MiB,6 MiB)`, declaration and environment tables in
`[6 MiB,10.5 MiB)`, labels/fixups in `[10.5 MiB,11.5 MiB)`, and AST storage in
`[16 MiB,31.75 MiB)`. The final `[31.75 MiB,32 MiB)` is the private 262,144-byte
payload buffer; the runnable Alpha payload limit is 262,140 bytes. Source,
table, arena, fixup, and payload writes are checked before mutation. No output
byte is published until every fixup and the complete payload extent validate.

The retained front end's four-word syntax nodes retain the exact
zero-based source start in the high bits of their tag word; the 4 MiB source
ceiling and closed tags make that packing exact without reducing AST capacity.
Its first source failure is sticky across the outer byte envelope, parsing,
literal checking, type-name resolution, and typed subexpression traversal.
This is coordinate custody for later absorption, not an oracle-owned diagnostic
format: the final compiler maps it through `GCOUT`'s fixed rejection table and
publication boundary. The Q2-selected generated-program application profile is
a separate concern.

An emitted Gamma program uses this Alpha-memory profile:

```text
[0, 256 KiB)       canonical Alpha tape
[256 KiB, 16 MiB)  downward Gamma activation/argument stack
[16 MiB, 48 MiB)   upward immutable value/Bytes heap
[48 MiB, 64 MiB)   Alpha hidden-return-stack allowance
```

Every private limit is an outer resource profile, not a Gamma validity rule.
The compiler measures the minimum generated frame and helper nesting needed to
keep the explicit stack, heap, tape, and hidden return stack disjoint.

Gamma values use two words `(kind, payload)`: `Int` carries all signed 64 value
bits in `payload`; `Bytes` points to an immutable descriptor; and an algebraic
value uses its resolved constructor tag plus a pointer to two-word fields.
Nullary constructors allocate no field vector. Functions return through
`r0/r1`. Arbitrary-arity arguments occupy two-word slots in the explicit Gamma
stack rather than Beta's four argument registers.

Generated code reserves `r252` for the downward stack pointer, `r253` for the
current frame base, `r254` for the upward heap pointer, and `r255` for the heap
limit. The runtime initializer fixes their canonical endpoints. Directly
emitted heap and stack reservation helpers reject negative, overflowed, and
adjacent-out-of-range requests before mutation and transfer to one
caller-supplied terminal failure label. The adjacent gate executes the emitted
Alpha payload at both exact boundaries and one byte beyond each, and separately
checks negative and arithmetic-wrap requests; no helper relies on Alpha's
undefined out-of-range memory behavior.

Directly emitted signed-add, subtract, multiply, divide, and remainder helpers
use a private scalar ABI through `r0` and transfer every arithmetic overflow,
zero divisor, and `INT64_MIN / -1` case to the supplied terminal failure label
before Alpha can trap. General lowering moves that scalar into the `Int` payload
in `r1` and restores kind `0` in `r0`. Their executed probe covers ordinary
negative division/remainder, both
addition and subtraction overflow directions, multiplication overflow and the
valid `INT64_MIN * 1` edge, and both exceptional division/remainder classes.

The first retained lowering slice consumes already checked closed `Int` trees
with literals and all seven primitive operators. It emits nested evaluation
left-to-right, spills intermediates through the guarded explicit stack, calls
the checked helpers, and reconstructs `(kind,payload)` results. Its adjacent
gate feeds real Gamma declarations through the canonical parser and type
checker, executes the emitted Alpha tapes for ordinary/nested arithmetic and
both comparison results, pins balanced stack restoration, and observes
contained overflow. This is pipeline material for general expression lowering;
no partial Gamma compiler or subset artifact is published.

Ordinary calls use Alpha `call`/`ret`. A tail call first evaluates arguments
exactly once from left to right into temporary stack slots, relocates them
overlap-safely into the replacement frame, restores the original caller frame,
and jumps to the callee. Tail-position `let`, `if`, and every selected match arm
propagate this transfer, so terminating tail recursion grows neither Gamma
activations nor Alpha's hidden return stack.

`Bytes` uses a compact immutable rope/view representation with closed descriptor
kinds `EMPTY`, `LEAF(pointer,length)`, `CONCAT(left,right,total_length)`, and
`SLICE(base,start,length)`. Concatenation is constant-time after checked length
addition; slicing validates the complete signed range; indexed access descends
iteratively. An application adapter preflights an entire returned rope and
output extent before replaying it to stdout, preventing partial artifacts.

The direct emitter owns byte/word append, label definition, and
`{payload_offset,label_id}` fixup rows. Branch and call targets remain private
placeholders until all code exists; duplicate or missing labels and out-of-range
targets are sticky internal failures. Final lowering/refinement must add replay
validation; this substrate does not yet claim it. Checked `Int` lowering
branches explicitly around overflow, division-by-zero, signed-division-overflow,
and invalid byte/range operations so required diagnostic publication never
depends on falling into an uncatchable Alpha trap.

Q3 blocks the final declaration/binder resolver policy. Q2 blocks generated
application-profile selection and therefore adapter publication/final tape.
Neither question authorizes a subset compiler or blocks the strict parser,
private target ABI, runtime helpers, direct emitter, or profile-independent
lowering described above.

Any future validation placed here must reconstruct the exact
Beta-source-to-Alpha-tape edge for `gamma_compiler.beta`. Generic evidence,
external interpreter execution, and host-side source lowering do not belong in
this owner.

The implementation order is tracked in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained file | Canonical role | Deletion condition |
| --- | --- | --- |
| `gamma_compiler.beta` | Sole Beta-written Gamma compiler source; currently owns the strict frontend and direct Alpha payload/fixup substrate. | Replace only atomically with another implementation of the same ruled edge. |
| `test-frontend.sh` | Adjacent bounded gate for the retained frontend, source/resource guards, exact emitter substrate, and executed runtime-containment payloads. | Delete or reduce when exact source-to-tape validation subsumes every named discriminator. |

## Deletion condition

Delete any future file or child subtree that does not reconstruct, implement,
or efficiently test `gamma_compiler.beta → gamma_compiler_bytecode.tape`;
replace this owner only atomically with a changed, explicitly ruled topology.

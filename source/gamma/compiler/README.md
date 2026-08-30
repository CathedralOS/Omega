# Gamma compiler owner

The canonical compiler owned here accepts Gamma, is implemented in Beta, and
emits platform-independent Alpha tape:

```text
gamma_compiler.beta → gamma_compiler_bytecode.tape
```

The source and artifact do not yet exist. Their language-level contract is
settled in [`../LANGUAGE.md`](../LANGUAGE.md); this owner is now an implementation
gap rather than a design-blocked placeholder. No interpreter-shaped artifact is
accepted in the meantime.

`../interp.beta` and `../typeck.beta` are retained at the Gamma language owner
as bounded semantic oracles and candidate implementation components. The
eventual compiler may reorganize or absorb them, but must type-check Gamma and
emit Alpha tape directly rather than publishing an interpreter plus source AST.
Delete either component when the compiler subsumes its unique failure
detection, or if it cannot be economically adapted to the contract.

The compiler uses a private arbitrary-arity frame ABI and preserves proper tail
calls. Its emitted compiler-application adapter alone supplies sealed input as
Gamma `Bytes`, invokes the typed `main`, and serializes exact success or the
accepted-language edge's failure frame. Fuel and private storage ceilings yield
outer resource outcomes; they never change Gamma meaning.

## Implementation shape

The compiler source has one pipeline inside the eventual
`gamma_compiler.beta`:

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
physical memory above it is Alpha's hidden-return-stack allowance. The reusable
front end already keeps sealed source in `[2 MiB,6 MiB)`, declaration and
environment tables in `[6 MiB,10.5 MiB)`, and AST storage above 16 MiB. When it
is absorbed, the final compiler reserves `[31.75 MiB,32 MiB)` as its private
262,144-byte payload buffer and ends every upward-growing frontend arena before
that buffer. It checks source, table, arena, fixup, and payload bounds before
mutation. No output byte is written until every fixup and the complete payload
extent have validated.

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
placeholders until all code exists; duplicate or missing labels, out-of-range
targets, and replay disagreement are internal failures. Checked `Int` lowering
branches explicitly around overflow, division-by-zero, signed-division-overflow,
and invalid byte/range operations so required diagnostic publication never
depends on falling into an uncatchable Alpha trap.

Q6 blocks the final declaration/binder resolver policy. Q5 blocks generated
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

## Deletion condition

This currently empty implementation owner is retained because its exact path is
part of the canonical lattice contract. Delete any future child subtree that
does not reconstruct, implement, or test
`gamma_compiler.beta → gamma_compiler_bytecode.tape`; replace the owner only
atomically with a changed, explicitly ruled lattice topology.

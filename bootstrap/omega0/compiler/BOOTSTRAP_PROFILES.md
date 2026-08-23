# Bootstrap compiler profiles (transitional `omega0` path)

This file freezes the first executable contracts for the Delta-built bridge
compiler. The directory and O0/O1 artifact names are transitional implementation
names; the architectural role is `omega-bootstrap`. This file is
bootstrap-owned and is not a product Omega language specification.

Four contracts are intentionally tracked separately:

- **Delta implementation profile D0** is the current Delta surface used by the
  bridge canaries. It is frozen so those slices do not acquire facilities merely
  because the Rust on-ramp has them. It is not the final Delta specification.
- **Omega canary acceptance profile O0** is the first input the bridge must accept.
  It is frozen now as a vertical proof of the pipeline, not falsely presented as
  sufficient to express the future production compiler.
- **Omega variable acceptance profile O1** is the first table-driven source
  slice. It is a monotonic extension of O0, frozen at explicit statement and
  storage ceilings below.
- **Omega self-hosting profile `Ωself`** is the ordinary-Omega source closure
  from which the production compiler is built. It remains open until that exact
  source and dependency manifest exists.

O0 and O1 are vertical pipeline canaries, not normative ancestors of `Ωself`.
The eventual profile may reuse their implementation, but it is derived from the
production source closure rather than declared to be the next numbered canary.
Sufficiency cannot be established from the current Rust product implementation.

O1 generalizes the O0 body to a bounded sequence of zero or more literal
`write_line` statements followed by exactly one literal `exit_process`. One
table-driven frontend, terminal emitter, and direct backend handle every
admitted statement count. This is still far smaller than the eventual `Ωself`
profile.

## D0 — current Delta implementation profile

The current bridge compiler slices may use only the following already
implemented, self-hosted Delta surface:

- `boundary trait`, `data`, free `machine`, and `Type::machine(&mut self, ...)`
  declarations;
- `i32` scalars; fixed `[i32; N]` and `[u8; N]` backing fields; zero-initialized
  receiver storage;
- locals, mutable locals and fields, checked indexed field access, and integer
  offsets as durable handles;
- `+ - * / %`, comparisons, bit operations, shifts, `min`/`max`, and
  parenthesized expressions with trapping arithmetic unless an explicit domain
  says otherwise;
- state-machine control flow, state parameters, calls/recursion, returns, and
  tag/payload `case` data where useful;
- `read_byte`, `write_byte`, `write_line`, and `exit_process` as the explicit
  compiler boundary;
- the fixed-backing allocation convention below.

The calling-profile limits are at most four value parameters for a free machine
and at most three value parameters for a self method. A D0 compiler slice must
use checked failure before exceeding any source, table, arena, call, or target
offset bound. It must not discard input or declarations to remain within a
bound.

The following are outside D0: host pointers, ambient heap allocation, individual
`free`, garbage collection, dynamic locals, threads, atomics, modules, a
production `PagedArena`, and optimization-specific data structures. Source is a
single deterministic byte stream. Multi-file input will be supplied by a
separately audited bundling contract until native packages are implemented. The
version-1 contract is frozen in [`OMEGA0_BUNDLE.md`](OMEGA0_BUNDLE.md).

### Fixed-backing allocation convention

A backing array is provisioned as zeroed storage by the executable loader.
Logical allocations have runtime sizes and return integer offsets into that
backing:

```text
reserve(count, alignment)
  valid arguments require count >= 0 and alignment > 0
  for valid arguments, pad = (alignment - (cursor % alignment)) % alignment

  success, when pad <= capacity - cursor and
                count <= capacity - (cursor + pad):
    base = cursor + pad
    cursor = base + count
    ok = 1

  failure, including invalid arguments or exhaustion:
    cursor and backing are unchanged
    ok = 0

reset(mark)
  success only when 0 <= mark <= cursor; set cursor = mark
  otherwise leave cursor unchanged and report failure
```

Zero-sized allocations succeed at the aligned cursor. Successful allocations
are deterministic, monotonic, aligned, and disjoint until a valid bulk reset.
Handles are indices, not host addresses. The executable canary is
[`../../rungs/delta/samples/bootstrap-storage.alp`](../../rungs/delta/samples/bootstrap-storage.alp).
The Delta-written `lowermachine.alp` now applies the same convention at compiler
scale: one explicitly reserved typed extent is partitioned into integer-offset
logical tables, while source bytes reserve contiguous cells at runtime in a
separate byte backing. Checked exhaustion cannot compile a retained prefix.

## `Ωself` — production compiler source profile (open)

`Ωself` is a strict profile of valid Omega source, not a bootstrap dialect or a
new rung. `omega-bootstrap` will accept exactly this profile; the compiler built
from it must nevertheless implement the full Omega specification. Every
accepted construct keeps its ordinary Omega semantics, ABI, layout, and
artifact contract, and unsupported constructs reject explicitly.

The profile cannot be frozen until `OMEGA-PRODUCT-COMPILER-SOURCE` publishes the
exact transitive compiler source and build manifest. The working defaults are:

- omit the math/proof surface and linear/dependent types from compiler source;
- omit terminal-Psi interpreters, REPLs, and product tools not imported by the
  compiler build;
- retain ordinary named fields, payload-bearing enums/sum data, and basic
  generics unless evidence shows that their Delta implementation and assurance
  cost exceeds their source-level benefit;
- measure concrete domains, domain polymorphism, advanced generic facilities,
  numeric/schema field tags such as `0:`, and complex transition payloads
  against the actual compiler source before admitting them.

The gate must compile the complete manifest under an explicit allowlist and
carry a negative canary for every rejected feature. The profile includes all
transitive libraries, generated and compile-time source, build behavior, and
compiler-imported tools. See
[`../../../wiki/architecture/bootstrap_lattice/self_hosting_profile.md`](../../../wiki/architecture/bootstrap_lattice/self_hosting_profile.md).

## O0 — Omega vertical-canary acceptance profile

O0 is exactly the single-file console program shape represented by
[`../../corpus/cli_mvp/main.omg`](../../corpus/cli_mvp/main.omg):

- UTF-8 source with whitespace, line comments, identifiers, integer literals,
  string literals, and the punctuation used by the declarations below;
- `use omega::language::std::console;`;
- one `data Main` declaration with one `console: Console` field;
- one `machine Main::main(&mut self)` entry;
- `self.console.write_line(<string literal>);` followed by
  `self.console.exit_process(<i32 literal>);`.

The O0 front end must lex and parse the complete input, reject trailing or
unknown constructs, resolve `Main`, `Console`, `main`, and both console
operations, and type-check their receiver and arguments. Duplicate declarations,
unknown names, wrong receiver types, wrong argument types/counts, malformed
strings, and a missing entry are negative gates.

[`omega0-frontend.alp`](omega0-frontend.alp) now implements that front end in
D0. `bootstrap/rungs/delta/samples/omega0-frontend.alp` is a rung-local
compatibility symlink for the shared compiler slice; the historical
`compiler/delta-rs/samples/omega0-frontend.alp` entry resolves through the
temporary on-ramp compatibility paths.
It accepts exactly one canonical bundled source, retains at most 2,048 source
bytes with checked exhaustion, validates the complete source as UTF-8, and uses
a streaming lexer rather than a token arena. Fixed O0 names use ASCII
identifiers; integers are unsuffixed nonnegative decimal `i32` literals; cooked
strings admit direct UTF-8 bytes plus `\n`, `\r`, `\t`, `\0`, `\"`, `\\`, and
`\xNN` byte escapes. The parser consumes the complete frozen declaration/call
shape and retains up to 1,024 decoded `write_line` bytes plus the exact
`exit_process` literal. Until terminal-Psi emission consumes those operands, a
success digest makes perturbations of either observable. The focused native and
Delta-written self-host gate covers the canonical source, trivia and UTF-8
variants, the name/type/count rejection matrix, malformed input, and distinct
source/string exhaustion.

Accepted O0 lowers through the selected terminal-Psi representation to a
deterministic runnable artifact. The artifact must print the literal plus one
newline, then exit with the requested low-byte status; those observations must
agree with canonical meaning.

Terminal-Psi vocabulary 25 represents both boundary operands needed by O0.
`BoundaryMachineDeclaration` carries ordered scalar parameter types and
`OperationKind::BoundaryCall` carries ordered scalar `ValueId` arguments. The
checked producer retains exact scalar expressions, the codec and verifier bind
their order, the interpreter supplies them to the effect handler, and Omega's
abstract operation preserves them. Provider candidates remain outside this
scalar boundary slice and reject such signatures rather than silently ignoring
them.

Vocabulary 25 also carries a first-class borrowed byte-sequence structural type,
an exact raw-octet literal establishment, and the literal's custody through a
bodyless boundary call. Psi syntax, resolved, typed, and checked trees own the
exact bytes, and checked-to-terminal lowering passes the established literal
place to that call. Omega preserves the same custody through abstract, target,
assigned, machine, object, image, and installation forms. On Linux x86-64 and
AArch64 the exact literal-only call is an import-free short-write loop over the
literal plus one newline; nonliteral forwarding and other targets reject rather
than transcoding or discarding it.

Linux `exit_process(i32)` is now realized directly as import-free `exit_group`
on x86-64 and AArch64. Lowering accepts only the exact constant/call/nominal-tail
shape, preserves the consumed scalar in settlement evidence, emits a trap after
the nominally nonreturning syscall, and keeps Darwin/Windows fail-closed pending
validated import/relocation evidence. The metadata-only port settlement remains
unrelated to process exit.

The authored `Main { console: Console }` shape remains attached in terminal Psi.
The relevant provider field is erased from runtime layout only alongside exact,
sorted provider roots for `write_line` and `exit_process`; verification requires
those roots to equal the boundary calls and rejects missing or substituted
attachments. The Delta frontend streams this canonical module directly through
ordinary `write_byte`, byte-identical to the shared-codec vocabulary-25 fixture.
It uses no private terminal representation or artifact buffer; incomplete output
is never accepted because every truncated prefix fails canonical decoding.

The direct artifact edge is also Delta-written:
[`omega0-terminal-to-elf.alp`](omega0-terminal-to-elf.alp) consumes the frozen
terminal module and emits the canonical 8 KiB Linux x86-64 ELF directly. It has
no assembler, linker, signing, or object-format host dependency. The decoder is
intentionally limited to O1, retaining every admitted literal up to O1's count
and aggregate-text limits plus every nonnegative `i32` exit operand. It must
grow with later profiles rather than be presented as the production Omega
backend.

The frontend is covered by the canonical lower-rung meaning route for its used
profile. Native execution, Delta-written `lowermachine`, and the Beta-written
`../meaning/omega2gamma.beta` plus Gamma interpreter agree on retained operand
digest 107
for the canonical `cli_mvp` input, while semantic rejection remains pinned at
251. Exact coverage must continue to grow with the eventual bridge source; this
gate is not authority for constructs that source has not exercised.

The direct backend is covered through the same lower-rung route. One bounded
compiler-scale elaboration is executed against the canonical O0 terminal module,
an operand variant, malformed magic, and both O1 exhaustion controls. Its full
`(Pair status stdout)` observation must equal native Delta execution byte for
byte; the canonical 8 KiB image must also equal the independent product image.
The fixed negative x86 branch displacement is represented by its literal encoded
bytes, so this evidence does not claim unused general signed bitwise semantics.

O0 excludes build files, packages beyond the fixed `use`, arbitrary data,
general expressions, user calls, control flow, allocation, proofs, and
optimization. Those features enter later numbered acceptance profiles only when
required by the Omega-source production compiler or a deliberate conformance
slice.

## O1 — variable straight-line console profile

O1 is the monotonic replacement for O0's exact two-statement body:

```text
self.console.write_line(<byte-exact literal>);  // zero or more
...
self.console.exit_process(<nonnegative i32 literal>); // exactly one, last
```

It does not add a new language construct or terminal-Psi vocabulary. Vocabulary
25 and the product pipeline support ordered literal places and Unit operations.
The Delta frontend and direct ELF backend use checked statement storage, dense
IDs, variable canonical counts, ordered operation emission, and complete
preflight of source/table/text/image exhaustion before artifact publication.

The frozen O1 ceilings are one source of at most 2,048 bytes, at most 16
`write_line` statements, and at most 1,024 aggregate decoded literal bytes.
Exceeding a declared storage/image ceiling reports checked exhaustion; malformed
or out-of-profile source reports semantic rejection. Neither case may publish a
partial terminal module or native image.

Acceptance covers 0, 1, 2, and 16 writes through the same code path;
aggregate stdout and newline order; the exact exit status; byte identity with
the shared codec/lowering for representative cases; canonical meaning; and
rejection of bad ordering, a non-final or duplicate exit, trailing operations,
and every declared resource ceiling. The frontend's native and Delta-self-host
gates and the backend's exact-product-image gate close those source/artifact
claims. The composite gate also compiles both compiler programs through the
Delta-written `lowermachine`, then requires bundle → vocabulary-25 terminal Psi
→ ELF to reproduce the independent product terminal and image bytes for 0, 1,
2, and 16 writes, with frontend and backend refusal before partial publication.
The gate's initial `lowermachine` executable is still produced by the
disposable Rust on-ramp, and its Darwin assembly/signing uses `clang` and
`codesign`; this is frozen-O1 dependency/behavior closure, not a Rust-free
compiler lineage or the `Ωself` profile. The lower-rung
`omega2gamma.beta` route also executes the 40-machine
frontend through Gamma and pins the O1 zero/two-write dual-channel results and
semantic rejection. Its previously unbounded expansion was metadata-table
aliasing at machine 25, not an inherent cost of the route. O1 remains a small
vertical compiler slice, not the `Ωself` profile.

The same route now admits the current 695-state `lowermachine.alp` source to
marker-free elaboration. Its explicit translator ceilings are 1,024 states per
machine, four parameters per state, and 524,288 cells for the private
compiler-sized scalar-array carrier; exact state admission and the adjacent
refusals are executable gates. The carrier is a depth-19 persistent tree with a
compact all-zero root, so admission does not materialize the fixed backing.
The canonical Gamma interpreter also executes the resulting compiler on the
arithmetic sample: decoded status 0 and all 800 output bytes equal native Delta
execution. The interpreter uses checked evaluator-private argument scratch
instead of allocating a persistent Gamma list for every tail-call transfer, and
the translator's state scanner treats quoted strings atomically so a literal
`//` cannot hide later declarations. Block-final `write_line` also stops at the
closing brace when the optional semicolon is absent. This closes whole-compiler
meaning for the existing Delta compiler; it does not create
`omega-bootstrap` or freeze `Ωself`.

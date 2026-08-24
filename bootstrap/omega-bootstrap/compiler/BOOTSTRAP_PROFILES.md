# Bootstrap compiler profiles

This file freezes executable regression contracts for the Delta-built bridge.
The architectural role and artifact owner is `omega-bootstrap`; this file is
bootstrap-owned and is not a product Omega language specification.

The names D0, O0, and O1 below are legacy canary profiles. They preserve exact
vertical-slice behavior while the real bridge grows, but they are not language
generations, build rungs, or candidate names for the final source profile.
Delta v1 and `Ωself` are the only two source-surface inventories being selected:
Delta v1 is the literal bridge implementation language, while `Ωself` is the
ordinary-Omega profile used by the production compiler source.

The retained canary/profile records are:

- **Delta implementation profile D0** is the current Delta surface used by the
  bridge canaries. It is frozen so those slices do not acquire facilities merely
  because the Rust on-ramp has them. It is not the final Delta specification.
- **Legacy Omega canary profile O0** is the first input accepted by the bridge
  regression slice. It is frozen as a vertical proof of the pipeline, not
  presented as sufficient to express the future production compiler.
- **Legacy Omega canary profile O1** is the first table-driven source slice. It
  is a monotonic extension of O0, frozen at explicit statement and storage
  ceilings below.
- **Omega product-compiler source profile `Ωself`** is the incidental ordinary-
  Omega profile selected by the source closure from which the production
  compiler is built. Versioned product checkpoints now expose provisional
  profiles; the contract remains open until the final exact source/dependency
  closure exists and the general bridge supplies measured implementation and
  assurance cost for the retained features.

O0 and O1 are legacy vertical pipeline canaries, not normative ancestors of `Ωself`.
The eventual profile may reuse their implementation, but it is derived from the
production source closure rather than declared to be the next numbered canary.
There is no implied O2 ladder. Sufficiency cannot be established from the
current Rust product implementation.

Likewise, D0 is discovery evidence rather than a normative ancestor of Delta
v1. Every listed D0 facility—including arithmetic domains, payload sums,
recursion, fixed-backing conventions, and `boundary trait` syntax—must be
re-justified by the complete bridge source or an explicit compiler-host
coherence/robustness argument. Delta v1 is frozen only after that complete
source closure is available and accidental producer behavior has been pruned.

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

These bullets describe what current D0 canaries may use. They do not prescribe
the eventual Delta spelling or require a general boundary-trait facility. The
provisional bridge host authority is narrower: byte input, artifact output,
diagnostic output, and process termination, supplied through a sealed interface
unless a concrete compiler-host argument requires more. Delta v1 chooses the
lowest-total-cost robust shape, not automatically the narrowest syntactic one.

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
version-1 contract is frozen in [`OMEGA_BOOTSTRAP_BUNDLE.md`](OMEGA_BOOTSTRAP_BUNDLE.md).

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

It is one of only two open feature inventories. Delta v1 governs the source
used to implement `omega-bootstrap`; `Ωself` governs the Omega source that
compiler accepts. There is no third bridge-language profile, and the
full-Omega features implemented by the resulting product compiler are not
re-selected here. Executable optimization quality is likewise outside this
source-profile inventory.

Checkpoint 000001 now supplies the first coherent Omega-written compiler closure
and a separately hashed, mechanically enforced provisional normalized-syntax/
resource
[profile](../../../compiler/source-checkpoints/profile-000001.json). Later
versioned checkpoints update and deepen that profile. Typed semantics,
ABI/layout, lowering, Delta capacity behavior, and measured bridge costs remain
open. The profile cannot be frozen until the final source closure exists and the
general bridge supplies the implementation and assurance evidence used to
settle them. Absence from an early checkpoint can justify rejection by that
checkpoint's candidate gate, but not a final exclusion while later compiler
phases remain unwritten. The working authoring and measurement biases for the
compiler's own source are:

- avoid the math/proof surface and advanced dependent/proof-indexed types in
  new compiler source, and presumptively exclude them from the final profile
  unless the complete closure establishes a real implementation need;
- measure ordinary ownership, linearity, and multiplicity separately rather
  than treating routine resource discipline as proof-oriented typing;
- retain ordinary named fields, payload-bearing enums/sum data, and basic
  generics unless evidence shows that their Delta implementation and assurance
  cost exceeds their source-level benefit;
- measure concrete domains, domain polymorphism, advanced generic facilities,
  numeric/schema field tags such as `0:`, complex transition payloads, and
  mixed field-plus-case data against the actual compiler source before
  admitting them. Numeric tags are distinct from ordinary named fields. Simple
  discriminants plus explicit context and separate record/sum shapes are
  comparison points, not preselected restrictions.

The hosted source closure separately omits terminal-Psi interpreters, REPLs,
proof explorers, viewers, debuggers, and other product tools unless the compiler
executable imports them. Tool membership is not an Omega language feature.
The current compiler architecture does import Terminal-Psi representation and
lowering modules, so those ordinary source modules belong to the manifest. That
does not require the bridge to contain or run the standalone Terminal-Psi
interpreter, verifier, viewer, or debugging tools, or to use Terminal Psi as its
own internal IR. A direct checked-IR conservative lowering path is valid;
Terminal-Psi validation is a bridge requirement only if that representation is
explicitly selected on total-cost grounds.

Candidate measurement does not itself settle this profile. A bounded
frontend/typechecker probe may be used to price a provisionally retained source
facility before its artifact representation is chosen. The facility is finally
retained only when its compositional rule, negative boundary, Rust-free meaning,
and selected artifact path are all enforced.

### Checkpoint 000001 source-custody frontend measurement

The first such measurement is now closed under
[`SOURCE_CUSTODY_FRONTEND_PROBE.md`](SOURCE_CUSTODY_FRONTEND_PROBE.md).
The Delta-written raw-unit checker generally parses, resolves, and type-checks
the record/field, fixed-array/index, attached-machine, receiver-mutation,
Trapping/range, scalar-result, and guarded-transition families isolated by
`compiler/psi/source/source.omg`. It accepts the exact unit and a renamed/
reordered equivalent, rejects phase-isolated semantic mutations, and carries
the applicable public resource ceilings. It deliberately does not claim the
checkpoint's qualified-name `path.components` resource merely from postfix
member nodes.

The checker is 78,450 bytes of Delta source with 5,395,760 bytes of fixed
zero-initialized table backing. Native and lowermachine-built observations are
millisecond-scale. Rust-free elaboration produces 626,059 bytes of Gamma under
the 1 MiB ceiling; canonical interpretation of the exact unit takes about two
minutes, so the meaning gate repeats the exact positive plus one semantic
rejection and one exhaustion observation rather than another equivalent large
positive. Its current signed-`i32` interval carrier admits only authored `u32`
literal/range endpoints through 2,147,483,647; larger endpoints remain explicit
unsupported input.

This checker-only measurement remains cost and feasibility evidence. Its
corresponding artifact tranche has since selected and implemented private
versioned `CKIR1` plus direct conservative ELF lowering. Exact native/self
bytes, canonical-Gamma status/publication meaning, exhaustive CKIR resource and
relation teeth, product fixture behavior, and exact independent reference
reconstruction of every selected ELF byte and relation are closed. A
persisted-Beta checker now
independently validates CKIR1 and recomputes the selected result across the real
fixture/library, valid structural controls, and all 142 schema mutations;
another persisted-Beta checker reconstructs the exact lower-rooted
CKIR1→limited-ELF relation and selected observation. Persisted-Beta source
checkers also reconstruct declarations, types, signatures, copy/layout,
body operations and operands, terminators, transition facts, canonical
evaluation order, and the full selected source result independently of CKIR
and ELF storage. Valid source/artifact and CKIR/ELF cross-pairs close the first
finite, acyclic, returning source→artifact tranche. Cycles, traps, divergence,
and later source-profile facilities remain later checkpoint obligations.
ABI/layout/lowering are private tranche rules rather than Omega ABI promises,
and the final
retain-versus-refactor disposition remains open. The raw-unit interface also
avoids silently widening the frozen O0/O1
bundle transport ceiling; eventual bridge admission must compose the measured
rules with the canonical bundle frontend.

These exclusions describe syntax used by the compiler implementation, not
features implemented for compiler users. For example, the product source may
avoid proof syntax and dependent types while ordinary records, sums, tables,
and procedures in that same source implement full proof parsing, checking, and
lowering. Full-Omega suites validate the resulting compiler independently of
the `Ωself` source census.

Each checkpoint gate must compile that checkpoint's complete manifest under
explicit compositional profile rules and carry a negative canary for every
rejected feature. The candidate profile may be enforced before the bridge is
complete, but it freezes only when the final source closure and general bridge
implementation supply the evidence used to settle every row. The profile
includes all transitive libraries, generated and compile-time source, build
behavior, and compiler-imported tools. See
[`../../../wiki/architecture/bootstrap_lattice/compiler_source_profile.md`](../../../wiki/architecture/bootstrap_lattice/compiler_source_profile.md).

## Legacy O0 canary — fixed console acceptance

O0 is exactly the single-file console program shape represented by
[`../../corpus/cli_mvp/main.omg`](../../corpus/cli_mvp/main.omg):

- UTF-8 source with whitespace, line comments, nested block comments,
  identifiers, integer literals, string literals, and the punctuation used by
  the declarations below;
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

[`omega-bootstrap-frontend.alp`](omega-bootstrap-frontend.alp) now implements
that front end in D0. `bootstrap/rungs/delta/samples/omega-bootstrap-frontend.alp`
is a rung-local symlink for the shared compiler slice. The historical
`bootstrap/rungs/delta/samples/omega0-frontend.alp` entry remains a role-local
compatibility path; the former top-level `compiler/delta-rs` facade is retired.
The frozen O0/O1 program remains one source unit. The frontend now decodes the
complete bounded canonical bundle first, retains every label and exact source
span, validates UTF-8 per unit, and selects exactly one nontrivial O1 unit while
allowing empty, line-comment-only, or nested-block-comment-only auxiliary units.
The reusable block-comment scanner is bounded by each exact unit span, so an
unterminated comment rejects and delimiters cannot pair across units. It never
concatenates units or injects separators. Fixed O0 names use ASCII
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

Terminal-Psi vocabulary 28 represents both boundary operands needed by O0.
`BoundaryMachineDeclaration` carries ordered scalar parameter types and
`OperationKind::BoundaryCall` carries ordered scalar `ValueId` arguments. The
checked producer retains exact scalar expressions, the codec and verifier bind
their order, the interpreter supplies them to the effect handler, and Omega's
abstract operation preserves them. Provider candidates remain outside this
scalar boundary slice and reject such signatures rather than silently ignoring
them.

Vocabulary 27 also carries a first-class borrowed byte-sequence structural type,
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
ordinary `write_byte`, byte-identical to the shared-codec vocabulary-28 fixture.
It uses no private terminal representation or artifact buffer; incomplete output
is never accepted because every truncated prefix fails canonical decoding.

The direct artifact edge is also Delta-written:
[`omega-bootstrap-terminal-to-elf.alp`](omega-bootstrap-terminal-to-elf.alp) consumes the frozen
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
optimization. O0 and O1 stay fixed regression canaries. Any of these features
needed by the production compiler enters the general, compositional `Ωself`
contract directly; there is no numbered acceptance-profile ladder. A separate
bounded conformance slice may exercise implementation machinery, but cannot
admit a feature to `Ωself`.

## Legacy O1 canary — variable straight-line console acceptance

O1 is the monotonic replacement for O0's exact two-statement body:

```text
self.console.write_line(<byte-exact literal>);  // zero or more
...
self.console.exit_process(<nonnegative i32 literal>); // exactly one, last
```

It does not add a new language construct or terminal-Psi vocabulary. Vocabulary
26 and the product pipeline support ordered literal places and Unit operations.
The Delta frontend and direct ELF backend use checked statement storage, dense
IDs, variable canonical counts, ordered operation emission, and complete
preflight of source/table/text/image exhaustion before artifact publication.

The frozen O1 language ceiling remains one program-bearing source, at most 16
`write_line` statements, and at most 1,024 aggregate decoded literal bytes. The
separate pre-profile transport canary admits a bundle of at most 16 source units,
64 bytes per label, 1,024 aggregate label bytes, and 2,048 aggregate exact source
bytes. Exactly one unit may contain the O1 program; all others must be empty or
contain only O1 whitespace, line comments, and nested block comments. These
transport/scanner bounds do not add modules, namespaces, cross-source lexing,
O2, or a feature to `Ωself`.
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
Delta-written `lowermachine`, then requires bundle → vocabulary-28 terminal Psi
→ ELF to reproduce the independent product terminal and image bytes for 0, 1,
2, and 16 writes, with frontend and backend refusal before partial publication.
The gate's initial `lowermachine` executable is still produced by the
disposable Rust on-ramp, and its Darwin assembly/signing uses `clang` and
`codesign`; this is frozen-O1 dependency/behavior closure, not a Rust-free
compiler lineage or the `Ωself` profile. The lower-rung
`omega2gamma.beta` route also executes the complete frontend through Gamma and
pins single- versus multi-source output identity, O1 zero/two-write observations,
semantic rejection, and resource exhaustion. Its previously unbounded expansion was metadata-table
aliasing at machine 25, not an inherent cost of the route. O1 remains a small
vertical compiler slice, not the `Ωself` profile.

## Profile-neutral scalar-call conformance slice

[`../gates/fixtures/omega-bootstrap-scalar-call-v28.hex`](../gates/fixtures/omega-bootstrap-scalar-call-v28.hex)
is the exact product-owned differential reference for the next general compiler
tranche. It is a proof-free vocabulary-28 module with two machines: the caller
establishes signed `i32` value 73, passes it through `OperationKind::Call`, and
returns the callee's scalar result. The owning product test decodes/re-encodes
the bytes, verifies and interprets them with fixed fuel, lowers them through the
Linux x86-64 internal-relocation path, and rejects mutated arity, callee,
argument identity, and result type. The exporter gate requires repeated output
to equal the committed bytes.

The fixture remains differential evidence rather than bootstrap authority. The
Delta frontend and backend now implement the corresponding bounded,
table-driven conformance slice: one program unit; at most 16 arbitrarily named
machines, four signed-`i32` parameters or arguments, and 16 operations per
machine; literals, parameter references, scalar results, and acyclic calls.
Declaration/name permutations take the same implementation path. Duplicate or
unknown names and IDs, signature/type/result mismatches, cycles, malformed
terminal structure, and adjacent table/code exhaustion reject before
publication. Native, lowermachine-built, lower-rung meaning, product terminal
validation, terminal mutation, and runnable-artifact gates carry the slice.

Only the canonical two-machine fixture has a product-owned exact source-lowering
byte identity. Other accepted programs are codec-compatible and independently
decoded, re-encoded, verified, interpreted, and lowered; they do not claim a
second product frontend produces identical terminal bytes. The source lane's
unique zero-parameter indegree-zero root and the backend's process-status shim
are deterministic conformance-adapter conventions. They do not replace Omega's
authored target-qualified `target::ProgramEntry` binding. The slice does not
define `Ωself`; modules, recursion, records, generics, domains, proofs, and
general control flow remain separate questions.

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

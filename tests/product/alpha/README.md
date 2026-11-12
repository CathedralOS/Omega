# Omega-written scalar compiler execution

This gate compiles an Omega source unit with the Omega-written Psi frontend,
lowers its checked scalar operations, emits Alpha tape with the Omega-written
backend, and executes those exact bytes on the audited Alpha seed. The host Rust
compiler builds the gate executable and remains an explicit development trust
dependency. This result does not establish bootstrap construction of that
executable or Omega compiler self-compilation.
The compiler source also retains language migration debt accepted by that
comparator, including retired domain-cast spellings and implicit domain
qualification. Execution does not establish conformance of the complete
compiler-source closure to current Omega.

```sh
OMEGA_CLI=/absolute/path/to/fresh/omega OMEGA_TARGET=linux_x86_64 \
  sh tests/product/alpha/run.sh
```

When using a comparator built in another checkout, set `OMEGA_LIBRARY_ROOT`
to this checkout's absolute `source/library` path. Pin the executable too; a
long review/build run must not switch compiler or library inputs midway.

The input is one little-endian `u32` selected machine index followed by the exact
source bytes of one unit in one diagnostic package. The frontend lexes, parses,
resolves, types and checks the entire source before lowering. Selection uses the
resulting declaration identity; no name or source fingerprint selects a special
compiler path. The selected entry must be a top-level nullary machine returning ordinary `u8`, with no
clauses whose preconditions the synthetic caller might otherwise assume.
Only ordinary free declarations are admitted; qualified owners and trait or
implementation members need their proper selection semantics first. Nested
helpers remain callable through ordinary lexical resolution, but cannot be
selected as external entries. Only machine root items are admitted; global
declarations and imports are unsupported even when unused. Erased bindings
are also unsupported, so they cannot silently become runtime data.

The initial lowering supports immutable initialized `u8` locals, exact integer
constant trees, checked `u8` identity casts, ordinary `u8` arithmetic and resolved
nullary calls. Explicit singleton `Wrapping`, `Saturating` and `Trapping` casts
retain the same byte payload through locals and casts. Arithmetic, signatures
and the selected entry still require ordinary unqualified `u8`. The machine
ends with its scalar result. Every machine must fit this subset, including
uncalled declarations; unsupported facilities produce no tape. Recursive call
cycles are explicitly unsupported until constant-space tail transfer is
implemented. Acyclic calls use separate frames and preserve all caller values.
Anonymous literal division remains rational through the typer. Fractional
history requires explicit `as` at a computational integer landing, even when
cancellation makes its final value integral; typed integer division uses the
language's truncating quotient. Unary negation preserves the exact denominator.
Checking and lowering share the typed annotation for a completed anonymous
integer root; earlier typed operations remain ordinary obligations. The
lowerer keeps policy storage separate from its unqualified arithmetic guard.
Canonical single-policy `+`, `=` and `-` casts preserve exact carrier and access.
Anonymous domain-only casts without an explicit carrier, named domains and
simultaneous multi-entry accounting remain explicit coverage refusals; no carrier
is guessed and no expected destination inserts a domain. Qualified builtin
indexing likewise requires an explicitly unqualified view until the source
compiler selects declared indexing operators. The frontend's bounded core
policy path accepts fixed-integer addition, subtraction, multiplication, division,
remainder and comparisons with a peer of the same carrier carrying the same
policy or none. Anonymous peers land only at the plain carrier; arithmetic
retains the policy result and comparison returns `bool`. Different policies,
different carriers, and other qualified builtin fallbacks are refused. Qualified
unary, bitwise and shift operators still require declaration support. These
frontend checks do not broaden the scalar backend's arithmetic support.
Mathematical `Int`/`Nat` and decimal landing remain
outside this bounded numeric frontier.

[`source/psi/lowering/scalar.omg`](../../../source/psi/lowering/scalar.omg) is the
first source-owned lowering implementation. Its bounded internal records use
stable machine/value indices and the existing `IntegerConstant`,
`ExactInteger*`, `Call`, and scalar-return meanings from the
[operation vocabulary](../../../wiki/spec/terminal-psi/encoding.md#operation-rows)
and [normal results](../../../wiki/spec/terminal-psi/calls_and_outcomes.md#normal-results).
It is an internal lowering slice, not another portable artifact format. Full
Psi publication must additionally retain canonical declaration, result, obligation
and contract identities; the current checker retains verdicts and counts, which
cannot be turned into those identities by inventing proof records. No serialized
Psi publication or independently replayable proof claim is made here.

[`source/omega/alpha/emitter.omg`](../../../source/omega/alpha/emitter.omg) consumes
only that lowered program. Each activation owns slots in `[32,64)` MiB of Alpha
memory. Frame allocation is checked before stores; the region is disjoint from
the at-most-2 MiB tape and Alpha's downward return stack. Layout and emission run
the same instruction traversal twice. The emitter publishes nothing until all
source checking, supported-form checks, call-cycle analysis and emission finish.

The public Omega compiler continues to enforce its sealed OCREQ boundary. This
diagnostic invocation makes no package-identity, source-snapshot admission or
complete product-publication claim. The public request still needs its structured
lineage framing migrated to the shared acquisition identity. Its canonical hash
preimage already exists in package acquisition; that implementation work does not
block the independent source invocation or require a new trust claim.
The gate creates an ignored package at `build/product-alpha` containing the exact emitter source and its
own entry, because the product application's package role disallows importing it
as a dependency. Its stable worktree-local path permits reuse of unchanged review
records; parallel runs require separate worktrees. It retains no alternate semantic implementation. Python only
frames inputs, checks outputs and invokes the existing seed stamper. The exact
host-built compiler is retained at `build/product-alpha/build/compiler-under-test`
for additional input checks without repeating native construction; the gate log
records its SHA-256. It is a development artifact, not an admitted bootstrap seed.

Success is exit 0 with the actual tape. Failure publishes no bytes: lexing 251,
parsing 250, resolution 248, typing 249, checking 247, unsupported lowering 246,
explicit gate buffer exhaustion 252, and malformed diagnostic framing 253. These are gate
observations, not new OCOUT code assignments. The tests execute changed source,
selected-entry, call-frame and arithmetic cases, and retain explicit refusal
controls for malformed, unchecked and unsupported inputs.

The focused cast construction path runs with the same artifact and driver:

```sh
OMEGA_CLI=/absolute/path/to/fresh/omega OMEGA_TARGET=linux_x86_64 \
  sh tests/product/alpha/run.sh --domain-casts
```

Its executable cases exercise policy accounting and actual Alpha execution.
The same invocation also packs the retained `SourceId`, `SourceUnit` and
`source_unit_clear` declarations directly from their source owner, with a
caller that removes the collection's policy through an explicit borrowed view.
That customer must pass parsing, typing and checking before the existing
storage-backend coverage refusal; it is not claimed as emitted by this scalar
backend. Bound, shared-access and live-loan negatives must fail earlier.

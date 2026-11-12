# Omega-written compiler execution

This gate compiles an Omega source unit with the Omega-written Psi frontend,
lowers its checked operations and storage, emits Alpha tape with the Omega-written
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

The executable is the actual production construction application. Its
[closed-source-set input](../../../source/omega/README.md#closed-source-set-input)
retains explicit package, unit and entry identities; the focused cases frame
one exact unit in one diagnostic package. The frontend lexes, parses,
resolves, types and checks the entire source before lowering. Selection uses the
resulting declaration identity; no name or source fingerprint selects a special
compiler path. The selected entry must be a top-level nullary machine returning ordinary `u8`, with no
clauses whose preconditions the synthetic caller might otherwise assume.
Only ordinary free declarations are admitted; qualified owners and trait or
implementation members need their proper selection semantics first. Nested
helpers remain callable through ordinary lexical resolution, but cannot be
selected as external entries. Machine roots and monomorphic record declarations are admitted. Global constants,
domains, imports, generic or sum declarations, and linear records remain
unsupported even when unused. Erased bindings
are also unsupported, so they cannot silently become runtime data.

The bounded lowering supports plain unsigned `u8`, `u16`, `u32`, `u64`, `bool`
and Unit values, monomorphic packed records, fixed arrays and references to
supported storage. Locals and parameters own separate storage. Projections use
checked field identities and one shared layout; loads and stores touch the
exact field width. Copyable aggregate reads take fresh snapshots before later
argument or constructor effects. Affine storage can be constructed and borrowed;
arbitrary affine reads, nested references, stored reference fields, arrays of
references, computed reference assignment targets without retained descriptor
places, and aggregate or reference results remain explicit refusals.
Unused field types do not imply support for their executable layouts.

Calls evaluate arguments in source order and stage completed values before
entering a callee. Every acyclic call retains its caller frame, including terminal
calls borrowing caller locals. Recursive cycles remain unsupported until proper
tail transfer is implemented. Final transitions test literal, guard and wildcard
arms in order. Boolean `&&` and `||` skip the complete unselected operand,
including its calls and writes. Ordinary checking still establishes indexing
bounds, access, loans, initialization and arithmetic obligations.

Unsigned comparisons cover the full `u64` range. Exact addition, subtraction
and multiplication preserve the ordinary checked bounds; division and remainder
currently support through `u32`, with `u64` explicitly refused. Explicit singleton
`Wrapping`, `Saturating` and `Trapping` casts preserve stored payloads. Runtime
arithmetic and comparisons still require plain operands; policy arithmetic is
not silently replaced with exact arithmetic. The selected external entry returns
ordinary unqualified `u8`.
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
source-owned lowering orchestration. Its adjacent `layout.omg`, `storage.omg`,
`expressions.omg` and `statements.omg` retain type layouts, owned places and
transfers, expression evaluation and control. Stable machine/value/field identities
use the existing memory, call, control and return meanings from the
[operation vocabulary](../../../wiki/spec/terminal-psi/encoding.md#operation-rows)
and [normal results](../../../wiki/spec/terminal-psi/calls_and_outcomes.md#normal-results).
It is an internal lowering slice, not another portable artifact format. Full
Psi publication must additionally retain canonical declaration, result, obligation
and contract identities; the current checker retains verdicts and counts, which
cannot be turned into those identities by inventing proof records. No serialized
Psi publication or independently replayable proof claim is made here.

[`source/omega/compilation/alpha/emitter.omg`](../../../source/omega/compilation/alpha/emitter.omg) consumes
only that lowered program. Each activation owns value slots, packed local and
parameter storage, and call-argument scratch in `[32,64)` MiB of Alpha memory. Frame allocation is checked before stores; the region is disjoint from
the at-most-2 MiB tape and Alpha's downward return stack. Copy and zero loops check zero size before accessing memory and use no hidden
return-stack frames. Layout and emission run the same instruction traversal twice. The emitter publishes nothing until all
source checking, supported-form checks, call-cycle analysis and emission finish.

The public Omega compiler continues to enforce its sealed OCREQ boundary. This
diagnostic invocation makes no package-identity, source-snapshot admission or
complete product-publication claim. The public request still needs its structured
lineage framing migrated to the shared acquisition identity. Its canonical hash
preimage already exists in package acquisition; that implementation work does not
block the independent source invocation or require a new trust claim.
The gate builds `source/omega/application/construction` directly, using the
production compilation library. There is no copied emitter or diagnostic-only
pipeline. Python frames inputs, checks outputs and invokes the existing seed
stamper. The exact host-built constructor is retained at
`build/product-alpha/build/compiler-under-test`; the gate log records its hash.
The actual whole-compiler endpoint is `source/omega/construct.py`, which supplies
the complete declared source closure rather than these focused cases.

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
caller that directly accesses the compiler's ordinary byte buffer. The compiler's
private source-buffer helpers retain their explicit bounds; independent qualified
array cases continue to check reference-view accounting. That customer must produce actual Alpha tape and exit 42 after clearing metadata
while retaining byte-buffer canaries. Bound, shared-access and live-loan negatives
must still fail before publication.


The existing driver also runs its storage cases against the same compiler:

```sh
OMEGA_CLI=/absolute/path/to/fresh/omega OMEGA_TARGET=linux_x86_64 \
  sh tests/product/alpha/run.sh --storage
```

These cases execute the actual `SourceUnit` declaration and clear helper, packed
field widths, full-width unsigned comparisons, argument snapshots, constructor
ordering, borrowed terminal-call storage, short-circuit effects and zero-sized
storage. Every accepted source is compiled twice for byte-identical tape and
executed on the same Alpha seed. Unsupported layouts and ownership violations
retain distinct refusal checks.

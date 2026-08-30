# Lattice — ratified decisions

This file records architectural decisions. Current implementation order lives
only in [`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## D1 — Rust exits by role, not by rung

The Rust implementation under `source/omega-rust/` may remain as a comparator,
migration aid, and working product while the lattice closes. It supplies no
semantic authority. Meaning/checking dependencies leave the trusted path before
convenient producers need to disappear.

## D2 — Superseded: the Delta-to-Gamma meaning route

The former decision made a Beta-written program parse Delta and emit Gamma for
evaluation. D11 supersedes it. That route crossed the Gamma ownership boundary,
made Beta an undeclared Delta implementation, and hid a bridge compiler behind
the phrase “meaning route.” Its implementation and receipts are deleted. Git
history is sufficient; no migration diagnostic may preserve a second compiler
architecture.

## D3 — Trust flows through proofs, not native pedigree

Canonical bootstrap artifacts are accepted Alpha tapes checked by direct
source-to-tape refinement under pinned source and Alpha semantics and an exact
observation profile. Native VM seeds have their own Alpha-realization
obligation. Reproducibility, producer identity, native realization, and
independent compiler agreement are useful evidence for operations and debugging
but are not compiler-correctness proofs.

## D4 — Every proof capability lands with operational seams

A calculus feature is not complete merely because the kernel parses it. Each
feature has positive seams to a real compiler obligation and negative controls
that perturb the claim and must reject.

## D5 — Compiler provenance closes by direct checked refinement

The required judgment is:

```text
source subject + produced Alpha tape
  → tape refines canonical source meaning
```

The obligation is reconstructed from both subjects. An artifact cannot define
the question against which it is accepted. Subject identity, source and Alpha
semantics, observation profile, resource profile, schema identity,
certificates, and admissions all participate in replay compatibility.

## D6 — Each rung is implemented by its immediate predecessor

The permanent chain is:

```text
audited Alpha seed
  → Alpha-written Beta compiler       → beta tape
  → Beta-written Gamma compiler       → gamma tape
  → Gamma-written Delta compiler      → delta tape
  → Delta-written Omega source D      → omega₀ tape
  → Omega-written Omega source C      → omega tape
```

Every arrow consumes a genuine compiler for exactly the language on its right.
No lower rung parses past its immediate successor. A compiler may use private
internal representations, but no older compiler, interpreter, or source
transpiler remains an external semantic dependency of a later edge.

`D` is the exact Delta-written source closure of the first full Omega compiler.
`C` is the exact Omega-written source closure of the self-hosting compiler.
They are different implementations of the same complete Omega language.
`omega₀` may be slow and poorly optimized, but it must compile `C` with exact
Omega semantics. The resulting `omega` may apply the optimizer implemented by
the compiler source.

The three source facts remain distinct:

| Contract | Meaning |
| --- | --- |
| Delta v1 | independent compiler-host language used to author `D` |
| features used by `C` | incidental ordinary-Omega subset used to author the self-hosting compiler |
| full Omega | language implemented for users by both Omega compiler implementations |

The Delta compiler accepts Delta, not Omega. The Omega compiler built from `D`
accepts the ordinary-Omega surface exercised by `C` and implements full Omega.
The compiler built from `C` is the optimized self-hosted implementation. Neither
source closure defines the Omega language it implements.

## D7 — Scripts coordinate but never become stages

Shell, Python, or other host scaffolding may invoke the chain, stamp an exact
tape into a selected Alpha seed, compare artifacts, and report diagnostics. It
may not discover a compiler closure, parse or lower accepted source, manufacture
evidence, or decide trust. A required semantic transformation must be
implemented in the named compiler/checker stage, not hidden in a runner.

## D8 — Evidence and admissions compose transitively

Verification records are re-derivable and subject-indexed. Admissions remain
visible through dependency closure and are re-evaluated under each consumer's
policy. A dependency cannot launder an unresolved obligation into a clean
“verified” result.

Human-facing output keeps verified facts, admitted claims, and provenance
metadata visually and semantically distinct. There is no unqualified “artifact
proven correct” verdict; the honest statement names the source, Alpha semantics,
observation and resource profiles, and admissions.

## D9 — Non-lockstep compiler refinement stays inside the existing calculus

A compiler edge does not add a coinductive or labelled-transition-system
judgment to the accepted checker merely because the subject contains loops. The
source language and Alpha have canonical deterministic small-step semantics.
The admission owner presents them as constructive total step functions with
explicit terminal self-loops and defines traces by primitive recursion over
`Nat`.

The proof relates source and Alpha states at nondecreasing synchronization
points. A source step may correspond to zero or more Alpha steps. Every
unmatched step must be observationally silent and decrease one well-founded
rank over the related state pair, so neither side can hide infinite internal
work. The existing first-order calculus and induction rules check the resulting
determinism, progress, observation, synchronization, and rank obligations.

The producer may elaborate and DAG-share the proof, but the artifact-aware owner
reconstructs both machines and the exact input, resource, and observation
profiles. Executable agreement remains diagnostic. A new trusted kernel
primitive is considered only after a concrete attempt proves an expressiveness
failure; certificate verbosity or producer inconvenience alone is insufficient.

This rule applies when a source-language transition system is related to a
lowered Alpha transition system. The Alpha-written Beta compiler's own
`.alpha`-source-to-`.tape` edge is simpler: authoritative assembly encoding
must equal the exact tape. Equal tapes under the same Alpha input and resource
profile have identical deterministic traces in lockstep, so that first edge
does not invent a synchronization function or stuttering rank.

## D10 — Delta meaning is independent; spelling and artifact names are explicit

Delta v1 is an independent compiler-host language with separately fixed syntax,
static judgments, small-step execution, resources, and observations. Delta may
share familiar spelling with Omega, but its language contract is self-contained:
neither the contract nor a checker may consult Omega documentation, a compiler
implementation, or the accepted corpus to decide what Delta source means. A
file accepted by both languages acquires only the meaning of the route under
which it is checked.

Delta refinement quantifies over a verifier-selected observation profile. The
profile fixes sealed input bytes, exact artifact and diagnostic bytes, and
terminal exit, rejection, trap, exhaustion, and incomplete outcomes. Ambient
filesystem state, environment, time, network, and process spawning are absent.

Each apparent fixed bound becomes exactly one of:

- a source-visible Delta semantic bound;
- an explicit resource-profile parameter; or
- a private producer/checker budget whose exhaustion yields `Incomplete` and
  grants no semantic verdict or publishable tape.

The Gamma-written Delta compiler implements Delta-to-Alpha compilation; it does
not define Delta. Coverage may land before authority. Authority additionally
requires the checked refinement route selected by D5 and D9.

Bootstrap file names identify format and role. `.alpha` is Alpha assembly
source, `.proof` is proof-source input to untrusted elaboration, `.beta`,
`.gamma`, `.delta`, `.omg`, and `.psi` identify their respective source
languages, and `.tape` identifies canonical Alpha VM bytecode. Artifact base
names remain descriptive. Native realizations are optional target-qualified
containers, never the canonical compiler identity.

## D11 — Alpha tape is the canonical bootstrap artifact

Every required compiler artifact from Beta through `omega` is one
platform-independent Alpha tape. Each compiler is standalone: it consumes its
own language and emits the next exact tape without invoking an older compiler,
interpreter, assembler, or host script to perform a semantic transformation.
The Alpha-written first compiler may use the Alpha assembler during its audited
construction; later compilers emit the canonical tape format directly.

A target-specific Alpha VM seed executes any admitted tape. Stamping a tape
into a seed or placing it in an equally transparent container is packaging, not
another compiler edge. Canonical identity and refinement belong to the tape,
not the Mach-O, ELF, PE, signature, or installation wrapper.

The product Omega compiler may emit native user artifacts. That does not make
native code the canonical bootstrap representation of the compiler itself. A
general Alpha-to-native realizer may accelerate any Alpha tape only when its
relation to Alpha semantics is independently checked. Source-, function-,
hash-, or workload-specific native substitutions (“jets”) are forbidden.

Common Alpha targeting reduces the trusted executable vocabulary but can
increase generated-code and certificate size. Internal CFGs, typed blocks, or
other proof vocabulary may structure a derivation, provided they are checked
lemmas rather than external historical compiler stages.

## D12 — Owner escalation protects the lattice from local workarounds

Implementation stops for an owner ruling rather than silently changing the
architecture when any of these occurs:

- representative `delta → omega₀` or `omega₀ → omega` work has terrible wall
  time, memory, or tape-size behavior after ordinary algorithmic and diagnostic
  cleanup;
- Alpha's instruction set appears too weak or excessively verbose, including
  pressure to add an opcode, widen an encoding, or smuggle a higher-level
  operation into the VM;
- source-to-Alpha certificates or checking time grow prohibitively despite DAG
  sharing, compositional lemmas, and removal of duplicate evidence;
- a special native accelerator, source-pattern substitution, tape-hash shortcut,
  or other jet appears necessary;
- target ABI, object-format, runtime, or hardware behavior leaks into Beta,
  Gamma, or Delta instead of remaining at the Alpha realization or Omega
  product boundary;
- a compiler cannot emit the next runnable tape without invoking an older rung,
  host parser/lowerer, or semantic script;
- a realistic compiler closure exhausts a private bound, requires undefined
  Alpha behavior, or cannot receive an explicit fail-closed resource profile;
- proving an edge appears to require a new trusted axiom or kernel rule rather
  than better untrusted proof production;
- conforming Alpha realizations disagree on the same exact tape and input;
- retaining a legacy component requires a second accepted chain, duplicated
  semantic owner, or permanent compatibility adapter; or
- implementation pressure encourages weakening a language contract,
  observation profile, subject identity, or fail-closed behavior.

These are escalation criteria, not advance permission to add complexity. Until
the owner rules, the existing language semantics and Alpha instruction set stay
fixed and the affected edge remains open.

## D13 — The Beta compiler boundary is typed without wrapping its artifact

The Alpha-written Beta compiler returns exactly one of `Complete(tape)`,
`Reject(reason, source_offset)`, `Incomplete(resource, limit, requested,
coordinate?)`, or `InternalFailure(reason, coordinate?)`. Rejection is a
judgment about observed source. Incompleteness is a private-profile refusal and
grants no verdict about unexamined source. Internal failure identifies a
compiler contradiction and grants no artifact authority.

Alpha halt values 0, 1, 2, and 3 carry only those four case tags, so the
distinction survives a shell's low-byte observation. `Complete` stdout is the
unchanged runnable Alpha payload. Every failure emits the canonical versioned
diagnostic frame beginning with permanently reserved non-opcode `0xFF`; its tag
must agree with the halt word and its closed fields retain exact reason,
coordinate, resource, limit, and requested-amount evidence. Unknown or
noncanonical frames reject. Parser phase numbers are private and never become
outcome identities.

Artifact publication requires `Complete`. Success has no inline diagnostic
header, so complete validation, replay-length agreement, fixup closure,
exact-length publication, total Alpha writes, and the final halt-0 observation
jointly protect its custody. Compiler resource ceilings share one profile table
and yield `Incomplete`; dominated corruption guards and pass disagreement yield
`InternalFailure`. Generated-program statuses 250 and 251 remain separate
runtime observations.

## D14 — One admitted edge may compose bounded checked equalities

The exact Alpha-source-to-Beta-compiler-tape edge remains one owner-fixed root
judgment, but no rule requires one compiler-scale normalization to decide it.
The artifact-owned proof may establish bounded subject-bound equalities as
named lemmas and compose them through the existing checked calculus. This uses
the checker's sound per-equality scratch boundary and adds no assembly-specific
kernel rule or evaluator path.

The two assembler passes have distinct schemas. Pass one partitions the exact
source, threads payload positions and the unique label map, and derives total
payload length; predicted positions do not own tape bytes. One checked joint
freezes that state. Pass two independently partitions source and tape and checks
every encoding and fixup against it. Composition proves adjacency and unique
ownership separately per pass, the exact pass joint, canonical endpoints, and
full exhaustion. Cut locations are untrusted witnesses; the owner fixes the
subjects, assembly relation, schemas, joint, composition theorem, endpoints,
and final proposition.

The canonical Beta checker publishes the finite resource profile certificate
producers must meet. Its Python reference is a diagnostic logical diamond, not
a second resource authority, and it is temporary development scaffolding rather
than a member of the completed bootstrap closure. Measurements bind the exact current subjects and
report conversion scratch, permanently retained proof state, semantic stack,
certificate size, and checking time across candidate chunk counts. Literal
artifact byte counts are observations derived from those subjects, not durable
architecture: editing or golfing either subject requires rebuilding and
rechecking its artifact-owned certificate.

## D15 — Bootstrap implementation source is closed textual ASCII

Alpha assembly, Beta, Gamma, and Delta source share one outer byte envelope.
The only admitted source bytes are horizontal tab (`0x09`), line feed (`0x0A`),
carriage return (`0x0D`), and printable ASCII (`0x20..0x7E`). Every other byte,
including NUL, DEL, a byte above `0x7F`, and every byte of a Unicode BOM,
rejects before tokenization at its exact byte offset. There is no source
decoding, Unicode normalization, Unicode classification table, or
host-locale-dependent predicate anywhere in these four language contracts.

Each language defines its narrower token grammar inside that envelope.
Identifiers and decimal digits use explicit ASCII ranges. Exactly space, tab,
CR, and LF may act as whitespace; a comment ends at CR, LF, or source end.
Direct literal contents are printable ASCII except for their delimiter and
backslash, with each language's closed escape set supplying admitted control
bytes. Arbitrary bytes remain program data and artifact bytes: a language may
produce them through numeric byte operations, escapes, or checked embedding,
but they do not occur raw in bootstrap implementation source.

The profile applies to exact source subjects, including comments. It is checked
by each language implementation before lexing and independently by a
closure-aware repository gate. File suffix alone never selects the gate:
Gamma's current implementation material is Beta source, and future compiler
closures may contain several owned files. Capacity controls use valid space
padding rather than treating NUL as invisible trivia. Cleaning an exact source
subject changes its source identity even when comments erase before lowering,
so affected certificates and measurements are rebuilt before publication.

The completed chain is self-contained from the audited native Alpha seed and
repository-owned bytes. Python, Rust, shell, and other ambient host tools may be
temporary invocation or differential scaffolding while edges are incomplete;
they never define source, decide admission, or enter a canonical closure. Every
such reference has a deletion condition, and Python reference implementations
are deleted once the checked direct edge subsumes their named diagnostic role.
No network, host Unicode database, package installation, or historical
compiler is a prerequisite of the completed bootstrap.

## D16 — Gamma is one typed pure language with an explicit compiler adapter

Gamma accepts typed `data* def*` programs with no trailing untyped expression.
It has nominal immutable algebraic data, checked signed 64-bit `Int`, compact
immutable `Bytes`, exhaustive matches, strict left-to-right evaluation, mutual
recursion, and proper tail calls. Functions and constructors have arbitrary
arity; a predecessor register count is an implementation concern rather than a
Gamma language limit. Gamma exposes no general byte-I/O effect.

`Bytes` is a language primitive because compiler input cannot be represented as
one algebraic node per byte within the rung's realistic memory profile. Its
representation is private and cannot expose storage coordinates. Fuel, source,
heap, stack, and output ceilings are implementation-profile bounds. Their
exhaustion yields `Incomplete`, never a Gamma result, rejection, divergence
verdict, or partial artifact.

A Gamma compiler application publishes a typed `main` under D19's sealed
application-profile selection. The Delta compiler returns `Complete(Bytes)`,
`Reject(DeltaRejectReason, source_offset)`, or D31's two exact
application-static-storage refusals. The accepted Delta contract owns the
source-declared closed reason sum, while the selected profile owns its explicit
constructor-to-code table; declaration order is not a wire code. The generated
Alpha adapter alone reads sealed input, invokes pure `main`, writes raw success
bytes, and maps checked storage refusal, private exhaustion, or a trap to outer
`Incomplete` or `InternalFailure` cases.

Compiler boundaries share D13's four halt tags and canonical failure-frame
shape, not its Beta-specific identity. `BCOUT`, `GCOUT`, and `DCOUT` have
edge-owned magic, version, code tables, and coordinates. One parameterized gate
may decode them, but an edge never accepts another edge's frame. Success remains
an unwrapped Alpha tape and every failure publishes no partial artifact.

The strict static frontend is now absorbed into the canonical Beta-written
compiler source; the Gamma interpreter remains a temporary oracle and candidate
algorithm source. Their historical correlated omission of match exhaustiveness
demonstrates the limit of a differential diamond: agreement detects divergence
between implementations but cannot establish a rule both omit. The compiler
frontend rejects incomplete coverage and the interpreter fails loudly on no
match during migration; the completed canonical compiler still owns the static
judgment. It type-checks and emits Alpha tape directly rather than packaging an
interpreter with source syntax.

## D17 — Delta v1 is one closed fixed-storage compiler-host language

`source/delta/LANGUAGE.md` is the self-contained normative Delta v1 contract.
Delta shares familiar spelling with Omega but inherits no Omega meaning. Its
source closure is resolved outside the language and packed into one exact
translation unit; top-level forward references ensure that packing order
changes coordinates rather than program meaning.

Checking and execution are distinct judgments. `CheckDelta` either accepts one
program or returns one closed `DeltaRejectReason` and exact packed byte offset.
`RunDelta` yields only `Exit`, `Trap`, or actual divergence. `Incomplete` and
`InternalFailure` belong to bounded tools and compiler adapters, never to Delta
program semantics, and publish no partial artifact. `DCOUT` owns explicit
versioned constructor-to-code tables independent of Gamma declaration order.

V1 reserves only its active syntax and removes contextual or speculative
surface: no packages, imports, attributes, domains, range types, contracts,
`terminates by`, generic parameters, heap, or recursive value types. It retains
finite records and sums, arbitrary finite payload arity, fixed arrays, bounded
non-escaping views, `i32`, storage-only `u8`, return-only `never`, checked
scalar operations, short-circuit Boolean connectives, assertions, machines,
states, transitions, recursion, and one exact receiver-qualified `Console`
boundary. Scalar transition misses trap deterministically.

Because Delta has no heap or recursive value type, `D` represents dynamic
compiler structures in source-declared fixed arrays with integer indexes.
Those capacities and their failure behavior are semantic program state. Small
parser, arena, declaration, parameter, or output ceilings inside an
implementation remain private budgets whose exhaustion is outer `Incomplete`,
not a language rejection or limit.

The Gamma-written compiler exposes D19's source-owned pure
`main : Bytes -> DeltaCompileOutcome`. That sum carries success, typed Delta
rejection, and D31's two selected-profile static-storage refusals. The sealed
`DeltaCompilerV1` selection validates the exact nominal schema and complete
reason-code bijection before its adapter alone owns sealed input, the four
compiler halt tags, `DCOUT` framing, and outer resource/internal failures. The
compiler emits an unwrapped Alpha tape and is accepted only by direct checked
Delta-source-to-Alpha-tape refinement. Deleted translator behavior and
historical samples grant no compatibility claim.

## D18 — Omega compilation crosses one sealed graph request and one admitted build checkpoint

The two standalone Omega compilers consume the same canonical logical request.
`OmegaCompilationSubject` carries the resolved package graph, root role,
requester-local aliases and edges, each `PackageKey` together with its exact
resolved source coordinate and complete immutable build-visible package
snapshot. An already accepted dependency may additionally carry its sealed
`PackageInstance`; the root's artifact-bearing `PackageInstance` cannot exist
until this compilation succeeds. `OmegaInvocation` carries the requested product, canonical target
profile, external admissions, and an exact binding to that subject. One sealed
request joins them. Discovery, network access, ambient filesystem traversal,
and host defaults remain outside the compiler; compiler-injected build
vocabulary remains part of the compiler identity rather than host-supplied
source.

The snapshots are deterministic virtual filesystems, not filesystem replay
records. They contain every byte, directory, absence, link, and metadata fact
that admitted build operations may observe, use canonical raw-byte paths and
directory order, and expose no ambient environment, clock, randomness, or
network. Build evaluation may produce an operation replay, but a prior replay
is neither the request's source authority nor a substitute for a complete
snapshot.

Each compiler independently lowers the sealed subject through the existing
coherent private typed stage, resolves exact dynamic-call identities, prepares
the existing static-machine-specialized build projection, infers its
operational and service reach, and admits or rejects the selected build
machine's complete authority ceiling. The compiler freezes that base frontend,
prepared projection, reach plans, source commitment, and verdict as one
activation-local checkpoint. It executes `build.omg` only from the admitted
projection, then continues ordinary checked lowering from the same retained
base rather than rereading source or reconstructing a nominally similar
frontend.

Source generated by that activation occupies a strictly later scope stratum.
Generated declarations may resolve authored declarations, but they are never
candidates for resolving any authored occurrence, including the selected build
machine's transitive helper closure. Admission is therefore stable by
construction rather than by a post-hoc non-interference comparison. Generated
units in that later stratum may resolve together. A dependency's already
finalized generated-source bundle is part of that dependency's imported
interface; the consumer never reruns the dependency build.

Package orchestration processes dependencies in deterministic dependency order
and retains durable source bundles, commitments, and review evidence between
activations. It never retains a graph-wide collection of live partial compiler
checkpoints. One activation's prepared projection and reach machinery may be
discarded after build execution; its normalized build configuration, generated
source custody, observations, and admission evidence continue through final
checking and artifact production.

The explicit bootstrap Alpha-tape product is an ordinary request product, not a
filename or host-target inference. The Omega compiler edge uses `OCOUT` under
D13's shared compiler-boundary family: halt tags 0 through 3 mean `Complete`,
`Reject`, `Incomplete`, and `InternalFailure`; success is the unwrapped Alpha
tape; every failure uses the edge-owned versioned frame and publishes no
artifact prefix. Request decoding rejects malformed identities, duplicate or
dangling graph rows, inconsistent lengths or custody, and trailing bytes.
Diagnostics are ordered first by fixed phase. Request-framing diagnostics then
use request-byte offset; source diagnostics use canonical package order,
source-unit order, and byte offset. Timing observations never enter semantic
identity.

D25 supplies the authoritative outer framing and canonical-content rules for
this logical request and outcome. It retains each selected immutable source
revision independently from the stable `PackageKey`, but deliberately gives no
V1 wire identity to the future accepted `PackageInstance` carrier. Q2 owns the
still-missing inner field/tag tables, commitment preimage, failure-code tables,
phase order, and scalar provisions needed for one byte-interoperable wire.

## D19 — Gamma application adapters are selected by one sealed two-profile input

Gamma source semantics ends at a pure returned value. A runnable Alpha tape may
join that value to sealed input, stdout, halt, and diagnostic framing through a
compiler-generated adapter, but Gamma source does not select that environmental
contract. The canonical Gamma compilation question therefore contains the
exact source together with one closed application-profile ID. The ID is sealed
invocation input and participates in compilation identity, reconstruction
evidence, and the emitted adapter's custody. It is neither Gamma syntax nor an
ambient host flag, filename inference, or post-emission rewrite.

Version 1 has exactly two profiles. `ConformanceBytesV1` requires the
resolved entry `main : Bytes -> Bytes`; its adapter supplies sealed input,
preflights the complete returned `Bytes`, publishes exactly those bytes on
success, and uses its own closed runtime-containment profile without claiming a
compiler boundary. `DeltaCompilerV1` requires source declarations with the
exact Gamma schema:

```text
(data DeltaCompileOutcome
  (Complete Bytes)
  (Reject DeltaRejectReason Int)
  (StorageIncompleteAt Int Int Int)
  (StorageIncompleteTotal Int Int))

(def main ((source Bytes)) DeltaCompileOutcome ...)
```

The rejection `Int` is the exact source byte offset. D31's storage cases carry
`(limit, requested, source_offset)` and `(limit, requested)` respectively and
are the sole source-authored path to outer `Incomplete`.
`DeltaCompilerV1` generates D17's `DCOUT` adapter: `Complete` publishes the
unwrapped Alpha tape, `Reject` publishes the versioned rejection frame, checked
storage refusal publishes the corresponding resource frame, and adapter-owned
exhaustion or contradiction publishes outer `Incomplete` or
`InternalFailure`. Every failure publishes no artifact prefix. Each profile
owns its exact sealed-input maximum, entry contract, result validation,
external observation profile, and private resource/status table. D21 requires
that maximum to lie in `0..INT64_MAX`, checked as profile metadata before
adapter emission.

`Int` and `Bytes` remain Gamma's only built-in types. `DeltaRejectReason` and
`DeltaCompileOutcome` remain ordinary source-owned nominal declarations; the
profile does not inject hidden builtins or make structurally similar nominal
types interchangeable. The sealed profile grants the external boundary. Names
such as `main`, `Complete`, `Reject`, or `DeltaCompileOutcome` grant nothing and
never select a profile.

Before emitting any adapter, the compiler resolves and retains the exact entry,
result type, outcome constructors, and rejection-reason constructors, then
checks them against the already selected profile. The outcome sum has exactly
the required four constructors and payloads. The profile-owned `DCOUT` table is
a checked bijection over the complete source-declared reason sum: every exact
constructor has one unique in-range code and every table row identifies one
exact constructor. Codes never derive from spelling or declaration order. A
schema, table, or entry mismatch is a `GCOUT` compilation rejection and can
never survive as an unhandled emitted-program case. Changing the reason sum or
wire table requires an explicit D17/profile-version decision.

One general Gamma compiler implements both profiles. Separately admitted
compiler artifacts per adapter would duplicate custody and refinement for one
checked language; an in-source application declaration would let source choose
an external boundary; and hardwiring the immediate Delta customer would prevent
the language's own general conformance use.

## D20 — Gamma names resolve through four namespaces without active shadowing

Gamma has four semantic namespaces selected by grammar position: types,
constructors, functions, and local values. Type declarations are unique among
types, constructor declarations are globally unique among constructors because
their uses are unqualified, and function declarations are unique among
functions. Global declarations are collected before their type spellings are
resolved, preserving D16's forward and mutual visibility. A duplicate rejects
at the exact later declaration; lookup never acquires first-wins or last-wins
meaning from table traversal.

Grammar-distinguished namespaces may reuse a spelling. A type and constructor
may both be named `Token`, and a global function and local binder may both be
named `f`: type, constructor, call-head, and value-atom positions already choose
the relevant namespace. Gamma has no function values. These permissions do not
make declarations structurally interchangeable or merge their retained rows.

Parameters, `let` binders, constructor-pattern binders, and catch-all patterns
share the local-value namespace. A new binder may not duplicate any binding in
its active lexical environment. Function parameters are mutually unique. A
`let` initializer sees only the outer environment and its binder is active only
in the body. Pattern binders are mutually unique, cannot duplicate an active
outer local, and are active only in their arm. Duplicate pattern names reject
rather than assert equality. Disjoint arms, branches, and sibling scopes may
reuse names because the bindings are never active together.

The canonical resolver therefore performs global collection and within-
namespace duplicate rejection, resolves mutually visible declaration types,
then checks bodies with explicit lexical-environment push/pop and exact source
coordinates for the later conflicting binder. D20 replaces the temporary
checker's accidental first-global and last-local lookup behavior and unblocks
the source-to-resolved-identity joins in the Beta-written Gamma compiler.

## D21 — Every Gamma Bytes value has an Int-representable logical length

Every valid Gamma `Bytes` has one exact logical length in `0..INT64_MAX`.
`bytes_empty`, `bytes_single`, and `bytes_slice` preserve that invariant.
Compiler-generated sealed-input adapters may construct `Bytes` only under a
D19 profile whose exact maximum input extent lies in the same range; the
compiler validates that profile metadata before emitting the adapter.

`bytes_concat` is the only length-increasing language constructor. It loads the
operands' stored logical lengths, computes their exact mathematical sum before
allocation, and traps if that sum exceeds `INT64_MAX`. No heap mutation or
partial descriptor may precede the check. On success the new descriptor stores
the exact sum, making `bytes_length : Bytes -> Int` total over valid values.
Deliberate rope doubling can reach the guard with little physical storage, so
the rule depends on logical length rather than representation size.

The overflow is an authored Gamma trap because it depends only on the operand
values. A representable concatenation that cannot allocate is profile-owned
`Incomplete`; malformed private descriptors or impossible checked states are
`InternalFailure`. D19 already maps a Gamma trap in the generated Delta-compiler
application to outer `InternalFailure`, so D21 adds no halt tag or wire outcome.
The Gamma guide owns the closed authored-trap condition list; exact published
diagnostic subcodes, if distinguished, remain in the selected edge profile's
versioned reason table.

## D22 — Delta names use scoped namespaces and one pre-type duplicate census

Delta resolves names through grammar-selected namespaces rather than one
universal spelling table. Boundary traits and data declarations share the
type-owner namespace. Machine identities are keyed by `(optional owner, name)`;
an unqualified machine may share a type owner's spelling, and qualified and
unqualified machines remain distinct. Boundary members, fields or cases, state
labels, and local values retain their exact owner-local scopes. Position-
qualified members such as `self.index` may share a spelling with a bare local
`index`.

After parsing and before type formation, the compiler performs one complete
scoped identity census over owners, machines, boundary members, data members
and payload names, parameters, states, and lets. Every `DuplicateName` belongs
to declaration collection; the globally earliest later declaration among all
scoped duplicate pairs supplies the diagnostic coordinate. Census does not
grant visibility: a let remains unavailable while its initializer is checked,
and ordered body checking still owns `UseBeforeInitialization`.

Delta permits no active local shadowing. Machine parameters are active for the
invocation; state parameters and lets are confined to their entry or state body;
a let becomes active only after initialization. A new local binder cannot reuse
an active machine parameter, state parameter, or earlier local. Distinct state
bodies are disjoint and may reuse spellings; values cross a state transfer only
through explicit state arguments.

Boundary members remain exact externally realized callable identities, never
source-fillable implementation slots. Any authored qualified machine body whose
owner is a boundary trait rejects as `InvalidBoundary`, independent of whether
its name matches a declared member. Duplicate signatures within a boundary
declaration remain `DuplicateName`; qualified bodies on data owners are ordinary
owner-qualified machine identities. Lookup never acquires first-wins or last-
wins meaning from collector traversal.

D22's transition-arm binder and collection-failure gaps are completed by D24.

## D23 — AlphaBootstrapV2 admits a one-MiB tape through a coherent checker profile

The canonical bootstrap execution profile advanced from the former 256-KiB
seed hole to `AlphaBootstrapV2`. Its seed hole is exactly 1,048,576 bytes,
including the four-byte stamped tape length, so its maximum raw Alpha tape is
1,048,572 bytes. This is one global lattice profile, not a Gamma-only exception.
It changes no Alpha opcode, encoding, or execution rule: runnable-tape capacity
remains an execution-profile fact rather than Alpha language semantics.

The profile revision is atomic across every owner of the old extent. Both
platform seeds and their stamping paths, the Beta compiler's payload storage
and generated-program memory maps, adjacent compiler-profile ceilings including
the procedure table, Gamma's emitted-program stack and heap boundaries, the
authoritative checker's input frame and subject arena, compiler outcome resource
tables, and all exact/adjacent limit gates moved together. The migration is now
landed: no current component may advertise V2 while retaining a V1 consumer or
checker bound.

V2 is admitted only when the authoritative checker can check a realistically
framed maximum-size subject. A seed accepting a maximum tape is insufficient if
the checker cannot retain that tape together with representative source,
certificate, lemma, normalization, and scratch demand. The profile therefore
owns an executed exact-edge acceptance canary over the combined checker frame,
plus adjacent fail-closed resource outcomes. Its maxima remain explicit and
conjunctive; an advertised tape maximum may not be practically unprovable under
the same profile.

The measured Gamma compiler pressure establishes that the old ceiling is a
lattice constraint, not permission for a weakened compiler, split compiler,
host helper, Gamma-specific jet, or deleted conformance gate. General Beta code
density improvements remain ordinary quality work and must preserve the fixed
gate set, but no further density pass gates V2. Per-feature size budgets do not
replace the profile revision.

D14's bounded equality lemmas and composition do not yet authorize a paged root
subject representation. A later checker revision may make subject custody
chunk-addressable and thereby break the current roughly linear tape-to-arena
coupling, but that is a separately specified checker/input change rather than an
unstated part of V2. Real Gamma and Delta artifacts are measured under V2 before
any further capacity revision; continued pressure first reopens subject custody,
not the Alpha instruction set.

## D24 — Delta transition binders complete the scoped identity census

Every syntactic transition payload binder is an ordinary local-value
declaration in D22's pre-type census. Binders in one arm are mutually unique and
cannot reuse an active machine parameter, state parameter, or earlier `let` from
the containing entry or state body. They become visible only while checking
that arm's continuation. Distinct arms are disjoint and may reuse spellings; a
reference to another arm's binder is therefore `UnknownName`, not a binder-
specific failure.

Collection records every binder independently of later pattern validity. An
unknown case or wrong payload arity does not suppress binder census, so duplicate
syntactic binders report the earlier-phase `DuplicateName` before `UnknownName`,
`ArityMismatch`, `DuplicatePattern`, or exhaustiveness checking can apply.
`DuplicatePattern` remains the body/control failure for repeated transition
selectors or cases; it never owns a repeated binder spelling. This intentional
two-round diagnosis follows D17's fixed phase ordering.

`DuplicateName` and `InvalidBoundary` both belong to declaration collection.
Every candidate is anchored at the first byte of its offending declaration:
the later declaration for a duplicate and the authored qualified machine
declaration for `InvalidBoundary`. The compiler reports the smallest packed
coordinate across both candidate families, independent of collector traversal
or reason-table order.

Boundary classification occurs only after the complete owner census and only
for an owner with one unique identity. A uniquely boundary-owned authored body
contributes `InvalidBoundary` whether or not its member name exists. A spelling
declared as both boundary and data has no owner kind: it contributes its
`DuplicateName`, while qualified bodies under that ambiguous spelling contribute
no inferred boundary failure. Fixing the owner collision may consequently
reveal `InvalidBoundary` on a later compile; the compiler never avoids that
two-round diagnosis by choosing a first owner row.

## D25 — One committed OCREQ v1 question drives both standalone Omega compilers

The Delta-written and self-hosted Omega compilers consume one byte-identical
`OCREQ` version-1 question. Its eight-byte identity is
`[4F 43 52 45 51 01 00 00]` (`OCREQ`, version 1, two reserved zero bytes),
followed by little-endian `u32` subject and invocation byte lengths, then those
two exact sections and exact end. Every variable byte value is length-framed;
every table has an explicit count; closed variants have fixed numeric tags and
zero reserved fields. Counts, lengths, and indices are at most `INT32_MAX` so
both Delta and Omega implementations can represent them. A Delta decoder reads
the four raw bytes, rejects a set high bit before signed conversion or checked
arithmetic, and therefore never turns hostile framing into a trap and outer
`InternalFailure`.

The identity, outer section extents, exact end, and canonical semantic contents
in this decision are fixed. They do not by themselves assign every inner row's
byte order, width, tag, or reserved fields, nor the closed failure/resource
numbers. Q2 completes those physical tables before either compiler may claim a
full V1 decoder or publisher.

The subject encodes package rows in ascending recomputed
`PackageKeyIdentity` order. Each row carries the structural package name and
source lineage, the separately selected immutable resolution coordinate, its
role, and one complete build-visible snapshot. The asserted key identity is
recomputed from the structural fields; indices into the validated package table
carry graph references. Dependency rows are ordered by requester index and
requester-local alias. The selected root is explicit. Duplicate, dangling,
foreign, unreachable, cyclic, role-inconsistent, identity-mismatched, or
noncanonically ordered rows reject before source processing. The selected
revision is not a `PackageInstance`: V1 carries no preaccepted compiled
dependency instance, and no omitted or zero row may be interpreted as one. A
later accepted-instance carrier requires a new request version.

Each snapshot is one closed raw-path-ordered tree whose only rows are root or
nested directories, regular files with executable state and exact content
bytes, and symbolic links with exact target spelling bytes. Absence is the
complement of that complete row set; direct-child enumeration follows row
order; payload lengths and canonical metadata are derived. None is serialized
a second time. The root directory, canonical parent closure, unique paths,
existing metadata-policy limits, and exact aggregate content are validated. No
physical host path may be dereferenced or substitute for a snapshot; an
external-local canonical path retained inside structural source lineage is
identity text only. Ambient traversal, environment, clocks, randomness,
network state, and build-operation replay are not request facts.

The invocation carries one closed requested-product tag, canonical target
profile, canonical external-admission tables, and a domain-separated SHA-256
commitment over the exact canonical subject section. The compiler validates the
whole frame and subject canonicality before accepting that commitment. This
binding remains mandatory even though both sections occupy one outer frame, so
cached or separately retained sections cannot be substituted. Bootstrap Alpha
tape is an explicit product. Rust object layout, serde, filename inference, host
defaults, pointers, and report fingerprints never enter the wire.

Request-profile ceilings are closed, named scalar resources. One exhaustion
reports one `u64` `(limit, requested)` pair; multidimensional capacity is split
into separately named resources rather than packed into one value. Invalid or
noncanonical bytes, graph or custody contradictions present in the request, and
ordinary source/build/checking refusals are `Reject`. A canonical request that
exceeds a named private provision is `Incomplete`. Only a contradiction reached
after accepting canonical input is `InternalFailure`. Validation completes
framing, exact end, identities, ordering, graph, snapshots, admissions, and the
subject commitment before any source tokenization.

`OCOUT` retains D13 and D16's common header exactly. Its eight-byte identity is
`[FF 4F 43 4F 55 54 01 00]`; bytes 8 through 39 keep the common outcome,
coordinate-space, reserved, reason/resource, coordinate, limit, and requested
fields, and the halt tag must agree. Coordinate spaces already expressible in
one `u64` remain exact 40-byte frames. Coordinate space 4 is the sole extension:
the header coordinate is the source byte offset and an exact eight-byte tail
carries little-endian `u32` canonical package ordinal and source-unit ordinal
within that package. No other tail is legal.

Omega source diagnostics anchor at the first byte of their retained primary
source span, not at a streaming consumed-prefix cursor. Declaration failures
use declaration start; token or expression failures use the relevant span
start; EOF uses source extent. A generated unit has no request coordinate, so
an `OCOUT` rejection in generated source uses a generated-source reason and
re-anchors to the exact authored `BuildOutput::include_source` call that handed
off that unit. The compiler retains the generated path and internal offset for
local diagnostics, but it never emits an uninterpretable generated-unit ordinal
as request evidence.

Success remains the unwrapped requested artifact. Every failure publishes only
its closed `OCOUT` frame; the profile-owned reason and resource tables, fixed
phase order, canonical coordinate ordering, unknown-code rejection, and
Complete-only publication are common to both compiler implementations.

## D26 — Consumer-owned opaque-representation demand completes build welding

Opaque by-value representation uses the existing `build.omg` trust weld; D26
adds no representation authority or second selection mechanism. Core's
`InterruptMaskGuard` and `InterruptAcknowledgement` are the live customer:
their semantic multiplicity and discharge belong to core, while one selected
provider owns their target-dependent runtime carrier. A producer-fixed source
ABI would contradict that contract and is not an alternate realization of it.

One compilation activation admits at most one selected
`OpaqueRepresentation<Opaque>` application for each opaque declaration. The
selection remains active policy and excludes a second selection even when no
by-value use ultimately demands a shape. Current orchestration admits one
authoritative build machine, so the existing per-machine harvest realizes this
rule; the completed build-configuration join must also validate the
activation-wide invariant and fail closed if a later orchestration ever admits
multiple build machines.

Dependency compilation publishes a compiler-issued generated-source bundle.
The consumer neither reruns that dependency's build machine nor imports its
selection as policy. A representation selected when package A was reviewed as
its own root is therefore historical evidence about A's compilation, not a
constraint on a later consumer C. C selects one application for C's complete
active compilation. Unrelated historical demand rows are never unified merely
because their packages occur in one source closure.

Canonical review keeps two roles distinct. Producer-owned availability binds
the exact opaque declaration and ordinary public conformance/carrier surface;
publication says only that the candidate exists. Consumer-owned demand is
emitted only for an actual runtime by-value use and binds the exact boundary
requirement application, opaque declaration, named conformance or
compiler-owned target-semantics source, concrete carrier, selected target,
closed shape and movement/finalization plan, representation version and
origin, closed-conformance commitment, and complete boundary-calling-plan
commitment. A foreign demand must rejoin the producer's exact canonical rows
and selected immutable source instance. Names, compact report fingerprints,
lockfile strings, and review prose are never agreement.

The selecting build-machine identity, source occurrence, and authored spelling
are retained as audit provenance and source custody, not ABI identity. Moving
or renaming an unchanged selection cannot make equal applications
incompatible. An unused selection consequently produces no demand/ABI row,
although it remains visible through build-source custody and still consumes
the activation's unique-selection slot. Adding the first by-value use may add a
demand row without any `build.omg` change; that diff means a use appeared.

Core's existing equality rule remains normative: every producer and consumer
of one opaque runtime value retains the same application. A fused compilation
uses its one active selection on both sides of every crossing. When future
independently compiled `PackageInstance`s exchange the value, composition must
compare the strong application commitments at that actual by-value edge before
placement or execution. Disjoint artifacts need not share a representation,
and source review must not claim that an artifact composition has already
occurred.

## D27 — Device protocols separate checked custody from opaque compatibility

DMA publication, device acquisition, cache maintenance, MMIO notification,
and posted-write completion are distinct protocol roles. The intended
checked-driver model represents them as explicit typed custody and ordering
transitions. The existing access-plan row is only provisional,
non-authorizing structural scaffolding: no checked source operation emits it,
test construction grants no authority, and its uniform one-range payload is
not public ABI. Notification, completion, and acquisition may require distinct
data, descriptor, doorbell, read-back, request, and completion coordinates.

The first source-admitted rung is one complete DMA service boundary for a named
customer. Hosted, firmware, native, and other opaque implementations enter
through the ordinary `build.omg` trust weld and keep the five lower-level roles
provider-private. The weld selects and audits an exact provider, target,
calling plan, authority, and contract; it does not prove the provider's
internal register, cache, queue, or firmware protocol. Checked source cannot
compose the private roles or claim their intermediate proofs. A public
primitive family waits for a concrete checked driver whose protocol determines
the exact role-specific signatures.

Build selection admits a provider and the schema of its sealed ordering-scope
capability. It cannot issue a runtime occurrence: one selected provider may
create many device, queue, or session scopes. The installed provider issues
each occurrence. Source may carry and pass the capability but cannot construct,
inspect, or compare its identity. Every event binds the exact mapped range,
mapping, stable device instance, runtime scope occurrence, and role. In every
role-keyed row, sum, or certificate collection, the role discriminant itself
enters canonical identity rather than serving only as a payload-decoding
selector.

DMA is an external borrow. Accepted submission creates one linear pending loan
bound to the exact per-transfer `ExternalLoanId`; incompatible CPU access stays
excluded until release is proven. A pre-commit rejection returns every consumed
candidate unchanged. Missing provider coverage rejects compilation or
installation and has no runtime `Rejected` arm. After submission, ordinary
device status and custody release are separate axes. Acquisition returns Stable
CPU custody plus the device status only when exact completion evidence proves
release. A stale, mismatched, incomplete, or non-releasing completion returns
the pending loan and completion candidate for higher-level recovery and never
fabricates Stable custody. Ordinary device failure is a protocol outcome, not
a crash; a provider contract violation remains a trusted-provider defect.

Generic result sums introduce no new multiplicity rule. The nominal container
keeps its declared multiplicity while the active payload carries every affine
or linear obligation introduced by substitution. This is the general rule
already exercised by `TaskOutcome`, atomic outcomes, and retry-custody sums;
it does not determine an operation's disposition table.

## D28 — Exact applications precede universal generic boundary coverage

Generic boundary requirements are real: core array and fixed-vector operators
carry type and const telescopes such as `<T, const N: u64>`. No checked generic
Omega machine currently realizes one of those operators, however. Production
therefore gains no generic-coverage carrier from a provider assertion or test
fixture. The first executable rung reconstructs and rechecks the finite exact
application set demanded by each emitted artifact. Bodyless, external, opaque,
compiler-intrinsic, and separately supplied realizations remain exact-only.

Every emitted application requires one closed compiler-recheckable semantic
coverage fact binding its tagged arguments and selected realization, followed
by one exact physical child for each operation surviving verified optimization.
Exact applications remain mandatory even if a future checked generic
realization has universal semantic evidence; universal checking never creates
concrete instances or physical plans. D29 fixes the semantic fact and its
realization-specific checking rules; D32 fixes the physical continuation.

A future compiler-issued generic row is permitted only for an exact checked
Omega body validated on the pristine pre-monomorphization graph under its
complete symbolic telescope. The row binds the requirement and realization
telescopes, every binder category, declared domain and bound, the exact binder
mapping, requirement coordinate, realization template, symbolic provider
routing and dispatch, and the transitive admissions needed to replay that
semantic check. The requirement domain must imply the realization domain. A
realization may reorder binders through an exact bijection, but it may not
collapse independent binders or narrow an unrestricted parameter to a stronger
property such as `[copy]`.

The universal quantifier ranges over source-semantic applications satisfying
the authored telescope and its `where` requirements. Target layout limits are
not part of that domain: they belong to the exact concrete application check.
A target may still qualify the row when provider routing or transitive
admissions are target-specific. Symbolic plan coverage means provider routing
and dispatch only; it expressly does not establish `Calling<C>`, byte layout,
register classes, stack placement, or any other shape-dependent physical plan.

The exact-application implementation owns the first shared typed-telescope
carrier. Arity and semantic strings are not evidence. Universal generic
coverage remains unrepresented and fail-closed until an actual checked generic
operator realization supplies a client. A future foreign generic contract
requires its own independently recheckable verifier and remains distinct from
checked-body evidence. Provider-authored `generic` flags, one successful
concrete compilation, toolchain strings, and compact fingerprints never prove
universal coverage.

## D29 — Compiler-issued exact boundary-application coverage

One checked boundary-operator use produces a typed application demand rather
than an arity plus display strings. An application binds each lifetime/static
telescope position by category and canonical identity. Binder owner, category,
and ordinal are identity inputs; names and source spellings are diagnostic
only. A const argument is its evaluated value in its declared carrier, so
equivalent expressions such as `2 + 2` and `4` denote the same `4 : u64`
application. Type and const arguments are the first supported production
categories. Lifetime, machine, and proposition applications remain explicitly
fail-closed until an operator actually requires their exact substitution and
replay rules.

A use inside an unspecialized generic artifact may retain arguments referring
to that artifact's own typed binders, but that row is demand rather than
coverage. Final composition substitutes reachable specialization arguments
and may publish coverage only after the application is closed. Every binder is
assigned exactly once and satisfies its declared category, carrier, domain,
bounds, and `where` requirements. Return shape, display text, or a producer
assertion never fills a missing argument. Identical closed applications are
deduplicated only after all semantic and physical joins succeed; individual
checked-use coordinates remain separate provenance.

A checked generic Omega realization is specialized once for each distinct
closed demanded application through the ordinary authoritative machine-
specialization path. The compiler rechecks the substituted signature,
contracts, effects, target restrictions, admissions, selected provider plan,
and semantic realization. The Terminal coverage fact retains the exact
requirement coordinate, tagged application, strong selected-provider-plan
identity, and one role-tagged realization payload. The closed realization
roles are specialized checked body, nongeneric checked body, compiler
intrinsic, and externally admitted concrete authority. The role discriminant
participates in canonical identity; roles do not share optional template,
specialization, or external-authority fields. D32 separately joins every
surviving executable row to its late physical realization.

The empty telescope is one canonical empty application. It performs no
substitution and is the ordinary cheap path for monomorphic boundary
operators, but it still rejoins the exact selected plan and realization before
authoritative publication. A compiler intrinsic retains its exact closed
execution atom. A bodyless, external, opaque, or separately supplied
realization has no body to specialize and therefore requires independently
admitted concrete authority for each demanded application. Requiring authored
concrete realizations for checked generic bodies would duplicate ordinary
monomorphization and is not an alternate language rule.

Zero-commitment bootstrap lowering is not an authoritative realization role.
An ordinary builtin fallback that selects no boundary requirement emits no
boundary-application coverage row. A resolved boundary operator that still
uses bootstrap lowering may remain in an explicitly non-authoritative
bootstrap profile, but it cannot publish exact realization coverage. An
authority-bearing artifact must route that operation through a selected
checked body, compiler intrinsic, or admitted external realization; it never
fills a plan or specialization field with zero and never silently omits a
demanded boundary application.

Terminal and package replay recompute the closed application and its
role-specific realization commitment from retained structural facts. They
reject open or missing binders, category erasure, const-carrier drift,
application/plan substitution, stale early plans, unsupported binder
categories, and any coverage row produced before its demand was closed. D28's
future universal semantic row remains complementary: it can establish
symbolic selection coverage, but every emitted artifact still carries D29's
finite concrete applications and D32's physical child receipts for the
surviving executable projection.

## D30 — Physical Gamma application profiles and compiler boundaries

One canonical Gamma compilation request is the exact byte sequence

```text
0..7    [47 43 52 45 51 01 00 00]  (`GCREQ`, version 1, reserved)
8..11   application-profile ID, little-endian u32
12..15  Gamma-source byte length, little-endian u32
16..    exact Gamma-source bytes; exact end of request
```

Version 1 assigns `1` to `ConformanceBytesV1` and `2` to
`DeltaCompilerV1`. Zero and every ID unknown to the consuming compiler
artifact reject. The compiler artifact owns the embedded profile registry, so
an older artifact correctly rejects a later ID; adding a value does not by
itself revise the envelope. The exact request bytes, selected registry row,
registry version, compiler artifact, and emitted adapter participate in
compilation identity. Profile metadata is not redundantly repeated in the
request and is never inferred from source, names, paths, or ambient flags.

Both V1 generated applications admit at most 4,194,304 sealed input bytes.
`ConformanceBytesV1` admits at most 4,194,304 successful output bytes;
`DeltaCompilerV1` admits at most 1,048,572 successful tape bytes, the committed
`AlphaBootstrapV2` raw-tape maximum. These are application-profile policies
except for the derived tape maximum. The Beta-written Gamma compiler's own
4,194,304-byte Gamma-source ceiling is a distinct compiler resource even when
the numbers coincide. Generated applications use the committed 15-MiB Gamma
stack and 112-MiB immutable heap. Exact and adjacent refusal canaries accompany
every selected maximum.

Envelope validation precedes Gamma lexing, declaration and type checking,
selected-profile schema checking, and lowering/emission. An unknown or
malformed profile therefore wins before any Gamma-source defect and reports a
`GCREQ`-byte coordinate; an entry or profile-schema mismatch reports its exact
Gamma-source coordinate. `GCOUT` V1 uses magic
`[FF 47 43 4F 55 54 01 00]`; `DCOUT` V1 uses
`[FF 44 43 4F 55 54 01 00]`. Both retain the common 40-byte compiler-failure
frame and halt tags 0 Complete, 1 Reject, 2 Incomplete, and 3 InternalFailure.
Their coordinate spaces are respectively:

```text
GCOUT: 0 none, 1 Gamma source, 2 emitted payload, 3 internal row, 4 GCREQ
DCOUT: 0 none, 1 Delta source, 2 emitted payload, 3 internal row
```

The closed tables in `source/gamma/compiler/gcout-v1.tsv` and
`dcout-v1.tsv` are normative. `GCOUT` exposes authored semantic rejection
classes rather than the compiler's private parser states or former one-bit
failure flag. Its compiler-resource limits are 4,194,304 source bytes, 65,536
total type rows, 65,536 constructor rows, 32,768 function rows, 65,536 active
environment rows, 65,536 coverage rows, 114,294,752 syntax-arena bytes, 1,024
parse levels, 65,535 live local slots, 65,536 labels, 116,508 fixups, and
1,048,572 payload bytes. `DCOUT` retains D17 rejection codes 1 through 26
unchanged and distinguishes its 4-MiB input, 15-MiB stack, 112-MiB heap,
1,048,572-byte output, and D31 application-static-storage resources. Limit and
requested fields carry the exact selected values; changing a producer layout
does not silently renumber the stable resource classes.

`ConformanceBytesV1` is a generated-program observation rather than a compiler
boundary. It publishes stdout only after the complete returned `Bytes` passes
the 4-MiB preflight. Halt 0 publishes exactly that value; every recognized
failure publishes no bytes. The common generated-program block is 248
InternalFailure, 249 AuthoredTrap, 250 StackExhausted, 251
MemoryContainmentViolation, 252 HeapExhausted, 253 InputExtent, and 254
OutputExtent. Alpha's illegal-instruction trap remains 132. Status 255 is
unassigned and noncanonical so a shell's projection of `-1` cannot masquerade
as a declared failure. Divergence has no terminal observation. The temporary
`interp.beta` oracle predates this block; its private 252-through-255 meanings are
interpreted only by its own harness and grant no generated-program authority.

`DeltaCompilerV1` translates generated-runtime conditions into `DCOUT`: input,
stack, heap, output, and a validated D31 storage refusal become Incomplete; a
Gamma trap, memory-containment violation, malformed return or storage payload,
invalid rejection offset, replay disagreement, or adapter contradiction becomes
InternalFailure. A returned `Complete` publishes only the fully preflighted raw
tape, and a returned `Reject` publishes the corresponding D17 frame. No failure
publishes a partial artifact.

`profiles-v1.tsv`, `conformance-observations-v1.tsv`, `gcout-v1.tsv`, and
`dcout-v1.tsv` are checked projections of constants embedded in the compiler
artifact, not runtime host inputs. Gates compare every projected row with the
embedded constants so documentation and executable meaning cannot drift. An
accidental implementation fact may be replaced, a profile fact may change
through a coordinated version revision, and a semantic invariant changes only
for an articulated language reason; settled records do not forbid a better
long-term design.

## D31 — Delta type formation is structural and profile-independent

A Delta array length is admitted exactly in `1..INT32_MAX`. Zero is
`InvalidArrayLength` at its literal; a negative spelling is not a `NAT` and
fails in parsing, while a positive token above `INT32_MAX` is
`IntegerLiteralOutOfRange`. Type formation never consults an Alpha memory map,
host layout, or current compiler capacity. Consequently a valid array may be
too large for one selected application profile without becoming invalid Delta.

An empty `data X {}` is explicitly one zero-field record with one
zero-initialized value. It is never an empty sum. A declaration mixing fields
and cases remains `InvalidDataShape` at its declaration name. This is a Delta
rule, not an inference from Omega's separate `Empty`, `Record`, `Enum`, and
`Mixed` representation categories.

`never` is admitted only as the exact outer machine-return type. Every other
occurrence is `TypeMismatch` at the `never` token. A view is admitted only as
an outer parameter or local type; a forbidden view is `EscapingView` at its
outermost forbidden `&`, and that structural placement failure suppresses
defects nested beneath it. `Console` is a sealed entry capability admitted only
at exact `Main.console`. Every other occurrence belongs to the entry-shape
judgment and is `InvalidEntry`; `InvalidBoundary` remains declaration-
collection-only.

Type formation derives all shape, placement, recursion, and unknown-type
candidates from the complete D22/D24 census and chooses the smallest packed
source coordinate independent of traversal and reason-table order. The
structural anchors above are disjoint. Two distinct reasons at one exact anchor
are a compiler contradiction and therefore `InternalFailure`, never a private
priority rule.

After `CheckDelta` succeeds, target realization expands only storage roots
reachable in the selected application. An unused large type consumes no
application storage. A single reachable expanded array that alone exceeds the
selected static-storage extent produces
`Incomplete(ApplicationStaticStorageBytes, limit, requested)` at that array's
length literal. If individually fitting roots, repeated instances, or several
fields exceed the extent only in aggregate, the same resource uses coordinate
space `none`. The aggregate requested amount is the exact canonical complete
demand rather than a traversal prefix, and both forms require
`requested > limit`.

`DeltaCompileOutcome` therefore adds the exact source-owned constructors
`StorageIncompleteAt(limit, requested, source_offset)` and
`StorageIncompleteTotal(limit, requested)`. The selected D19/D30 adapter checks
their complete payloads and maps them to DCOUT Incomplete resource code 5.
These are the only source-authored Incomplete outcomes; input, stack, heap, and
output exhaustion remain adapter-owned. A malformed limit, request, or source
offset is `InternalFailure(InvalidReturnedOutcome)`, and no refusal publishes a
tape prefix. The adapter has not been published, so D31 revises its V1 schema
and checked table projection in place; a post-publication schema change would
require an explicit profile version.

## D32 — Split semantic boundary coverage from physical realization

Terminal Psi owns the semantic half of D29 boundary-operator coverage. Each
row binds the exact checked use to its source-free operation, closed tagged
application, strong selected-provider-plan identity, and role-tagged semantic
realization. Equal closed applications may share that semantic row while their
checked-use coordinates remain separate provenance. Package and Terminal
replay may publish that semantic evidence
without claiming register assignment, final call placement, relocation, or
emitted bytes that do not yet exist.

A plan may enter Terminal only when its carrier and independent validator can
live at architectural rank `representations` or below without a backend-owned
type, and the fact is reconstructible before assignment and emission from
Terminal semantics, the admitted target schema, and selected plans alone. The
dependency-layer guard enforces carrier ownership; independent reconstruction
enforces that a backend result was not merely renamed or laundered into a low
layer. Schema-derived representation, layout, access, and calling plans may
meet that rule. Assigned registers or stack homes, final placement,
relocations, and emitted-byte custody do not.

Terminal remains immutable through optimization. A verified optimization run
publishes a validated transformed unit or projection retaining the canonical
`TerminalPsiIdentity`; it does not mint a post-optimization Terminal. Each
surviving boundary-operation occurrence carrying a D29 row then receives one
physical child receipt in `NativeArtifact`. Multiple occurrences may share one
semantic parent row, but their optimized-operation identities and physical
children remain distinct. The child binds both the domain-separated strong
identity of its canonical Terminal D29 parent row and its exact surviving
optimized operation/projection identity, then retains the target-lowering,
instruction-selection, assignment, relocation, and emitted-byte-span joins.

Native replay derives the surviving boundary-operation occurrence set from the
published validated optimization projection and requires exact correspondence
with the physical children. Missing, duplicate, stale, substituted, or padded
children reject. An eliminated occurrence needs no child only when the verified
optimization proof establishes that elimination. With no optimization the
projection is the identity projection. Rechecking a byte digest or a self-
consistent physical receipt without both parent bindings establishes no
semantic-to-physical relation.

One realized-artifact envelope may contain the canonical Terminal artifact,
validated optimization projection, and physical children, but their evidence
classes and replay rules remain distinct. A package or semantic claim may stop
at Terminal; a native or external execution claim must recheck the complete
physical relation. D32 introduces no `Chi` stage, backend-to-representation
dependency, or layering exception merely to preserve older wording.

Every strong evidence commitment is domain-separated by a stable schema and
semantic role. A subject, composition joint, Terminal parent, optimized
projection, or physical child commitment is therefore unusable in another
role even when its underlying bytes or compact report coordinates coincide.

## Dependency order

1. finish the Alpha-written Beta compiler edge and common tape boundary;
2. publish the Beta-written Gamma compiler tape;
3. implement and publish the Gamma-written Delta compiler tape;
4. compile the Delta-written Omega source closure `D` into `omega₀`;
5. compile the Omega-written source closure `C` with `omega₀` into `omega`; and
6. optimize or natively realize tapes without changing the semantic chain.

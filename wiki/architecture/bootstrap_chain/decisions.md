# Lattice — ratified decisions

This file records architectural decisions. Current implementation order lives
only in [`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## D1 — Rust exits by role, not by rung

The Rust implementation under `source/omega-rust/` may remain as a comparator,
migration aid, and working product while the lattice closes. It supplies no
semantic authority. Meaning/checking dependencies leave the trusted path before
convenient producers need to disappear.

## D2 — Superseded: the Epsilon-to-Delta meaning route

The former decision made a Gamma-written program parse Epsilon and emit Delta for
evaluation. D11 supersedes it. That route crossed the Delta ownership boundary,
made Gamma an undeclared Epsilon implementation, and hid a bridge compiler behind
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

This decision originally selected the following chain. D61 now treats it as the
implemented comparison baseline rather than a permanent topology:

```text
audited Alpha seed
  → direct Beta assembler tape
  → Beta-written Gamma compiler       → gamma tape
  → Gamma-written Delta compiler       → delta tape
  → Delta-written Epsilon compiler      → epsilon tape
  → Epsilon-written Omega source D      → omega₀ tape
  → Omega-written Omega source C      → omega tape
```

The first arrow executes the direct Beta assembler program; every later arrow
consumes a genuine compiler for exactly the language on its right.
No lower rung parses past its immediate successor. A compiler may use private
internal representations, but no older compiler, interpreter, or source
transpiler remains an external semantic dependency of a later edge.

`D` is the exact Epsilon-written source closure of the first full Omega compiler.
`C` is the exact Omega-written source closure of the self-hosting compiler.
They are different implementations of the same complete Omega language.
`omega₀` may be slow and poorly optimized, but it must compile `C` with exact
Omega semantics. The resulting `omega` may apply the optimizer implemented by
the compiler source.

The three source facts remain distinct:

| Contract | Meaning |
| --- | --- |
| Epsilon v1 | independent compiler-host language used to author `D` |
| features used by `C` | incidental ordinary-Omega subset used to author the self-hosting compiler |
| full Omega | language implemented for users by both Omega compiler implementations |

The Epsilon compiler accepts Epsilon, not Omega. The Omega compiler built from `D`
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
lowered Alpha transition system. The Beta-written Gamma compiler's own
`.beta`-source-to-`.tape` edge is simpler: authoritative assembly encoding
must equal the exact tape. Equal tapes under the same Alpha input and resource
profile have identical deterministic traces in lockstep, so that first edge
does not invent a synchronization function or stuttering rank.

## D10 — Epsilon meaning is independent; spelling and artifact names are explicit

Epsilon v1 is an independent compiler-host language with separately fixed syntax,
static judgments, small-step execution, resources, and observations. Epsilon may
share familiar spelling with Omega, but its language contract is self-contained:
neither the contract nor a checker may consult Omega documentation, a compiler
implementation, or the accepted corpus to decide what Epsilon source means. A
file accepted by both languages acquires only the meaning of the route under
which it is checked.

Epsilon refinement quantifies over a verifier-selected observation profile. The
profile fixes sealed input bytes, exact artifact and diagnostic bytes, and
terminal exit, rejection, trap, exhaustion, and incomplete outcomes. Ambient
filesystem state, environment, time, network, and process spawning are absent.

Each apparent fixed bound becomes exactly one of:

- a source-visible Epsilon semantic bound;
- an explicit resource-profile parameter; or
- a private producer/checker budget whose exhaustion yields `Incomplete` and
  grants no semantic verdict or publishable tape.

The Delta-written Epsilon compiler implements Epsilon-to-Alpha compilation; it does
not define Epsilon. Coverage may land before authority. Authority additionally
requires the checked refinement route selected by D5 and D9.

Bootstrap file names identify format and role. `.beta` is Beta assembly
source, `.proof` is proof-source input to untrusted elaboration, `.gamma`,
`.delta`, `.epsilon`, `.omg`, and `.psi` identify their respective source
languages, and `.tape` identifies canonical Alpha VM bytecode. Artifact base
names remain descriptive. Native realizations are optional target-qualified
containers, never the canonical compiler identity.

## D11 — Alpha tape is the canonical bootstrap artifact

Every required compiler artifact from Gamma through `omega` is one
platform-independent Alpha tape. Each compiler is standalone: it consumes its
own language and emits the next exact tape without invoking an older compiler,
interpreter, assembler, or host script to perform a semantic transformation.
The direct Beta assembler is itself an Alpha tape. It turns Beta text into the
canonical tape format; later compilers emit that format directly.

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

- representative `epsilon → omega₀` or `omega₀ → omega` work has terrible wall
  time, memory, or tape-size behavior after ordinary algorithmic and diagnostic
  cleanup;
- Alpha's instruction set appears too weak or excessively verbose, including
  pressure to add an opcode, widen an encoding, or smuggle a higher-level
  operation into the VM;
- source-to-Alpha certificates or checking time grow prohibitively despite DAG
  sharing, compositional lemmas, and removal of duplicate evidence;
- a special native accelerator, source-pattern substitution, tape-hash shortcut,
  or other jet appears necessary;
- target ABI, object-format, runtime, or hardware behavior leaks into Gamma,
  Delta, or Epsilon instead of remaining at the Alpha realization or Omega
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

## D13 — The Gamma compiler boundary is typed without wrapping its artifact

The Beta-written Gamma compiler returns exactly one of `Complete(tape)`,
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

The exact Alpha-source-to-Gamma-compiler-tape edge remains one owner-fixed root
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

The canonical Gamma checker publishes the finite resource profile certificate
producers must meet. Its Python reference is a diagnostic logical diamond, not
a second resource authority, and it is temporary development scaffolding rather
than a member of the completed bootstrap closure. Measurements bind the exact current subjects and
report conversion scratch, permanently retained proof state, semantic stack,
certificate size, and checking time across candidate chunk counts. Literal
artifact byte counts are observations derived from those subjects, not durable
architecture: editing or golfing either subject requires rebuilding and
rechecking its artifact-owned certificate.

## D15 — Bootstrap implementation source is closed textual ASCII

Beta assembly, Gamma, Delta, and Epsilon source share one outer byte envelope.
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
Delta's current implementation material is Gamma source, and future compiler
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

## D16 — Delta is one typed pure language with an explicit compiler adapter

Delta accepts typed `data* def*` programs with no trailing untyped expression.
It has nominal immutable algebraic data, checked signed 64-bit `Int`, compact
immutable `Bytes`, exhaustive matches, strict left-to-right evaluation, mutual
recursion, and proper tail calls. Functions and constructors have arbitrary
arity; a predecessor register count is an implementation concern rather than a
Delta language limit. Delta exposes no general byte-I/O effect.

`Bytes` is a language primitive because compiler input cannot be represented as
one algebraic node per byte within the rung's realistic memory profile. Its
representation is private and cannot expose storage coordinates. Fuel, source,
heap, stack, and output ceilings are implementation-profile bounds. Their
exhaustion yields `Incomplete`, never a Delta result, rejection, divergence
verdict, or partial artifact.

A Delta compiler application publishes a typed `main` under D19's sealed
application-profile selection. The Epsilon compiler returns `Complete(Bytes)`,
`Reject(EpsilonRejectReason, source_offset)`, or D31/D34's two
application-static-storage refusals. The accepted Epsilon contract owns the
source-declared closed reason sum, while the selected profile owns its explicit
constructor-to-code table; declaration order is not a wire code. The generated
Alpha adapter alone reads sealed input, invokes pure `main`, writes raw success
bytes, and maps checked storage refusal, private exhaustion, or a trap to outer
`Incomplete` or `InternalFailure` cases.

Compiler boundaries share D13's four halt tags and canonical failure-frame
shape, not its Gamma-specific identity. `GCOUT`, `DCOUT`, and `ECOUT` have
edge-owned magic, version, code tables, and coordinates. One parameterized gate
may decode them, but an edge never accepts another edge's frame. Success remains
an unwrapped Alpha tape and every failure publishes no partial artifact.

The strict static frontend is now absorbed into the canonical Gamma-written
compiler source; the Delta interpreter remains a temporary oracle and candidate
algorithm source. Their historical correlated omission of match exhaustiveness
demonstrates the limit of a differential diamond: agreement detects divergence
between implementations but cannot establish a rule both omit. The compiler
frontend rejects incomplete coverage and the interpreter fails loudly on no
match during migration; the completed canonical compiler still owns the static
judgment. It type-checks and emits Alpha tape directly rather than packaging an
interpreter with source syntax.

## D17 — Epsilon v1 is one closed fixed-storage compiler-host language

`source/epsilon/LANGUAGE.md` is the self-contained normative Epsilon v1 contract.
Epsilon shares familiar spelling with Omega but inherits no Omega meaning. Its
source closure is resolved outside the language and packed into one exact
translation unit; top-level forward references ensure that packing order
changes coordinates rather than program meaning.

Checking and execution are distinct judgments. `CheckEpsilon` either accepts one
program or returns one closed `EpsilonRejectReason` and exact packed byte offset.
`RunEpsilon` yields only `Exit`, `Trap`, or actual divergence. `Incomplete` and
`InternalFailure` belong to bounded tools and compiler adapters, never to Epsilon
program semantics, and publish no partial artifact. `ECOUT` owns explicit
versioned constructor-to-code tables independent of Delta declaration order.

V1 reserves only its active syntax and removes contextual or speculative
surface: no packages, imports, attributes, domains, range types, contracts,
`terminates by`, generic parameters, heap, or recursive value types. It retains
finite records and sums, arbitrary finite payload arity, fixed arrays, bounded
non-escaping views, `i32`, storage-only `u8`, return-only `never`, checked
scalar operations, short-circuit Boolean connectives, assertions, machines,
states, transitions, recursion, and one exact receiver-qualified `Console`
boundary. Scalar transition misses trap deterministically.

Because Epsilon has no heap or recursive value type, `D` represents dynamic
compiler structures in source-declared fixed arrays with integer indexes.
Those capacities and their failure behavior are semantic program state. Small
parser, arena, declaration, parameter, or output ceilings inside an
implementation remain private budgets whose exhaustion is outer `Incomplete`,
not a language rejection or limit.

The Delta-written compiler exposes D19's source-owned pure
`main : Bytes -> EpsilonCompileOutcome`. That sum carries success, typed Epsilon
rejection, and D31/D34's two selected-profile static-storage refusals. The sealed
`EpsilonCompilerV1` selection validates the exact nominal schema and complete
reason-code bijection before its adapter alone owns sealed input, the four
compiler halt tags, `ECOUT` framing, and outer resource/internal failures. The
compiler emits an unwrapped Alpha tape and is accepted only by direct checked
Epsilon-source-to-Alpha-tape refinement. Deleted translator behavior and
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

Implementation status: the maintained Rust compiler implements the checkpoint
shape directly. Build admission produces an opaque, consuming carrier with the
prepared program, exact program-bound entry token, reach plans, authority
verdict, initial `Build` snapshot, target inputs, filesystem scope, and sponsor.
Primary execution and replay both enter by that token and must return its exact
selected symbol. Checked orchestration retains the coherent base frontend and
package/source-consumption verdict beside that carrier.

Own generated output is read and parsed once into extension-owned source units.
Each unit independently consumes the ordinary pre-resolution evaluator under
the package selection authority, so normalization cannot synthesize across unit
custody; its matching one-shot post-typing continuation is retained. Seeded
resolution then appends the complete later stratum to the exact resolved base
and rebases only extension-owned authored-selection occurrences. Seeded typing
currently admits the validated generated-data cohorts, ordinary monomorphic
machines, monomorphic attached methods whose `attached_data_symbol` rejoins an
exact retained or newly generated data declaration, and checked-body generic
machines with an authored-order erased-lifetime telescope plus exact machine-
parented ordinary `Type` or const binders whose carrier is scalar or an exact
validated structured-data shape. Structured const occurrences replay the
exact binder symbol/name and matching template carrier. Ordinary `Type`
binders retain their complete authored multiplicity and four-axis carry-
property bounds. The first static-machine cohorts retain an exact machine-
parented binder and body calls targeting it. The structural form owns one flat
parameter/result signature over the same supported type surface. The nominal
form retains one exact ordinary nongeneric trait and requirement pair from
either the authored base or the current generated extension. An
extension-owned trait must be non-boundary, nongeneric, nonempty, and composed
only of flat bodyless requirements over the same supported type surface; the
trait and every requirement retain their exact new symbols, and a nominal
machine binder rejoins that pair by its authored path. Boundary traits, nested
and operational contracts, default bodies, declaration-identity and
proposition binders, generated-machine `satisfies`, conformance bounds, and
broader supply forms remain outside these first generic cohorts.
Satisfied-declaration
settlement, domain constraints, qualification casts, fixed-byte literal
landing, and wire-plan publication are frontier-aware. After post-typing
evaluation, a structural
validator requires every base root graph, symbol/name/path row, authored
selection, semantic table, and compiler-owned sidecar to remain an exact
prefix. After ordinary final checking, package-aware compilation repeats the
authored-declaration authority gate over that exact completed program before
deriving the package subject. The retained pre-build verdict continues to name
the frozen base commitment; generated selections separately rejoin their exact
requesting package and must satisfy the same public-visibility and direct-
dependency rules.

The maintained route now has one `lower_checked_frontend` call. The raw
combined-syntax carrier, rebuild switch, second frontend construction, and
nominal `(source_span, callable_identity)` build-machine rebind have been
deleted. A generated declaration shape outside the admitted continuation
cohort rejects transactionally; it never selects another semantic route.
Target-scoped generated machines likewise reject until extension-aware target
filtering can retain their exact selected-origin and provider-default custody.
This is implementation incompleteness, not a language-design decision.

Canaries retain the exact build symbol and callable identity, target and
subsystem, evaluation/replay usage, observations, canonical source metadata,
generated bytes and staged-tree custody, source counts and commitments, and
package authority evidence. Additional canaries pin one-way overload and
conformance visibility, unit-local normalization, generated-machine and
attached-method continuation, dependency-bundle no-rerun, target-scoped
rejection, retained-prefix preservation, generated transitive-dependency
rejection at the final authority replay, exact generic-machine lifetime-
telescope, Type-property bounds, structural static-machine signature/call
custody, base-owned nominal static-machine requirement/call custody, and
scalar/structured-const carrier/value custody, transactional rejection of
unadmitted binder kinds, and structural absence of the retired rebuild path.

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
V1 wire identity to the future accepted `PackageInstance` carrier. D59 fixes
the remaining inner-table, commitment, failure-order, scalar-publication, and
shared-profile rules needed for one byte-interoperable wire; assigning and
implementing its closed numeric tables remains compiler closure work.

## D19 — Delta application adapters are selected by one sealed two-profile input

Delta source semantics ends at a pure returned value. A runnable Alpha tape may
join that value to sealed input, stdout, halt, and diagnostic framing through a
compiler-generated adapter, but Delta source does not select that environmental
contract. The canonical Delta compilation question therefore contains the
exact source together with one closed application-profile ID. The ID is sealed
invocation input and participates in compilation identity, reconstruction
evidence, and the emitted adapter's custody. It is neither Delta syntax nor an
ambient host flag, filename inference, or post-emission rewrite.

Version 1 has exactly two profiles. `ConformanceBytesV1` requires the
resolved entry `main : Bytes -> Bytes`; its adapter supplies sealed input,
preflights the complete returned `Bytes`, publishes exactly those bytes on
success, and uses its own closed runtime-containment profile without claiming a
compiler boundary. `EpsilonCompilerV1` requires source declarations with the
exact Delta schema:

```text
(data EpsilonCompileOutcome
  (Complete Bytes)
  (Reject EpsilonRejectReason Int)
  (StorageIncompleteAt Int Int Int)
  (StorageIncompleteTotal Int Int))

(def main ((source Bytes)) EpsilonCompileOutcome ...)
```

The rejection `Int` is the exact source byte offset. D31/D34's storage cases carry
`(limit, requested, source_offset)` and `(limit, requested)` respectively and
are the sole source-authored path to outer `Incomplete`.
`EpsilonCompilerV1` generates D17's `ECOUT` adapter: `Complete` publishes the
unwrapped Alpha tape, `Reject` publishes the versioned rejection frame, checked
storage refusal publishes the corresponding resource frame, and adapter-owned
exhaustion or contradiction publishes outer `Incomplete` or
`InternalFailure`. Every failure publishes no artifact prefix. Each profile
owns its exact sealed-input maximum, entry contract, result validation,
external observation profile, and private resource/status table. D21 requires
that maximum to lie in `0..INT64_MAX`, checked as profile metadata before
adapter emission.

`Int` and `Bytes` remain Delta's only built-in types. `EpsilonRejectReason` and
`EpsilonCompileOutcome` remain ordinary source-owned nominal declarations; the
profile does not inject hidden builtins or make structurally similar nominal
types interchangeable. The sealed profile grants the external boundary. Names
such as `main`, `Complete`, `Reject`, or `EpsilonCompileOutcome` grant nothing and
never select a profile.

Before emitting any adapter, the compiler resolves and retains the exact entry,
result type, outcome constructors, and rejection-reason constructors, then
checks them against the already selected profile. The outcome sum has exactly
the required four constructors and payloads. The profile-owned `ECOUT` table is
a checked bijection over the complete source-declared reason sum: every exact
constructor has one unique in-range code and every table row identifies one
exact constructor. Codes never derive from spelling or declaration order. A
schema, table, or entry mismatch is a `DCOUT` compilation rejection and can
never survive as an unhandled emitted-program case. Changing the reason sum or
wire table requires an explicit D17/profile-version decision.

One general Delta compiler implements both profiles. Separately admitted
compiler artifacts per adapter would duplicate custody and refinement for one
checked language; an in-source application declaration would let source choose
an external boundary; and hardwiring the immediate Epsilon customer would prevent
the language's own general conformance use.

## D20 — Delta names resolve through four namespaces without active shadowing

Delta has four semantic namespaces selected by grammar position: types,
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
the relevant namespace. Delta has no function values. These permissions do not
make declarations structurally interchangeable or merge their retained rows.

Parameters, `let` binders, and constructor-pattern binders share the local-value
namespace. A new binder may not duplicate any binding in
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
the source-to-resolved-identity joins in the Gamma-written Delta compiler.

## D21 — Every Delta Bytes value has an Int-representable logical length

Every valid Delta `Bytes` has one exact logical length in `0..INT64_MAX`.
`bytes_empty` and `bytes_single` establish that invariant; `bytes_concat`
preserves it through the checked sum below. Delta has no slicing operation.
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

The overflow is an authored Delta trap because it depends only on the operand
values. A representable concatenation that cannot allocate is profile-owned
`Incomplete`; malformed private descriptors or impossible checked states are
`InternalFailure`. D19 already maps a Delta trap in the generated Epsilon-compiler
application to outer `InternalFailure`, so D21 adds no halt tag or wire outcome.
The Delta guide owns the closed authored-trap condition list; exact published
diagnostic subcodes, if distinguished, remain in the selected edge profile's
versioned reason table.

## D22 — Epsilon names use scoped namespaces and one pre-type duplicate census

Epsilon resolves names through grammar-selected namespaces rather than one
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

Epsilon permits no active local shadowing. Machine parameters are active for the
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
D36 applies the grammar-selected principle to the one owner-qualified callable
position that otherwise admits both a case and machine: those spellings share a
narrow collision registry while their retained declaration rows stay distinct.

## D23 — AlphaBootstrapV2 admits a one-MiB tape through a coherent checker profile

The canonical bootstrap execution profile advanced from the former 256-KiB
seed hole to `AlphaBootstrapV2`. Its seed hole is exactly 1,048,576 bytes,
including the four-byte stamped tape length, so its maximum raw Alpha tape is
1,048,572 bytes. This is one global lattice profile, not a Delta-only exception.
It changes no Alpha opcode, encoding, or execution rule: runnable-tape capacity
remains an execution-profile fact rather than Alpha language semantics.

The profile revision is atomic across every owner of the old extent. Both
platform seeds and their stamping paths, the Gamma compiler's payload storage
and generated-program memory maps, adjacent compiler-profile ceilings including
the procedure table, Delta's emitted-program stack and heap boundaries, the
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

The measured Delta compiler pressure establishes that the old ceiling is a
lattice constraint, not permission for a weakened compiler, split compiler,
host helper, Delta-specific jet, or deleted conformance gate. General Gamma code
density improvements remain ordinary quality work and must preserve the fixed
gate set, but no further density pass gates V2. Per-feature size budgets do not
replace the profile revision.

D14's bounded equality lemmas and composition do not yet authorize a paged root
subject representation. A later checker revision may make subject custody
chunk-addressable and thereby break the current roughly linear tape-to-arena
coupling, but that is a separately specified checker/input change rather than an
unstated part of V2. Real Delta and Epsilon artifacts are measured under V2 before
any further capacity revision; continued pressure first reopens subject custody,
not the Alpha instruction set.

## D24 — Epsilon transition binders complete the scoped identity census

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

The Epsilon-written and self-hosted Omega compilers consume one byte-identical
`OCREQ` version-1 question. Its eight-byte identity is
`[4F 43 52 45 51 01 00 00]` (`OCREQ`, version 1, two reserved zero bytes),
followed by little-endian `u32` subject and invocation byte lengths, then those
two exact sections and exact end. Every variable byte value is length-framed;
every table has an explicit count; closed variants have fixed numeric tags and
zero reserved fields. Counts, lengths, and indices are at most `INT32_MAX` so
both Epsilon and Omega implementations can represent them. A Epsilon decoder reads
the four raw bytes, rejects a set high bit before signed conversion or checked
arithmetic, and therefore never turns hostile framing into a trap and outer
`InternalFailure`.

The identity, outer section extents, exact end, and canonical semantic contents
in this decision are fixed. D59 governs the one flat inner profile, exact
commitment preimage, failure ordering, and bounded numeric publication. Its
closed checked tables must be assigned and implemented before either compiler
may claim a full V1 decoder or publisher.

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
closed shape, physical movement, role-tagged lifecycle disposition,
representation version and origin, closed-conformance commitment, and complete
boundary-calling-plan commitment. A foreign demand must rejoin the producer's
exact canonical rows and selected immutable source instance. Names, compact report fingerprints,
lockfile strings, and review prose are never agreement.

The compiler records the exact carrier shape root at the moment each opaque is
materialized. Review groups all occurrences of one opaque under the exact
target-closed boundary requirement application and retains, for each
occurrence, its parameter/result role, path through the complete checked shape
graph, and replay-validated placement. Structural equality, carrier size, or
alignment cannot identify an occurrence. The marker and shape root enter the
strong boundary-plan application commitment. The compact report fingerprint
remains diagnostic compatibility data and is not canonical demand evidence.

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

## D29 — Compiler-issued exact boundary-operator-application coverage

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
and semantic realization. Target-neutral Terminal retains the exact checked
operator occurrence, requirement coordinate, and tagged application as a
demand. The strong selected-provider-plan identity and one role-tagged
realization payload live in an exact representation-rank companion bound to
that Terminal product; coverage exists only after replay rejoins both. The
closed realization roles are specialized checked body, nongeneric checked
body, compiler intrinsic, and externally admitted concrete authority. The role
discriminant participates in canonical identity; roles do not share optional
template, specialization, or external-authority fields. D32 separately joins
every surviving executable row to its late physical realization.

The empty telescope is one canonical empty application. It means that a
boundary operator has a real static telescope of length zero; it never stands
for an ordinary boundary-trait machine, which has no telescope construct. It
performs no substitution and is the ordinary cheap path for monomorphic
boundary operators, but it still rejoins the exact selected plan and
realization before authoritative publication. A compiler intrinsic retains
its exact closed execution atom. A bodyless, external, opaque, or separately supplied
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

Coverage and package replay recompute the closed application and its
role-specific realization commitment from the Terminal demand and bound
companion. They
reject open or missing binders, category erasure, const-carrier drift,
application/plan substitution, stale early plans, unsupported binder
categories, and any coverage row produced before its demand was closed. D28's
future universal semantic row remains complementary: it can establish
symbolic selection coverage, but every emitted artifact still carries D29's
finite concrete applications and D32's physical child receipts for the
surviving executable projection.

## D30 — Physical Delta application profiles and compiler boundaries

One canonical Delta compilation request is the exact byte sequence

```text
0..7    [44 43 52 45 51 01 00 00]  (`DCREQ`, version 1, reserved)
8..11   application-profile ID, little-endian u32
12..15  Delta-source byte length, little-endian u32
16..    exact Delta-source bytes; exact end of request
```

Version 1 assigns `1` to `ConformanceBytesV1` and `2` to
`EpsilonCompilerV1`. Zero and every ID unknown to the consuming compiler
artifact reject. The compiler artifact owns the embedded profile registry, so
an older artifact correctly rejects a later ID; adding a value does not by
itself revise the envelope. The exact request bytes, selected registry row,
registry version, compiler artifact, and emitted adapter participate in
compilation identity. Profile metadata is not redundantly repeated in the
request and is never inferred from source, names, paths, or ambient flags.

Both V1 generated applications admit at most 4,194,304 sealed input bytes.
`ConformanceBytesV1` admits at most 4,194,304 successful output bytes;
`EpsilonCompilerV1` admits at most 1,048,572 successful tape bytes, the committed
`AlphaBootstrapV2` raw-tape maximum. These are application-profile policies
except for the derived tape maximum. The Gamma-written Delta compiler's own
4,194,304-byte Delta-source ceiling is a distinct compiler resource even when
the numbers coincide. Generated applications use the committed 15-MiB Delta
stack and 112-MiB immutable heap. Exact and adjacent refusal canaries accompany
every selected maximum.

Envelope validation precedes Delta lexing, declaration and type checking,
selected-profile schema checking, and lowering/emission. D33 fixes the bounded
suborder: fixed header and profile selection precede the declared source
provision, while body exact-end validation occurs only after that provision is
admitted. An unknown or malformed profile therefore wins before any Delta-
source defect and reports a `DCREQ`-byte coordinate. Schema rejections use
D33's category priority and truthful source-or-none coordinates. `DCOUT` V1
uses magic
`[FF 44 43 4F 55 54 01 00]`; `ECOUT` V1 uses
`[FF 45 43 4F 55 54 01 00]`. Both retain the common 40-byte compiler-failure
frame and halt tags 0 Complete, 1 Reject, 2 Incomplete, and 3 InternalFailure.
Their coordinate spaces are respectively:

```text
DCOUT: 0 none, 1 Delta source, 2 emitted payload, 3 internal row, 4 DCREQ
ECOUT: 0 none, 1 Epsilon source, 2 emitted payload, 3 internal row
```

The closed tables in `source/delta/compiler/dcout-v1.tsv` and
`ecout-v1.tsv` are normative. `DCOUT` exposes authored semantic rejection
classes rather than the compiler's private parser states or former one-bit
failure flag. Its profile-context column is checked against the originating
request; the detached frame does not carry that context. Its compiler-resource
limits are 4,194,304 source bytes, 65,536
total type rows, 65,536 constructor rows, 32,768 function rows, 65,536 active
environment rows, 65,536 coverage rows, 114,294,752 syntax-arena bytes, 1,024
parse levels, 65,535 live local slots, 65,536 labels, 116,508 fixups, and
1,048,572 payload bytes. `ECOUT` retains D17 rejection codes 1 through 26
unchanged and distinguishes its 4-MiB input, 15-MiB stack, 112-MiB heap,
1,048,572-byte output, and D31/D34 application-static-storage resources. Limit
and requested fields carry exact selected values except that D34 explicitly
defines the storage requested amount as a bounded canonical witness. Changing
a producer layout does not silently renumber the stable resource classes.

`ConformanceBytesV1` is a generated-program observation rather than a compiler
boundary. It publishes stdout only after the complete returned `Bytes` passes
the 4-MiB preflight. Halt 0 publishes exactly that value; every recognized
failure publishes no bytes. The common generated-program block is 248
InternalFailure, 249 AuthoredTrap, 250 StackExhausted, 251
MemoryContainmentViolation, 252 HeapExhausted, 253 InputExtent, and 254
OutputExtent. Alpha's illegal-instruction trap remains 132. Status 255 is
unassigned and noncanonical so a shell's projection of `-1` cannot masquerade
as a declared failure. Divergence has no terminal observation. The temporary
`interp.gamma` oracle predates this block; its private 252-through-255 meanings are
interpreted only by its own harness and grant no generated-program authority.

`EpsilonCompilerV1` translates generated-runtime conditions into `ECOUT`: input,
stack, heap, output, and a validated D31/D34 storage refusal become Incomplete; a
Delta trap, memory-containment violation, malformed return or storage payload,
invalid rejection offset, replay disagreement, or adapter contradiction becomes
InternalFailure. A returned `Complete` publishes only the fully preflighted raw
tape, and a returned `Reject` publishes the corresponding D17 frame. No failure
publishes a partial artifact.

`profiles-v1.tsv`, `conformance-observations-v1.tsv`, `dcout-v1.tsv`, and
`ecout-v1.tsv` are checked projections of constants embedded in the compiler
artifact, not runtime host inputs. Gates compare every projected row with the
embedded constants so documentation and executable meaning cannot drift. An
accidental implementation fact may be replaced, a profile fact may change
through a coordinated version revision, and a semantic invariant changes only
for an articulated language reason; settled records do not forbid a better
long-term design.

## D31 — Epsilon type formation is structural and profile-independent

A Epsilon array length is admitted exactly in `1..INT32_MAX`. Zero is
`InvalidArrayLength` at its literal; a negative spelling is not a `NAT` and
fails in parsing, while a positive token above `INT32_MAX` is
`IntegerLiteralOutOfRange`. Type formation never consults an Alpha memory map,
host layout, or current compiler capacity. Consequently a valid array may be
too large for one selected application profile without becoming invalid Epsilon.

An empty `data X {}` is explicitly one zero-field record with one
zero-initialized value. It is never an empty sum. A declaration mixing fields
and cases remains `InvalidDataShape` at its declaration name. This is a Epsilon
rule, not an inference from Omega's separate `Empty`, `Record`, `Enum`, and
`Mixed` representation categories.

`u8` is admitted as stored data and as a nested array/view element, but not as
a standalone parameter, local, or return; a forbidden occurrence is
`TypeMismatch` at its type token. `never` is admitted only as the exact outer
machine-return type. Every other occurrence is `TypeMismatch` at the `never`
token. A view is admitted only as an outer parameter or local type; a forbidden
view is `EscapingView` at its outermost forbidden `&`, and that structural
placement failure suppresses defects nested beneath it. `Console` is a sealed
entry capability admitted only at exact `Main.console`. Every other occurrence
belongs to D56's entry-shape judgment and is `InvalidEntry`; `InvalidBoundary`
remains declaration-collection-only.

Type formation derives all shape, placement, recursion, and unknown-type
candidates from the complete D22/D24 census and chooses the smallest packed
source coordinate independent of traversal and reason-table order. The
structural anchors above are disjoint. Two distinct reasons at one exact anchor
are a compiler contradiction and therefore `InternalFailure`, never a private
priority rule.

After `CheckEpsilon` succeeds, target realization expands only storage roots
reachable in the selected application. An unused large type consumes no
application storage. A single reachable expanded array that alone exceeds the
selected static-storage extent produces
`Incomplete(ApplicationStaticStorageBytes, limit, requested)` at that array's
length literal. If individually fitting roots, repeated instances, or several
fields exceed the extent only in aggregate, the same resource uses coordinate
space `none`. D34 fixes deterministic selection among excessive arrays and the
bounded canonical meaning of `requested`; no traversal prefix defines either
form, and both require `requested > limit`.

`EpsilonCompileOutcome` therefore adds the exact source-owned constructors
`StorageIncompleteAt(limit, requested, source_offset)` and
`StorageIncompleteTotal(limit, requested)`. The selected D19/D30 adapter checks
their complete payloads and maps them to ECOUT Incomplete resource code 5.
These are the only source-authored Incomplete outcomes; input, stack, heap, and
output exhaustion remain adapter-owned. A malformed limit, request, or source
offset is `InternalFailure(InvalidReturnedOutcome)`, and no refusal publishes a
tape prefix. The adapter has not been published, so D31 revises its V1 schema
and checked table projection in place; a post-publication schema change would
require an explicit profile version.

## D32 — Split semantic boundary evidence from physical realization

Terminal Psi owns target-neutral boundary demands, not target acceptance or
native settlement. For D29 boundary operators it retains the exact checked use,
source-free operation, and closed tagged application; the selected plan and
role-specific semantic realization remain in the strongly bound product
companion. For D41 ordinary boundary-trait invocations it retains the exact
`BoundaryCall`; the consuming lowerer creates a settlement only after rejoining
that demand and companion proposal to its own target catalog and admissions.
Neither form claims register assignment, final call placement, relocation, or
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
surviving executable boundary occurrence then receives one physical child
receipt in `NativeArtifact`. Its parent is the closed role-tagged
`PhysicalChildParent` sum: `OperatorApplicationCoverageRef` names
reconstructible D29 coverage, while `BoundaryTraitSettlement` retains the D41
settlement first created by consuming-lowerer admission. The parent role enters
canonical identity. Multiple occurrences may share one D29 semantic parent,
but their optimized-operation identities and physical children remain
distinct. The child binds both the domain-separated strong parent identity and
its exact surviving optimized operation/projection identity, then retains the
target-lowering, instruction-selection, assignment, relocation, and emitted-
byte-span joins.

The D41 payload reuses the representation-rank `BoundaryExecutionBinding` sum:
`AdmittedProvider(ProviderExecutionBinding)` or
`CompilerBuiltin(CompilerBuiltinExecution)`. It introduces no parallel role
tag. `CompilerBuiltinExecution`, not the planner-rank
`CompilerIntrinsicExecutionIdentity`, enters the settlement and parent
commitments. Conversion from a planner classification into this lowerer
catalog is one exhaustive `match` returning
`Option<CompilerBuiltinExecution>`; the tested planner role and returned
payload are never separate expressions, and unknown roles fail closed.

Because D41 settlement is first created at consuming-lowerer admission, the
native artifact retains its complete canonical content rather than only its
commitment: the Terminal occurrence, selected-plan commitment, target/catalog
identity, execution binding, realization, and role-specific custody inputs.
Replay treats those bytes as inputs, rejoins them to the Terminal product and
bound companion, and independently validates the receiving target catalog or
admitted provider custody. D29 may retain a reference because its complete
coverage is reconstructible from already-published demand and companion facts.

Native replay derives the surviving boundary-operation occurrence set from the
published validated optimization projection and requires exact correspondence
with the physical children. Missing, duplicate, stale, substituted, or padded
children and parent-role substitution reject. An eliminated occurrence needs
no child only when the verified optimization proof establishes that
elimination. With no optimization the projection is the identity projection.
Rechecking a byte digest or a self-consistent physical receipt without both
parent bindings establishes no semantic-to-physical relation.

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

## D33 — DCOUT admission and schema diagnosis are bounded and total

The standalone Gamma-written Delta compiler decides `DCREQ` in one bounded
order. It first obtains the fixed 16-byte header, reporting a truncated header
at its first missing request byte. It then validates magic, version, and
reserved bytes, reporting the first differing byte; validates the complete
profile ID, anchoring `unknown_profile` at byte 8; and compares the declared
little-endian `u32` source length with the selected profile's provision,
anchoring `Incomplete(source_bytes)` at byte 12 with the declaration as
`requested`. Only an admitted length causes the compiler to read that many
body bytes and one exact-end probe. Early EOF reports the first missing body
byte and one extra byte reports the first trailing byte as
`malformed_request`.

The length provision therefore precedes body exact-end validation. A four-byte
length may not force the compiler to consume an attacker-selected extent merely
to reject it, and a profile-specific limit cannot be consulted before the
profile is known. The admitted body plus one-byte probe is the maximum request
body work required before Delta source processing.

After the ordinary Delta frontend has accepted the source, selected-profile
schema diagnosis uses the fixed category order `missing_entry` (19),
`entry_schema_mismatch` (20), then `profile_schema_mismatch` (21). Category
priority precedes coordinate order. Within one category, missing required facts
using coordinate space `none` precede located defects; located defects use the
smallest Delta-source coordinate. Exact-coordinate ties produce the same public
outcome rather than exposing validator traversal.

No `main` is code 19 with coordinate space `none` and coordinate zero. A
present `main` with the wrong selected-profile signature is code 20 at the
`main` declaration name. For `EpsilonCompilerV1`, an entirely absent required
nominal type, constructor, or rejection reason is code 21 with space `none`; a
present malformed declaration, constructor payload, outcome shape, or
reason-to-ECOUT-code bijection is code 21 at its declaration or constructor
name. `ConformanceBytesV1` has no nominal profile schema beyond
`main : Bytes -> Bytes`, so it cannot emit code 21. A missing type mentioned by
source remains the earlier ordinary `unknown_type` rejection, not a schema
candidate.

The normative `dcout-v1.tsv` records allowed coordinate spaces and request-
profile contexts. `unselected` denotes a failure before one valid profile is
known. `malformed_request` is legal while unselected or after either profile
has been selected; `unknown_profile` is legal only while unselected; code 21 is
legal only for profile 2. Every later code lists its selected profiles. The
request/outcome join, not a detached DCOUT decoder, checks that availability
because the 40-byte frame does not repeat the profile ID. A code impossible for
the originating request is a noncanonical compiler result, not an authored
rejection.

The frame layout and version do not change. Space 0 and coordinate zero were
already the canonical representation of no coordinate; D33 revises the checked
table projection because codes 19 and 21 previously claimed only Delta-source
coordinates. The completed compiler retains all schema `(reason, coordinate)`
candidates and applies the ordering above. A Boolean validator, one shared
`FAIL_OFF`, adapter-emission order, or first encountered row cannot define the
public wire result.

## D34 — Epsilon storage refusal uses a bounded canonical witness

Epsilon array and composition validity remains independent of an application
profile. A valid reachable static-storage demand may exceed Delta's signed
64-bit `Int` and ECOUT's fixed scalar, so D31's earlier universal-exactness
wording is narrowed rather than imposing a target-dependent Epsilon type limit.
The selected application-static-storage limit must be in
`0..INT64_MAX-1`. For this resource alone, `requested` is
`min(exact_demand, INT64_MAX)`: exact when the complete mathematical demand is
representable as nonnegative Delta `Int`, and the canonical exceeded-demand
witness `INT64_MAX` otherwise. Exact `INT64_MAX` and larger demands are
observationally equivalent because both exceed every admissible limit and
produce the same admission, coordinate, and no-publication result.

Storage calculation uses the private closed domain
`Exact(nonnegative Int) | Overflowed`. Before adding `a + b`, the compiler
tests `a > INT64_MAX - b`. Before multiplying `a * b`, it handles either zero
factor as exact zero and then tests `a > INT64_MAX / b`; only a passing pair is
multiplied. The zero guard is mandatory before division because zero-field
records can contribute zero-sized components. Addition involving `Overflowed`,
and multiplication involving it with no exact-zero factor, remains
`Overflowed`; a known zero factor still produces exact zero. The calculation
never executes a trapping Delta operation and never turns capacity analysis
into `InternalFailure`.

Attribution exists exactly when one reachable expanded array occurrence alone
exceeds the selected limit. Among nested excessive occurrences the outermost
one wins; among disjoint candidates the smallest packed Epsilon-source
coordinate wins. The result anchors at that occurrence's own length literal,
not at a private arithmetic crossing. Record-field composition, sum layout,
repeated roots, and cross-declaration totals that exceed only through
composition are aggregate and use coordinate space `none`.

D34 changes no source-owned outcome constructor, ECOUT resource code, 40-byte
frame field, or frame version. Bytes 10 and 11 remain zero-reserved. The
adapter validates the exact selected limit, `requested > limit`, and the
coordinate shape; compiler correctness establishes the canonical witness.
Every refusal remains outer `Incomplete` and publishes no tape prefix. A
canonical arbitrary-precision tail or distinct overflow code requires a
future version only if an actual consumer needs to distinguish exact
`INT64_MAX` from a larger demand.

## D35 — Provider assertions are not indexed-application coverage

The pre-D29 indexed-provider application subsystem is retired. Its display
identity plus arity, normalized string arguments, substitutions, provider-
asserted generic or exact families, closure hashes, selected-plan attachment,
and package-review `NonGeneric` / `ExactApplications` rows do not establish
that a selected realization implements a demanded application. Production has
no issuer for those assertions; matching one to a compiler demand would prove
only that two descriptions agree.

D29's compiler-derived tagged type/const applications remain the sole
application demand vocabulary. `Demand` means a requirement reconstructed by
the compiler or consumer; `Claim` or `Candidate` means a provider-supplied
statement; `Coverage` is reserved for an independently reconstructed relation
that rejoins the exact demand, selected role-specific semantic realization,
and its successful recheck. An empty telescope is one canonical empty
application, not proof of coverage. Generic provider-family review therefore
remains fail-closed until final specialization can reconstruct that complete
join.

Delete the indexed-provider module, attachment and fingerprint surfaces,
planning rejoin, package-review field and encoding, and fixtures built around
the obsolete schema. Useful malformed-order and substitution cases may be
rewritten against D29's tagged arguments and artifact-qualified binders, but
the arity/string schema does not survive in production or tests. A demand-only
substitution utility may be introduced only for a real final-composition
consumer and may neither attach to a provider nor be serialized as coverage.

The device-operation carrier is independent foundation-layer scaffolding for
runtime custody protocols, not a dependency of the representation-layer
indexed-provider model and not a D29 static-application carrier. D27 continues
to retain it. Its provider-authored `Coverage` terminology must become
`Claim` or `Candidate`; this naming correction does not grant authority or
change the D27 lifecycle.

Removing the canonical package-review field advances the next evidence schema
from 96 to 97 and row schema from 54 to 55. New encoders never emit the retired
row. Package-review and obligation-ledger recovery remain current-version-only:
current admission and decoding reject earlier vocabularies. Historical bytes
may be retained opaquely and interpreted by their corresponding old toolchain
or a separately authorized migration tool, but no legacy row or decode-and-
discard parser enters the current record model.

## D36 — Epsilon callable spelling is single-valued before body resolution

D51 narrows D36's original callable partition. An unqualified machine accepts
ordinary parameters and is invoked as `name(...)`. An owner-qualified data
machine requires `&mut self` as its first input and is invoked only through a
typed receiver postfix such as `value.name(...)`. A data-owner
`Owner::name(...)` is constructor syntax. These grammar-selected positions,
not one cross-kind declaration registry, make callable spelling single-valued.

The grammar for an unqualified machine accepts no receiver, so an authored `&`
there is `UnexpectedToken` at that byte. A qualified declaration requires its
receiver immediately after `(`; `)` or an ordinary parameter there is
`UnexpectedToken` at that token. No form infers `Main`, global storage, or a
receiver from later use. A qualified machine with an unknown owner remains
later `UnknownName` at the owner spelling.

Every machine application requires an authored argument list, including `()`
for a zero-parameter free machine or receiver method. Constructors retain their
optional payload-list syntax and resolve only in the case namespace. A case and
receiver method may share `(owner, name)` because their invocation positions
are disjoint; fields remain independently position-qualified and the existing
field/case member collision rule still applies.

`InvalidControlTarget` remains only a continuation judgment. D50 completes its
callable-category cases: a uniquely resolved constructor used as a
continuation, a known bare state, a known bare unqualified machine, and an
unqualified state/machine collision all reject at the continuation expression
start. These are distinct causes sharing one public judgment, not separately
observable outcome classes. Expected result, arity, declaration order, and
preferred-namespace lookup never choose among callable identities.

## D37 — Epsilon body diagnostics follow a complete premise DAG

Epsilon body/control checking visits every authored child and derives every
independent child candidate, but a failure candidate or absent semantic fact
does not satisfy a parent premise. No error type, guessed value, place, or
callable is manufactured for recovery. A parent success or rejection candidate
exists exactly when every fact consumed by that rule has resolved. Thus a
binary scalar rule consumes both operand results, while arity consumes an
admitted resolved callable and argument count but not the independently checked
argument facts. An implementation may reduce derived candidates online to the
smallest coordinate; it need not physically retain candidates that cannot win.

The callable path is a DAG rather than a traversal chain. Callable resolution
feeds context/category admission, whose admitted result feeds arity; authored
argument expressions are checked as a sibling branch from the application
syntax. Argument-type comparison additionally consumes arity success and
complete argument facts; only that join produces the call result. An
inadmissible callable category contributes `InvalidControlTarget` or
`TypeMismatch` and blocks arity/type derivation without suppressing child
argument checking. Wrong arity may coexist with a failing argument, but not
with an unresolved callee or inadmissible category.

Every complete expression result is a value with type and optional place,
resultless, or `never`. Place checking consumes only a complete value result: a
value without a place is `InvalidPlace`, while `missing() = 0` has no value fact
and therefore contributes only its callee `UnknownName`. Resultless in a
value-required position is `TypeMismatch`; a `never` call outside its exact
terminal position, or a later executable construct in the same block, is
`InvalidTerminal`.
Immutable views and other nonassignable values need no separate mutability row;
they simply carry no place.

Projection likewise consumes complete premises. An absent member is
`UnknownName` at the member spelling. A known member of the wrong kind, a known
contextual member on an unsupported receiver, or a non-`i32` index/bound is
`TypeMismatch` at the enclosing postfix expression start. This failure rule
also governs D38's complete non-array `.as_slice` receivers; D38 separately
requires a place fact from an otherwise supported fixed-array receiver.

Operator, call, constructor, and postfix relational failures anchor at the
enclosing expression start; arity at the application start; `InvalidPlace` at
the left-side place start; and let, assignment, assert, return, and transition
value relations at their initializer, assigned value, asserted expression,
returned expression, or subject. A required absent return value anchors at the
`return` keyword. Control and terminal failures anchor at their continuation or
mispositioned `never` call. D53 supersedes the former first-following-statement
anchor with that construct's terminating delimiter.

Candidates merge only by the existing body-phase smallest packed coordinate.
Duplicate derivation of one reason/coordinate is one candidate. No two
simultaneously derivable distinct reasons may share one coordinate by
construction. Finding such a pair is an internal compiler contradiction and
yields outer `InternalFailure`; reason-code order never breaks the tie. Runtime
short-circuiting, transition selection, and compiler traversal do not suppress
static checking of authored children or arms.

## D38 — Epsilon `.as_slice` borrows only a place-valued fixed array

Epsilon's field-like contextual `.as_slice` postfix is admitted only when its
complete receiver result is a fixed-array value carrying a place. It evaluates
that receiver exactly once and produces a non-place immutable `&[T]` view over
the exact full range `0..N`. The operation allocates and copies nothing,
performs no bounds check, and cannot trap. This is the allocation-free bridge
from owning fixed storage to the view surface; an existing view is used
directly and owns no redundant identity conversion.

A fixed-array value without a place contributes `InvalidPlace` at the receiver
expression's start. Every other complete receiver type, including an immutable
view, contributes `TypeMismatch` at the enclosing postfix expression's start.
An unresolved receiver produces no `.as_slice` candidate. These are applications
of D37's premise DAG: the first rule consumes a complete value and its required
place fact, while an absent receiver result cannot create a parent failure.
Consequently `f().as_slice = x` produces only the receiver `InvalidPlace`; the
failed postfix has no result from which assignment-place checking could derive
a second candidate.

The accepted spelling is the `.as_slice` postfix itself. A following `()` is a
separate syntactic call suffix, so `array.as_slice()` parses as
`(array.as_slice)()` and rejects when it attempts to call the resulting view.
Authored record fields named `as_slice` remain ordinary fields because
contextual selection happens only after base-type classification. A computed
place such as `arrays[index()].as_slice` remains useful and evaluates the
effectful receiver exactly once; admitting place receivers does not create a
temporary-storage or lifetime rule for returned array values.

This matches the owning-container boundary in Omega core: `Array::as_slice`
adapts fixed storage to the borrowed `Slice` surface, while `Slice` itself owns
no `as_slice` identity operation. Epsilon remains independently normative, but it
does not broaden that member set in the rung intended to be Omega-shaped.

## D39 — TerminalTraceV1 observes exact maximal semantic traces

`TerminalTraceV1` is the first reusable Terminal observation schema. An
observation profile is a static definition of what behavior is compared, not a
report filled in by one execution. An execution has one maximal semantic trace;
an artifact certificate proves refinement under the independently reconstructed
profile. Equal profile identities establish only that two proofs use the same
observer. They never establish behavior equality without the exact
subject-bound refinement proof.

The trace domain is termination-sensitive and ordered:

```text
ExternalEvent* Return(exact semantic value)
ExternalEvent* Crash(Trap | Abort)
ExternalEvent* ExternalTerminate(effect identity, exact semantic arguments)
an infinite maximal execution, represented coinductively by its finite
observable prefixes
```

Unit return is a distinct value-free `Return`. A silent infinite reduction and
an infinite event-producing reduction are both semantic divergence, not fuel
exhaustion. A step simulation may preserve their maximal traces without proving
global termination or deciding which outcome occurs for every input. Missing
ranking evidence, `Unknown`, and `NoFiniteGuarantee` therefore create no
execution outcome and no profile row.

The version-1 profile instance is reconstructed from the exact canonical
Terminal module and contains, in closed order:

1. a domain tag, schema version, Terminal vocabulary marker, and exact module
   semantic commitment;
2. one root row naming the entry, its ordered scalar and structural input
   schemas, and its Unit/scalar/structural result comparison schema;
3. crash-site rows ordered by `(machine, block, edge)`, each retaining the
   closed cause;
4. ordinary external-event rows ordered by `(machine, block, operation)`, each
   retaining the event kind, exact public boundary or service identity, ordered
   argument schemas, and result schema; and
5. external-termination rows ordered by their exact Terminal site, public
   effect identity, and argument schemas.

The canonical identity encoding begins with the domain separator
`omega.terminal.observation-profile.v1`, uses fixed little-endian numeric
coordinates and length-prefixed canonical identities, and includes explicit row
tags and counts in the order above. The root row makes every valid profile
nonempty. Unknown schema versions, vocabulary markers, row tags, operation
classifications, malformed ordering, duplicate rows, missing observable sites,
extra sites, and empty profiles reject during decoding or reconstruction. A
consumer selects the typed schema and may retain an authenticated expected
instance commitment; the verifier derives the instance independently. A proof
producer supplies neither rows nor weakening flags. Cross-profile reuse remains
closed until a checked canonical forgetting projection exists.

The static profile carries semantic type and comparison rules. Runtime traces
carry the actual returned, argument, and result values, and refinement compares
those values exactly. A digest may be a compact report coordinate only under an
explicit commitment/collision admission; digest equality never silently
replaces semantic value equality. Terminal machine/block/operation/edge sites
are proof and correspondence coordinates. They do not become user-visible
trace values unless a separate language contract explicitly observes a
location.

The bounded runtime comparator covers scalar schemas and exact whole-root
structural schemas. Scalar operands must exactly match the verifier-derived
Boolean, fixed-integer, binary32, or binary64 type; malformed fixed-width
integer values and address-carrier schemas reject rather than compare. Boolean
and fixed-integer values compare exactly, while IEEE values compare their
retained interchange bits so signed zero and NaN payload identity are
preserved. Structural operands require the exact structural type, canonical
complete required qualification rosters, empty runtime paths, and complete
opaque value identity. Projected qualifications and nested runtime values
remain outside this rung. These comparison results grant no trace or refinement
authority.

Version 1 classifies every ordinary `BoundaryCall` and direct semantic service
operation such as `PortWrite` as an ordered external event. Every new Terminal
operation variant must be classified explicitly as internal, ordinarily
observable, or terminal-external under a known profile version; no default or
unknown classification is accepted.

External termination is a source-semantic effect, not the inference that a
provider happens not to return. A result type of `never` proves only absence of
normal return and cannot distinguish successful termination from divergence or
crash. The checked boundary contract and Terminal declaration must retain a
closed `TerminatesExternally(effect_identity)` completion kind, and invocation
must lower as a terminal transfer rather than an ordinary `BoundaryCall` with a
fictional successor. The selected provider and target realization consume that
retained fact. Spelling, provider name, syscall choice, or backend convention
never manufactures it.

The current Omega `Console::exit_process(i32) -> Unit` path is therefore a
physical migration slice, not complete semantic external-termination authority.
The checked interpreter still recognizes `exit_process` by spelling, Terminal
retains an ordinary Unit boundary call, and native lowering later introduces a
nominally nonreturning `ExitProcessI32` operation plus containment trap. Those
paths remain diagnostic/implementation evidence until the boundary contract,
checked trees, Terminal codec/verifier/interpreter, and target join carry the
same explicit terminal-effect identity end to end.

Fixed-fuel exhaustion, evaluator timeouts, and producer/checker `Incomplete`
remain consumer or product results and are not Terminal meaning. Source
diagnostics, emitted artifact bytes, and compiler request/resource outcomes
belong to compiler-product subjects; formal-target-to-silicon evidence belongs
to deployment. D10's Epsilon compiler profile composes `TerminalTraceV1` with
sealed input, exact diagnostic/artifact bytes, and its closed product outcomes
rather than contradicting or replacing the reusable trace profile.

## D40 — FloatMeaning equality is proof-only structural correspondence

`Float::meaning32` and `Float::meaning64` project one exact IEEE runtime carrier
into the proof-only `FloatMeaning` sum. NaN payload erasure occurs in this
projection: the public sum has one payloadless `NaN` case. Equality therefore
uses ordinary structural equality of the sum. `NaN` is reflexive, while
`Zero(Negative)` and `Zero(Positive)` remain distinct. No float-specific
payload-erasure rule enters the proof kernel, and this relation is separate
from atomic IEEE `==`/`!=`, where NaN is non-reflexive and signed zeros compare
equal.

The kernel retains one closed proof-value classification, initially with a
`FloatMeaning` term child, but relations remain carrier-specific. It admits
`FloatMeaningEqual(left, right)` and no FloatMeaning ordering proposition.
Cross-carrier comparison is unrepresentable rather than accepted and rejected
later. This follows the existing parallel `IntegerMathEqual` and ordered
integer-math proposition family instead of broadening runtime `ScalarTerm`
equality or inventing a runtime tagged ABI.

A projection term is identified by the canonical tuple

```text
(landed float source term, binary format, projection operation,
 exact projection-contract identity)
```

The landed source term is a verifier-reconstructible semantic coordinate, not
a fresh producer number or source byte offset. Its closed forms cover the
contract parameter/result, Terminal value, structural float leaf, or exact-bit
literal that actually supplies the carrier, each with its exact binary32 or
binary64 format. Authored projection occurrence and source span remain separate
diagnostic/provenance facts. Dense `ProofValueId` and projection-input numbers
are canonical encoding coordinates only.

Implementation realizes the exact-bit-literal form without a producer
coordinate. Raw binary32/binary64 bits cross checked lowering, Terminal
encoding, and independent verifier reconstruction directly; equal tuples
deduplicate, signed-zero bits remain distinct, and NaN payloads erase only when
the verifier reconstructs the public meaning. Direct owning-machine parameter
and result carriers rejoin their exact artifact declarations. A Terminal-only
non-call operation-result carrier additionally retains exact owner, producer
operation, declared scalar result, and format identity and is independently
rejoined to the operation table. A separate Terminal-only block-parameter
carrier retains exact owner, block, direct scalar parameter, and format
identity and cannot be substituted by another value-declaration class. A
separate Terminal-only call-result carrier retains exact owner, producer,
declared scalar result, and format identity and accepts only the scalar-result
call operation classes. The operation-result and call-result checked/source
producers remain transitional until expression-to-Terminal-operation
correspondences exist; block-parameter source production remains transitional
until nested-state contract identity is proven to the emitted block coordinate.
One Terminal-only structural-leaf source additionally retains the owner and the
existing complete `IeeeFloatStructuralField` root/path coordinate plus format.
Independent replay accepts the root only from that owner's direct structural
parameter table with owned, shared-borrowed, or mutable-borrowed access;
write-only custody cannot be observed. Replay walks relevant record/mixed
fields, fixed-array indices, and sum-case payload fields through declared
structural types, and requires the
selected leaf to be the exact IEEE format. Checked production remains
transitional until an exact checked-expression to Terminal owner/root/path join
exists.
There is no separate nested-state result source: authored states have arrival
requirements but no exit guarantees, and state completion produces the owning
machine's declared result. D40 therefore uses the existing direct-machine-
result carrier for that value rather than adding a state- or exit-scoped term.
Other arbitrary Terminal-value forms remain explicitly transitional until
their artifact-relative carriers exist.

Source admission now recognizes the projection declaration only when its
resolved symbol has the exact toolchain-owned `Float::meaning32` or
`Float::meaning64` hermetic identity and originates in the sealed
`float_operations.omg` source, with the exact toolchain `FloatMeaning` result
owned by `float_meaning.omg`. Checked binding independently repeats these
joins. A local same-path, same-signature operator or result lookalike therefore
cannot mint facts or receive already validated facts. The checked and Terminal
projection rows now carry the rooted-checker tuple `(format, operation,
declaration, catalog-version)` and a domain-separated commitment to the exact
owners, hermetic identity, private contract-free ordinary signature, source
carrier, and nominal result. Independent replay rejects component or
commitment drift. Artifact-aware contract/result and remaining Terminal-value
source coordinates remain separate engineering.

The compiler canonicalizes equal tuples to one proof value before equality is
formed. Multiple authored occurrences may therefore reference the same
`ProofValueId`; the verifier independently reconstructs the tuple and the
deduplication. Reflexivity discharges equality of that shared term. Distinct
source terms never coalesce because their projected values happen to coincide.
Relating them requires an explicit theorem application with complete contract
and evidence provenance; ordinary IEEE equality is insufficient because its
signed-zero and NaN laws differ.

Projection authority binds both the exact compiler-recognized core
`Float::meaning32` or `Float::meaning64` declaration and the exact closed
numeric-catalog identity/version that realizes its meaning. The verifier
reconstructs that join. Matching namespace, spelling, signature, format label,
fingerprint, or a locally declared lookalike grants no projection semantics.
The two current closed descriptors are `(32, 1, 1, 1)` and `(64, 2, 2, 1)`;
`FloatMeaningEqual` additionally requires both operands to carry the same
format, operation, and exact contract descriptor.

These terms and propositions are PCC metadata. They are erased before runtime
and add no runtime `FloatMeaning` object, conversion, comparison, branch, or
check. Native float lowering remains ordinary float lowering; the retained
correspondence lets the proof certificate establish that the selected runtime
operation refines its authored mathematical contract.

## D41 — Native realization resumes authority after standalone Terminal Psi

Terminal Psi is a complete publishable compilation product, not an in-process
checkpoint. Its representation and semantic vocabulary remain target-neutral.
A producer may stop after PCC and publish the canonical Terminal module; a
later interpreter or native lowerer may consume it on another machine under a
different authority. The ordinary source-to-native compiler uses that same
consumer boundary rather than receiving a semantic or authority shortcut.

A distributable Psi product may bind a target-constrained realization proposal
beside the target-neutral Terminal module. Selected provider-plan facts,
external-binding requirements, requested target/profile identity, and other
build-owned realization inputs are demands and constraints, not authority.
They must either be carried canonically in the product envelope or supplied by
an exact strongly bound companion; unbound host objects are never part of a
portable lowering contract. A proposal may be as narrow as one Windows or
machine-specific configuration. The receiving consumer independently accepts
that exact proposal using its local target catalog, installation admissions,
foreign-binding custody, and invocation policy, or rejects it. Rejection means
that this consumer cannot realize the valid Psi product; it is not retroactive
source or Terminal invalidity.

Compiler-owned target builtins use the consuming lowerer's trusted target
catalog. They do not implement `ProviderExecutionEvidence`, mint installation
receipts, or derive authority from compact report fingerprints. The exact
Terminal requirement and selected compiler-intrinsic proposal join a closed
structural catalog identity and target in a role-tagged native settlement.
Installed and foreign implementations remain a disjoint role and retain their
real admitted provider-execution custody. A normalized realization-machine
spelling may remain diagnostic or pre-closure provenance, but after checked
selection it cannot select native lowering in place of the structural catalog
identity.

Native emission publishes D32's occurrence-specific physical child bound to
the exact role-tagged `PhysicalChildParent` and surviving optimized occurrence.
For an ordinary `BoundaryCall`, that parent retains the complete replayable D41
settlement using the existing representation-rank `BoundaryExecutionBinding`;
for a D29 operator it references independently reconstructible application
coverage. The child, not a self-issued pre-lowering hash, retains target
lowering, assignment, relocation, and emitted-byte custody. D39 remains
orthogonal: selecting and emitting `exit_group` cannot manufacture the source-
semantic `ExternalTerminate` completion kind.

The former `CompilerIntrinsicSettlementEvidence` adapter violated this split:
it derived compact coordinates solely from compiler-owned inputs and presented
them through `ProviderExecutionEvidence`. It is retired. The first replacement
lane carries Linux `exit_group(i32)` as the structural role
`CompilerBuiltin(LinuxExitGroupI32)` from consuming-lowerer admission through
machine, image, and installation custody; the native artifact reports no
provider execution for that builtin.

## D42 — Target variation is flat, selected, and independently checked

The compiler invocation owns exact target identity. Maintained Omega source
does not declare targets, and the retired `target X { ... }` syntax is rejected.
Target-specific `host` and `boundary` policy comes from immutable
compiler/package inputs. The authoritative `build(builder: &mut Build)` machine owns
application and package role, roots, dependencies, generated outputs,
subsystem/image facts, provider selections, and other build decisions. Moving
roots, dependencies, or subsystem into target blocks would reverse this
deliberate ownership split.

Graph-forming control flow on `builder.target` is retired. So are
`depend_when` and `depend_as_when`. Dependencies are unconditional static build
rows in the current language; target-qualified entry bindings are unconditional
flat rows such as `roots.bind(target::ProgramEntry, Main::main)`. The package
manager never interprets a build-machine state graph to discover edges, and no
`common + by_profile` dependency projection or mixed/wildcard-path authority
exists. The `ProjectedDependencies::by_profile` carrier and its fixture-only
producers are removed with the retired syntax rather than retained as
live-looking scaffolding.

Platform implementation variation belongs to target-scoped machine/boundary
implementations and target-owned packages. Each exact-target child filters to that target's
declaration closure before resolution and typing, evaluates the corresponding
flat build facts, and publishes at most that child's Terminal Psi/PCC subject.
Merely declaring support for Linux, Windows, macOS, or another profile does not
place every profile in one Psi subject or define an application support set.

Multi-target orchestration is settled separately by D54. No unresolved target
branch enters Psi or lets one target's authority realize another's proposal.
D42 does not authorize inventing a project support allowlist, scanning
dependencies for targets, or treating a compiler catalog as application
intent.

If a concrete future customer requires a target-specific dependency edge, it
must use an explicit unconditional target-qualified build row, analogous to
`roots.bind`, and not arbitrary control flow or a target-block image fact. No
such syntax is reserved before that customer exists.

## D43 — Source custody validates the result, not the ambient Git executor

The source resolver delegates Git, SSH, HTTPS, credentials, helpers, proxies,
configuration, and ordinary same-user operating-system authority to the
invoking host. Omega does not attest that environment by hashing the selected
Git executable, classifying platform confinement, or signing its own command
and completion observations. Those facts cannot close same-user replacement,
authenticate an operator or repository owner, or establish that an audit took
place. The self-issued execution-guarantee matrix, canonical execution
provenance, `GitSourceReceipt`, and Seatbelt/Landlock executable/filesystem
confinement are retired.

This explicitly supersedes the earlier retention of executable content and
metadata drift checks as non-authoritative same-operation provenance. A
changed executor that changes the accepted source is detected by the verified
commit, tree, content, and immutable snapshot. A changed executor that produces
the same exact source has no source-custody consequence. Metadata observations
around a hashing window neither strengthen that result nor prove continuous
executable identity. Bounded operational details may remain ordinary logs,
but their absence cannot reject an otherwise valid resolution and they never
enter canonical source, lock, review, or admission identity.

Resolver controls survive when they do at least one of three things:

1. validate package-derived material;
2. directly enforce a concrete resource or process-lifecycle property; or
3. prevent package-controlled input from influencing a decision Omega makes
   using the operator's authority.

The third class retains the load-bearing executable-selection boundary. Before
reading package-controlled input, Omega snapshots the operator's selection,
resolves one absolute Git path, rejects a path inside any package-controlled
workspace, source, build-output, quarantine, or cache root, and uses that exact
path for the operation. It performs no later bare-name lookup. A package may
select a bounded source locator and revision, but cannot choose the executable,
credentials, helpers, arbitrary arguments, hooks, filters, or process policy.

The first two classes retain argument separation, noninteractive execution,
closed protocols, redirect/hook/replacement/filter/submodule rejection, Git
object validation, bounded command count and captured output, deadlines,
whole-process-tree cleanup, honest portable resource ceilings, safe object and
path traversal, bounded materialization, and immutable snapshot publication.
Windows Job Objects and comparable mechanisms may remain where they implement
cleanup or a concrete limit; they issue no host-execution trust claim. Stronger
operator-selected sandbox, VM, container, or CI isolation stays outside the
universal source-success contract.

Network acquisition admits HTTPS and SSH as one closed production transport
class. Ordinary host `insteadOf` configuration may route an authored HTTPS
locator through SSH or an authored SSH locator through HTTPS. HTTP,
unauthenticated `git://`, `file`, `ext`, and every other production protocol
remain denied. Effective host routing is not package identity and cannot
mutate `PackageKey`.

The authored canonical source lineage remains the identity input. GitHub and
GitLab adapters normalize HTTPS and SSH spellings only because they establish
one repository namespace. Generic Git lineage conservatively retains
transport, user, host, port, path case, and suffix distinctions until an
adapter proves equivalence; allowing host routing does not justify stripping
those fields or merging unrelated repositories. Resolved commit, tree,
content, and snapshot identities remain exact instance custody. Git object
hashes establish content integrity, not repository-owner authentication.

Locks, review, and compilation consume those direct resolved-source facts,
not a receipt over the process that fetched them. Ordinary live-source and
snapshot drift checks remain because they compare material consumed across
resolver-owned phases; only executor drift telemetry and its canonical
attestation are retired.

## D44 — Opaque representation carriers are inert storage in v1

D26's selected `OpaqueRepresentation<Opaque>` application chooses the physical
carrier for boundary-opaque data; it does not grant the carrier a second
lifecycle. The opaque declaration owns semantic multiplicity and every
authorized discharge, such as `InterruptMaskGuard::restore(self)` or
`InterruptAcknowledgement::complete(self)`. The compiler-owned representation
trait is empty, so it has no term in which to relate one of those operations to
a carrier finalizer. Automatically invoking ordinary carrier cleanup would
therefore invent semantics that neither source nor build selection expressed.

Every selected v1 application carries a closed, role-tagged lifecycle
disposition whose sole v1 role is `Inert`. The role is an input to the strong
application commitment; it is never represented by an absent optional field or
a provider assertion. V1 decoders reject every unknown role. A later role still
requires the ordinary coordinated schema/version revision; using a sum now
keeps the row structurally honest but does not reserve unversioned semantics.

Selection admission compiler-derives the property over the complete closed
carrier graph. It traverses every field, array element type, and every sum-case
payload independent of the case active at runtime. The carrier must be legal
for the required target ABI movement and must contain no independently invoked
nominal cleanup, nested live linear debt, or unjoined opaque, external, or
otherwise disposable obligation. A direct no-`drop` check is insufficient, and
an authored or provider-issued `cleanup_free` assertion is not evidence.
An invalid explicit selection rejects at its selection occurrence even when no
by-value use follows; absence remains demand-driven and an unused valid
selection still emits no consumer-demand row.

Carrier and opaque multiplicities need not match. For affine and linear opaque
values, physical register, spill, argument, return, and aggregate copies are
placements or relocations of one ledger-owned semantic occurrence; they never
manufacture another occurrence. For a copyable opaque value, only a checked
semantic copy operation creates another occurrence. The ownership ledger, not
the backend copy instruction, makes that distinction, and admission also
requires a structurally copyable inert carrier. This is D32's semantic/physical
split applied to opaque storage: Terminal ownership remains semantic while
native receipts own placement and emitted realization.

Resource-owning representation is not an interpretation of the empty v1
relationship. If needed later, it uses a distinct, opt-in, versioned
relationship such as `ManagedOpaqueRepresentation`, with an exact finalizer and
total disposition rules for construction, movement, duplication where legal,
success, failure and retry, return, abandonment, and partial construction.
Existing inert conformances acquire no new obligation merely because that
richer relationship is introduced.

## D45 — Target policy classifies every demanded terminal-authority leaf

Declared service reach and installed terminal authority remain separate review
axes. An exact service identity plus normalized schema maps to the closed set
of terminal-authority classes it permits. The receiving interpreter or native
lowerer independently maps each exact physical terminal mechanism it accepts
to the classes that mechanism exercises. The selected provider row joins those
facts, and realization rejects unless every exercised class is permitted. A
service, provider, package, filename, alias, or risk label never determines the
physical mechanism's class and cannot launder an excessive binding.

Classification authority resumes at D41's standalone Terminal-Psi consumer
boundary. Versioned target-policy modules belong to the receiving realization
authority; a compiler distribution may ship default modules, but a Psi
producer cannot force their acceptance. The accepted policy version and strong
commitment enter the realization evidence. Rejection means that consumer cannot
realize the otherwise valid Psi product and is not a retroactive source or
Terminal-semantic rejection.

The policy key is one closed, role-tagged, post-normalization terminal-
mechanism sum rather than a common row with optional or redundant fields. Its
roles include structural compiler-intrinsic execution identities; target ABI,
syscall number, and checked argument contract; normalized foreign locator and
admitted implementation contract; exact target firmware/table declaration,
field, and checked receiver contract; and exact checked physical-operation
catalog entries. Each role carries only its meaningful coordinates, and its
discriminant enters identity. Provider context and service schema remain join
inputs, not mechanism-key fields. Checked adapters are not terminal roles:
admission traverses their exact selected closures and rejects cycles, missing
leaves, duplicates, substitutions, and unclassified physical operations.

Target policy is deliberately partial over the universe of possible syscalls,
exports, and firmware coordinates, but demand-complete for each admitted
artifact. Every demanded terminal leaf has exactly one row. Known
authority-free mechanisms have explicit empty class sets, whether written as
individual rows or an exhaustive match over a closed structural family. There
is no complement/default-pure rule and no wildcard empty arm: absent, unknown,
or multiply classified leaves reject. An empty dangerous-authority set says
nothing about purity, general side effects, foreign-code trust, or provider
custody.

Rows classify the conservative union over every reachable argument value. A
narrower set is valid only when compiler-checked constants, ranges, handle
provenance, or another exact constraint proof excludes the broader behavior;
that constraint identity is part of the mechanism role. A requirement named
`open_read` narrows nothing by itself. Flag-polymorphic filesystem operations
therefore publish the union of content read, content write, metadata query,
directory enumeration, namespace mutation, and metadata mutation until their
lowerings pin or prove narrower flags. Faceted service requirements improve
precision but are not required for sound conservative containment. Raw integer
descriptors still establish operation classes, not object confinement.

The first closed terminal-class vocabulary distinguishes filesystem content
read, filesystem content write, filesystem metadata query, directory
enumeration, filesystem namespace mutation, filesystem metadata mutation,
process output, process termination, machine control, port I/O, interrupt
control, interrupt entry, and root-memory access. Exact requirement and
mechanism identities remain alongside these grouping classes. The existing
broad `Filesystem` and `Process` package-risk labels are transitional review
summaries, not terminal-policy identities and not permission grants.

The first implementation rung classifies the existing closed
`CompilerIntrinsicExecutionIdentity` families, including explicit empty sets
for authority-free numeric families and the nonempty process-termination class
for `LinuxExitGroupI32`. It does not infer a verdict from authored
`Binding::CompilerIntrinsic` spelling or use a wildcard for future family
members. Every new catalog role must receive an explicit exhaustive policy
disposition before admission can accept it.

The next implementation rung shares that policy through one role-tagged
`TerminalMechanismIdentity` and admits exact normalized-foreign rows. A
foreign role binds the selected target and collision-resistant normalized
locator identity to the strong contract commitment of its canonical admitted
`BoundaryEntryPlan`; provider report fingerprints never enter the key.
Receiving authorities supply a finite explicit table, and policy version 2
commits the complete intrinsic inventory, every foreign row, canonical order,
and every disposition. Direct demanded imports classify before provider
settlement. Missing or duplicate rows, locator or contract substitution, wrong
target, duplicate selected/external rows, and legacy string-backed bindings
reject. This rung covers PE-by-name, PE-by-ordinal, versioned ELF, and Mach-O;
it does not yet prove complete selected-provider-closure traversal or the
service/schema containment join.

String-backed imports are never durable classification identities. Before
foreign-import classification replaces the transitional filename-and-trait
review, the structural locator vocabulary first gains Mach-O, ordinary source
binding evaluation then converts authored `DllImport` values to exact
`NormalizedForeignLocator` roles, and target policy supplies rows for those
locators. PE-by-name, PE-by-ordinal, versioned ELF, and Mach-O identities remain
distinct. A string bridge may normalize only when it losslessly reconstructs
one exact structural locator; otherwise the receiving lowerer rejects that
realization proposal. The same structural-identity requirement applies to
table, firmware, and intrinsic strings that have not completed their catalog
join.

Once the intrinsic table and the locator/evaluator/policy joins are live,
package review replaces the blessed `(filename, trait-name)` dangerous-
authority classifier with the exact service-identity/schema permission table
and binding-derived containment. The filename table is not extended during
filesystem faceting. Tests retain at
least: explicit-empty versus unknown, target-distinct equal syscall numbers,
argument-union versus proof-bound narrowing, adapter cycles and missing leaves,
unknown intrinsic family members, every normalized import role including
Mach-O, string-only rejection, and an exercised-not-permitted containment
failure.

The first package-side table rung retains explicit consumer policy inside an
accepted semantic binding. Each row names the complete normalized schema, one
exact requirement identity, and its canonical permitted classes. Construction
and compiler replay reject schema substitution, invalid or duplicate keys, and
requirements that do not rejoin exactly one method in the reconstructed
schema. Candidate discovery may nominate the semantic binding but never adds a
permission row or class.

Canonical package review publishes that input as a separate blocking row with
the compiler-resolved service nominal, schema digest, exact requirement,
permitted classes, and both service and requirement source custody. It remains
separate from the transitional broad `Process` and `Filesystem` summaries,
which grant nothing. Root policy must accept the exact row, and direct native
compilation requires the resolved row to match the independently supplied
receiving permission policy. This is the first migration seam, not permission
completion: retained-Terminal realization, portable filesystem facets, and
complete replacement of every broad legacy row remain required before the
filename classifier is deleted.

Accepted ordinary closure evidence now derives the canonical exact accepted-
permission set only from obligations that survived fresh root-policy replay.
Retained Terminal native proposals preserve the checked package rows. Re-entry
first requires bijective equality with that independently accepted set and then
requires the accepted rows unchanged in the distinct receiving policy;
omission, substitution, and coordinated proposal/policy widening reject, while
unrelated receiving rows remain legal. The manager-owned entrypoint consumes
the complete package-aware Terminal report, matches its production manifest to
the accepted root's dependency closure, source-consumption commitment, selected
build-machine identity, deterministic evaluation usage, build-observation
identity, and target. Evaluation equality is invocation-local: aggregate
review-sponsor ceilings and session-wide peaks remain separate orchestration
custody. Compiler-report replay also requires the manifest target
and native target to equal the retained proposal. The manager derives the
accepted set from opaque evidence rather than accepting a caller's policy as
package admission. Direct and retained routes share the final exact accepted-
row-to-receiving-policy validator. Executable-root review preserves the exact
authored `builder.roots.bind` span through its generated marker, and an
application integration fixture now covers exact report/evidence success plus
source-consumption, build-observation, and coordinated retained-proposal/
receiving-policy substitution through the manager-owned route. A build with no
filesystem reach canonically retains one empty Output tree in both sponsored
review and ordinary production, keeping their complete observation identities
equal without treating sponsor context as semantic input. This closes and
covers the in-memory library join without embedding policy authority in target-
neutral Terminal Psi. Production CLI project compilation now consumes that
join. Policy-blind preparation retains raw compiler inputs and resolver custody;
the native operation separately compiles the final review, reconstructs current
conflicts, recovers an explicitly selected root-policy file against them,
accepts fresh evidence, and invokes the manager-owned retained route. Missing
policy for blockers and policy supplied for a blocker-free review both reject.
The accepted package permission projection and the separately supplied
receiving permission policy remain distinct through native realization.

Receiving-policy version 5's first direct-syscall settlement is conservative
and signature-derived. The checked argument-contract identity commits the
verified boundary's scalar carriers, canonical structural parameter positions,
multiplicity, access, and exact carrier identities. Every retained call
occurrence must match the declared arity and structural access. A root or
projected structural qualification or boundary requirement rejects until the
abstract plan retains its stable semantic-domain declaration; module-local
domain IDs never enter the digest. All runtime values admitted by the accepted
unqualified contract remain reachable: this rung claims no constant, range,
handle-provenance, or raw-descriptor narrowing. Native provider settlement
requires exactly one selected syscall row and one retained external row with
the same supported Linux profile and `u32` number, classifies that exact
mechanism, and passes it unchanged to closure review. Missing, duplicated,
unsupported, unclassified, or substituted coordinates reject; no service or
method name participates in derivation.

## D46 — Same-process package review does not observe its executable pathname

Package orchestration drives compiler review inside the same loaded `omega`
process. Reading the bytes reachable through `current_exe()` before and after
that review observes only the current pathname target. Replacing that file
cannot change the image already executing the bracketed review, so equality or
drift establishes no property of that operation. Internal reconstruction of
compiler-issued rows remains a canonical and semantic consistency check; it is
not a process-isolation boundary or executable attestation.

No compiler-executable path-byte commitment enters package-review envelopes or
rows, closure commitments, comparison, conflicts, locks, admission, or source
rendering. Mixed executable digests never reject. The unconsumed
`omega-build-provenance` carrier is retired, and no evidence-format migration
is required because the commitment never entered the encoded review schema.

Cross-invocation compatibility uses the exact subject's obligation-semantics
and evidence-schema identities plus the versioned package-review and row
encodings. A meaning-changing revision changes its semantic identity; an
encoding change changes the corresponding encoding version. Executable byte
identity is an overstrict proxy for those explicit contracts and cannot replace
them.

No executable digest is retained merely because a future cache might exist. A
concrete cache must first state whether it partitions exact implementation
artifacts or reuses semantically compatible results, then key that claim on the
appropriate artifact or semantic/build/schema identity. Cache absence or miss
never changes package validity.

This ruling is scoped to a same-process producer pathname. Exact bytes remain
load-bearing when the artifact itself is the proof subject, as on bootstrap
edges, and an explicitly selected sealed compiler artifact may remain a
deployment or reproduction subject. Real process/image attestation is separate
deployment evidence until a concrete Omega claim and independent verifier
consume it.

## D47 — Boundary domain requirements consume carried qualifications

A Terminal boundary structural-domain requirement is an argument position plus
one exact domain identity. Boundary-call admission checks whether the selected
structural argument already carries that identity in its qualification roster.
The requirement has no proposition term or proof conclusion, mints no
`ObligationId`, and does not reuse the positionally aligned proposition rows of
ordinary, Unit, or structural-scalar in-module calls.

The carried qualification may have been established upstream by predicate
proof, an authorized routed operation, propagation, validation, or another
route permitted by the domain. Those routes remain distinct. In particular, a
generic proof cannot replace sealed routed provenance such as the qualification
introduced by `InterruptEntry::enter`; the boundary call merely consumes the
already-established result.

Terminal format 54 and vocabulary 57 remove the former boundary
`requirement_obligations` field and wire payload. The preceding current-only
format required the legacy slot to remain empty and rejected every nonempty
roster; the new format says that the field is absent, not that boundary calls
own an optional proof facility. Abstract lowering and native settlement retain
the boundary, structural arguments, completion receipts, and declaration join;
they do not fabricate or preserve boundary proof IDs.

Qualification rosters are non-recomputable carried semantic facts. A
transformation may retain a qualification only when every incoming occurrence
represented by the output carries it through valid establishment lineage.
Control-flow joins use at most the common intersection, never the union.
CSE/GVN treats unequal rosters as unequal values unless it deliberately forms
the common intersection and revalidates all uses. Widening is an
authority-forging soundness failure. Narrowing is fail-closed, but an optimizer
claiming exact Terminal preservation must reject rather than publish that
semantic change.

A future proposition over an exact structural argument place would be a new
feature requiring its own term, substitution, admission, and publication
custody. It may constrain use but cannot establish or replace routed
qualification provenance unless the domain itself explicitly authorizes proof
as an establishment route. Mapping a qualification to `Proposition::Truth`,
copying opaque obligation IDs, or accepting a nonempty boundary roster without
reconstructing a conclusion fabricates evidence and rejects.

## D48 — Accepted locks are current-version generated artifacts

`omega.lock` is generated accepted state and is normally committed for exact
reproduction. It is not a durable multi-version evidence runtime. The accepted
lock begins with fixed magic and one outer format version, both checked before
payload allocation or interpretation. That version covers the complete payload
contract, including every nested source-subject, reconstruction-question,
obligation-semantics, review, and row schema or encoding required for
acceptance. Any incompatible nested change bumps the outer lock version.

Current Omega decodes only the exact current lock version. An unknown version
rejects immediately with regeneration guidance. From the exact source closure,
regeneration reconstructs the current package question and evidence, checks it
under the current semantic schemas, obtains fresh root admission where needed,
and writes a new lock. Old discharge and policy decisions are never
grandfathered merely because fields, compiler versions, or producer claims look
compatible.

There is no semantic-schema migration registry, compatibility classifier, or
backward-compatible accepted-lock requirement. A future format may add a
specific compatibility path only for a concrete independently motivated
product need; it does not arise automatically because a second version exists.
Unsupported historical locks may be retained opaquely and interpreted with
their matching old toolchain or separate audit tooling. Unavailable old source
continues through the standalone review packet and audit-recommendation path;
neither route grants current acceptance or erases a valid historical baseline.

Nested frames keep their exact identities for local reconstruction and
corruption diagnosis, but accepted-lock compatibility is decided by the one
outer version before those frames are read. A fingerprint may bind the exact
canonical lock bytes after decoding begins; it does not replace the cheap
magic/version gate or become an admission baseline by itself.

## D49 — Physical children retain role-specific semantic parents

D29 remains specific to checked boundary-operator applications. Its canonical
empty application means one real operator telescope of length zero; it never
means that an ordinary boundary-trait machine has no telescope. A target-
neutral Terminal operator demand and its strongly bound realization companion
jointly reconstruct D29 coverage.

D32's parent relation is the closed `PhysicalChildParent` sum rather than a
widening of D29. `OperatorApplicationCoverageRef` references independently
reconstructible D29 coverage. `BoundaryTraitSettlement` retains the complete
D41 settlement first created when the consuming lowerer accepts an ordinary
Terminal `BoundaryCall`. The parent discriminant enters canonical identity,
and a child of one role cannot satisfy the other even when native lowering is
byte-identical.

The D41 branch reuses `BoundaryExecutionBinding` and its existing
`AdmittedProvider` / `CompilerBuiltin` roles. Its compiler-builtin identity is
the representation-rank `CompilerBuiltinExecution`; the planner-rank
`CompilerIntrinsicExecutionIdentity` remains package-review provenance.
Conversion between them is one exhaustive `match` returning
`Option<CompilerBuiltinExecution>`, so the classification and returned payload
cannot drift as parallel expressions and every unclassified role rejects.

Native replay derives the complete surviving boundary-occurrence set from the
validated optimization projection. For each occurrence it reconstructs or
replays the role-specific parent, then checks the physical child's exact
parent, optimized occurrence, lowering, assignment, relocation, and emitted-
byte custody. Missing, duplicate, stale, substituted, padded, or role-swapped
children reject; verified elimination is the only omission. Retained D41
settlement content is replay input, not self-issued authority.

## D50 — Epsilon state transfers always author an argument list

Every Epsilon state transfer has an authored argument list, including `()` for a
zero-parameter state. A known bare state in continuation position is not a
state transfer and contributes `InvalidControlTarget` at the continuation
expression's first byte, regardless of the state's arity. Only an explicit
application enters arity and argument checking, so `retry` is
`InvalidControlTarget` while `retry()` may be `ArityMismatch`.

This is a semantic judgment over the existing postfix-expression continuation
grammar. Mandatory parentheses in a state declaration support the symmetric
surface but do not derive the use-site rule: the grammar still admits a bare
identifier. Ordinary-expression resolution is unchanged. State labels do not
enter the local-value namespace; a same-spelled active local resolves as that
local, a pending local is `UseBeforeInitialization`, and genuine absence is
`UnknownName`.

D36's continuation cases consequently include a known bare state and a known
bare machine in addition to a uniquely resolved constructor and an unqualified
state/machine collision. These are distinct source causes that deliberately
share `InvalidControlTarget` and the continuation-start coordinate; public
outcome bytes cannot distinguish them, so controls exercise each cause
independently. Expected type, arity, source order, or preferred-namespace
lookup never selects or admits one.

A bare state retains neither a state application nor a first-class state
reference. No application was authored, state labels are not values, and the
state-application ledger remains reserved for exact application syntax. A bare
machine may retain the existing general callable reference because that
identity has consumers outside this failed continuation judgment; this carrier
asymmetry does not grant either spelling application status.

## D51 — Epsilon owner-qualified machines always bind their data receiver

Epsilon has free machines and receiver machines, but no owner-qualified static
machine. An unqualified declaration accepts ordinary parameters and no
receiver. Every owner-qualified data-machine declaration begins with
`&mut self`, and every invocation is receiver-qualified. A receiverless
`machine Owner::name(...)` declaration rejects syntactically at the first token
where the required receiver is absent. Boundary-trait members remain their
separate bodyless declaration form.

For `machine Buffer::clear(&mut self)`, the first input is exactly the mutable
`Buffer` data instance. `self` is the fixed reserved symbol bound by that input;
it is not a distinct type, inferred global, or special runtime value. The
compiler normalizes expression occurrences of the keyword to the ordinary
named-local path using their source spans. The receiver binding carries the
owner-derived nominal type and place and remains active through the machine's
states. An occurrence outside that lexical scope therefore reaches ordinary
local lookup and is `UnknownName` under the existing rule.

The special `EpsilonSelfExpression`, nameless `EpsilonSelfLocal`, self-only lookup,
and `NoEpsilonLocalBindingName` path are representation defects and are retired.
The replacement receiver-local carrier retains the keyword span and exact
owner-derived type. `NoEpsilonLocalBindingType` remains because incomplete
transition-binder typing uses it independently. The catalog's broad
`EpsilonDeclaration` owner payload is separate upstream cleanup and no longer
survives in the receiver binding.

The surface partition removes the former case/qualified-machine collision
registry. For a data owner, `Owner::name(...)` selects a sum constructor,
`value.name(...)` selects a receiver machine, and bare `name(...)` selects an
unqualified machine. A case and receiver machine may share a spelling; neither
arity, expected type, statement context, nor source order selects between
namespaces. Owner qualification that supplies only namespacing adds no Epsilon
capability and can be reintroduced later if a concrete static-owner operation
requires semantics distinct from a free machine.

Controls cover free-machine calls; valid receiver binding in entry and state
bodies; undeclared `self` and multiple occurrences under ordinary earliest-
coordinate selection; case/method spelling reuse; rejection of empty and
ordinary-parameter qualified declarations; direct `Owner::method(...)` not
selecting a receiver machine; and receiver syntax never selecting a case.

## D52 — Resultless Epsilon arguments own their value-use anchor

A resultless call used directly as a machine or constructor argument
contributes `TypeMismatch` at the first byte of that authored argument
expression. Redundant grouping is part of the argument expression, so
`consume((write()))` anchors the resultless failure at the outer `(`. This is a
sibling judgment to the enclosing application's callable admission and arity;
it is derived even when either enclosing branch fails.

The adjacent `never` argument relation adopts the same argument-subtree
ownership but deliberately retains its existing call-head coordinate.
`consume((stop()))` therefore anchors `InvalidTerminal` at `stop`, because the
defect is the mispositioned nonreturning call, while the resultless defect is
that the authored argument position supplied no value. Controls pin this
reason-specific grouping distinction rather than normalizing it away.

The body-phase accumulator publishes only the smallest packed coordinate and
retains a reason list only for exact ties. An enclosing `ArityMismatch`,
`UnknownName`, or inadmissible-callable rejection therefore precedes and
replaces a later resultless-argument candidate; the observable requirement is
the earlier rejection and never `InternalFailure`, not physical co-retention of
both candidates.

The application-start audit is closed by the D37 premise graph. For any call
`C`, every relation consuming `C`'s result category is blocked by an arity
failure on `C`, because `C` produces a result only after its own arity and
argument-type join succeeds. An inner call `G` may successfully produce
`Resultless`, whose argument-position failure is then a sibling of outer call
`F`'s admission and arity; their coordinates are distinct. Thus no two
simultaneously derivable distinct reasons share an application coordinate.

This ruling adds no rejection reason or wire code. Controls cover a valid
outer call, wrong outer arity, unknown and inadmissible outer callees,
constructor arguments, grouped resultless arguments, and grouped `never`
arguments.

## D53 — Epsilon block exits are local facts, not a reachability fixed point

The grammar no longer classifies `call ";"` as both a statement and a
terminal. Every semicolon-terminated call is syntactically a statement. A
successfully resolved call returning `never` gives its block the semantic
`NoNormalReturn` exit effect. Any later executable statement, return, or
transition in that same entry or state block contributes `InvalidTerminal` at
its terminating delimiter: `;` for a statement or return and the closing `}`
for a transition. State declarations following the entry sequence are not
executable successors and remain permitted. The later construct's children are
still checked, but it contributes no return, transition, or other block-exit
parent relation after flow has ended.

Every machine entry and every declared state body is checked independently,
whether or not another block currently transfers to it. Each block edge has
exactly one of five effects:

```text
Falloff | ReturnNone | ReturnValue(type) | NoNormalReturn | StateTransfer(state)
```

There is no `MachineTail` effect. A transition continuation that is a
resultless machine call has `Falloff` if the call returns; a call returning
`never` has `NoNormalReturn`; and a value-returning call is `TypeMismatch` at
the continuation expression start. Value-returning control is written
explicitly as `-> return expression`. These rules apply to every admitted
machine continuation, including free, receiver, and sealed-boundary calls.

The enclosing machine category checks each edge locally. A resultless machine
admits `Falloff`, `ReturnNone`, `NoNormalReturn`, and `StateTransfer`. A machine
returning `T` admits structurally compatible `ReturnValue(T)`,
`NoNormalReturn`, and `StateTransfer`. A `never` machine admits only
`NoNormalReturn` and `StateTransfer`. An incompatible falloff is `TypeMismatch`
at the exact closing `}` of the entry or state body. Existing explicit-return
anchors remain unchanged.

`StateTransfer` is compatible with every category because its target is a
state of the same machine and that state body is checked independently under
the same declared return category. Consequently no reachability traversal,
cycle detection, or least fixed point participates in return validation. On
any finite execution, a normal return must occur through one locally checked
falloff or return edge; an infinite state cycle takes no normal-return edge and
requires no termination proof. Unused malformed states reject deliberately,
so adding a transfer elsewhere cannot change whether an untouched state body
is valid.

D37's rule that runtime selection does not suppress checking of authored arms
continues to govern child diagnostics, but reachability is not a block-exit
premise. Controls cover empty and nonempty blocks, every machine category,
unused states, closed cycles, each of the five effects, resultless/`never`/value
machine continuations, constructs after `never`, and exact delimiter/brace
coordinates. This ruling adds no rejection reason or wire code.

## D54 — Explicit target sets fan out at target-sensitive stages

Omega accepts either one exact target or a caller-supplied nonempty set of exact
targets. A multi-target request normalizes to canonical profile order with
duplicates removed. `all`, `*`, an empty set, inference from source or
dependencies, and iteration over the compiler's complete profile catalog are
not target-set inputs. Source-level target policy is not an authored
application-support matrix. The toolchain catalog likewise mixes deployment
profiles with abstract and local modes and therefore cannot define deployment
intent.

The former root-level empty `target X { }` activation and discovery blocks and
the last maintained nonempty policy blocks have been removed. The declaration
grammar and lowering are deleted, and a directed parser diagnostic plus an
architecture gate reject their return. Host/boundary policy is supplied by
immutable invocation/package inputs.

Multi-target compilation is staged fan-out, not an opaque loop around the whole
compiler. Source acquisition, the immutable source snapshot, and parsing are
formed once. Flat build facts mean only syntax-projectable declarations that
cannot observe `Build.target`: the selected project role and name and the
resolver's unconditional dependency rows. Target-qualified root bindings are
unconditional rows in that shared parsed source, but slot membership, schema
compatibility, and exact entry selection remain child-local. The pipeline forks
at each first target-sensitive consumer: exact root selection, target-scoped
declaration filtering, target semantics and admission, provider and
foreign-binding selection, build-machine execution, and native realization.
`Build.target`, filesystem observations, generated output, optimization and
provider selection, and every other evaluated `Build` result are therefore not
shared build facts. An implementation may share more work only when the shared
result is exactly the same fact each independent child would have consumed;
mutable target state or one child's authority never enters a sibling.

Each target child has exactly the subject, semantic identity, diagnostics, and
outcome it would have under a standalone exact-target invocation. Adding or
removing siblings cannot change that child identity. Every requested child is
checked even when another rejects, and the orchestration returns one ordered
outcome per canonical target. A nonzero process result or human summary when
any child fails is ordinary orchestration behavior, not semantic evidence about
the application.

Target-neutral checked, Terminal Psi, PCC, or other immutable products may be
forwarded to several target children. Byte-identical or semantically identical
products are shared only after their governing strong identities compare equal;
coincidental structure or names do not authorize reuse. A target-specific Psi
product remains a separate child. Each native branch receives its own target,
admission profile, provider plans, external bindings, and consuming-lowerer
authority and may independently accept or reject the Psi proposal. No
unresolved target branch is encoded inside Psi.

The maintained native batch route prepares each child's canonical Terminal
artifact independently, then shares only the target-neutral decode,
proof-admission, and abstract-input lowering keyed by the complete
`TerminalArtifactIdentity`, exact `AdmissionProfile`, and the
optimized/unoptimized entrance. The selected target, entry and calling plans,
provider and external settlements, authority policies, callbacks, physical
evidence, and machine/image lowering remain exact-child inputs. A prepared
input rechecks its key when consumed. Controls pin both one identical Terminal
artifact entering distinct x64/AArch64 lowerers and target-selected roots with
different Terminal identities remaining separate. This reuse is an internal
compiler optimization, not proof, review, or audit evidence.

The optional batch manifest commits to the exact explicit request set and each
child commitment/outcome. It claims only that those requested compilations were
performed. It does not assert that the set is complete, supported, tested,
audited, deployable, or equal to every target known by Omega. CI and release
configuration may own an operational matrix, including cross-compiling several
profiles on one host, without promoting that matrix into language or package
identity.

## D55 — Exact requirement edges declare their lifetime application

A checked or external machine realizing one exact trait requirement writes the
complete target-trait application in its `satisfies` clause. Lifetime arguments
precede other static arguments in the existing angle list:

```omega
machine read_external<'scope, Item>(value: &'scope Item) -> &'scope [u8]
    satisfies Reads<'scope, Item>::read
    via Binding::DllImport("driver", "read");
```

The compiler carries those lifetime arguments through syntax, symbol-resolved,
typed, checked, provider, and package-review custody. It reuses the existing
whole-conformance judgment: argument count equals the target trait's lifetime
telescope, every argument names an in-scope binder in the realizing machine's
lifetime telescope, and the raw declaration-order ordinal vector substitutes
through the exact requirement signature, inherited requirements, contracts,
and evidence requirements. Repeated ordinals are valid; mapping two trait
lifetimes to one realizer lifetime is a legitimate application rather than a
duplicate or malformed mapping.

The raw ordinal vector is compiler-internal checking material, not the public
edge identity. The realization edge exposes the trait requirement rather than
the implementation's binder numbering. Its canonical identity scans the raw
vector in trait-parameter order and numbers each distinct realizer binder by
first occurrence. Thus `[0,0]` and `[1,1]` both normalize to `[0,0]`, while
`[0,1]`, `[1,0]`, and `[4,2]` normalize to `[0,1]`; `[4,2,4]` normalizes to
`[0,1,0]`. This retains exactly the equality partition among target-trait
lifetimes and is stable under private binder renaming, reordering, and insertion
of unused machine lifetime binders.

That normalization applies to the exact requirement edge regardless of the
realizing machine's visibility. A public machine's direct callable application
may separately use its declared telescope as public callable identity. The
machine application and its `satisfies` edge are different identities for
different consumers; package review must not publish raw realizer ordinals as
the requirement-edge key.

Checked and irreducible external realizations use this same semantic edge and
canonical lifetime partition. Distinct partitions remain distinct opaque
supply rows even when runtime signatures and `via` bindings are physically
identical. The external row identifies the promised borrow contract but does
not prove the foreign implementation obeys it. The motivating prospective
customer is a zero-copy parser, driver, or callback whose result view borrows
from caller-owned input; a lifetime-free adapter would have to copy and cannot
preserve that relationship.

Lifetime application is declared rather than inferred from the realizing
machine signature or contracts. Incidental signature edits therefore cannot
change or ambiguate edge identity. Runtime erasure likewise does not erase the
source contract. Omega currently has no lifetime constant such as `'static`:
every target-trait lifetime argument names an active binder. If lifetime
constants are later added, every declared trait lifetime slot remains explicit
and the argument identity becomes a closed binder-or-constant sum; a lifetime
fixed directly in a trait requirement and absent from its telescope requires no
application argument.

## D56 — Epsilon entry diagnostics close inside type formation

Epsilon's accepted entry shape remains unchanged: one exact `Console` boundary
signature set, a record `Main` with one sealed `console: Console` field plus
ordinary program fields, and one `machine Main::main(&mut self)` with no value
parameters or return. This ruling totals rejection taxonomy and coordinates; it
does not redesign entry execution.

Entry closure is the final whole-program subjudgment of the existing type-
formation phase. It is not a sixth checking phase and it never merges with the
later body/control candidate carrier. A type-formation rejection therefore
precedes every body/control rejection by phase, irrespective of coordinates;
the separate `EpsilonTypeFormationCandidate` and `EpsilonFinalCandidate` carriers
make cross-phase coordinate comparison unrepresentable.

The entry premise gate is an authored owner/name candidate for `Main::main`, not
a signature-valid entry. If no such declaration candidate exists, the sole
entry verdict is `MissingEntry` at source extent. Missing `Main`, `Console`, or
`Main.console` does not contribute another candidate in that branch. Once an
authored `Main::main` candidate exists, absence is no longer a possible verdict:
a wrong receiver, parameters, result, duplicate or competing entry declaration,
or malformed supporting component is `InvalidEntry`.

Supporting-component shape checks consume that entry-name premise. An authored
offending boundary declaration/member, `Main` declaration/field, field type, or
entry declaration anchors `InvalidEntry` at the first byte of the offending
construct. A required but absent supporting component anchors `InvalidEntry` at
source extent. Multiple absent components therefore derive the same reason and
coordinate and merge into one candidate. `Console` boundary members form an
unordered exact identity/signature set; parameter binder spellings are
diagnostic only. Member order and binder renaming never change validity.

Within type formation, candidate identity is exactly `(packed offset, reject
reason)`. Repeated derivation of the same pair is one candidate; distinct
reasons at one offset are a compiler contradiction and produce outer
`InternalFailure`. The current Delta source carries a separately authored
integer `kind` beside each reason. Although every existing call site maintains
a reason/kind bijection, that convention is redundant and unenforced. The D56
implementation deletes the `kind` parameter and derives equality centrally from
the existing total reason-to-ECOUT-code bijection. Codes are compared only for
equality; numeric code order never selects a rejection.

Source extent is the only omission coordinate. Epsilon introduces no synthetic
coordinates for missing entry components. The retained Epsilon corpus currently
contains no entry-bearing program and therefore does not exercise this
judgment. Implementation must author dedicated entry fixtures before publishing
golden coordinates, covering absent, malformed, duplicate, reordered, renamed,
and body-error-adjacent forms without treating corpus silence as evidence.

## D57 — Epsilon transition patterns have a total staged judgment

Epsilon transition syntax admits at least one arm and makes wildcard placement a
grammar property. A transition body is either one or more nonwildcard arms with
an optional final wildcard arm, or one wildcard arm. After a wildcard
continuation the parser requires the transition-closing `}`. Any following
pattern token, including another `_`, is `UnexpectedToken` at that token.
Wildcards never participate in `DuplicatePattern`; that reason retains its D24
scope of repeated scalar selectors and exact sum cases.

A final wildcard is legal even when every current sum case is already named.
The sum-coverage `or` is inclusive: exact enumeration or a final wildcard is
sufficient, and exact enumeration plus that wildcard is also valid. Adding a
case therefore does not turn an untouched defensive wildcard into an error.

Body/control checking resolves pattern names first. An unknown owner or case is
`UnknownName` at that name. A resolved owner that cannot own sum cases, a scalar
selector against a sum, a case against a scalar, or a case belonging to a
different nominal sum is solely `TypeMismatch` at the pattern start. Such a
pattern does not enter duplicate or arity checking.

Subject admission grants a scalar selector or exact case its semantic identity
before case-payload arity. The first admitted occurrence owns that identity even
if it later fails arity. A later occurrence is solely `DuplicatePattern`; a
unique case with the wrong binder count is solely `ArityMismatch`. Only a
unique, subject-compatible, arity-compatible pattern supplies complete-pattern
and typed-binder facts. This follows D24's existing phase discipline: syntactic
binder identity is collected independently before later case and arity
validation rather than inferred from a completed downstream judgment.

Scalar selector identity is the validated `i32` value. Decimal spelling is not
identity: `false`, `0`, and `00` coincide, as do `true`, `1`, and `001`.
Negative patterns are syntactically unavailable and reject at the authored `-`;
out-of-range positive tokens reject before an identity exists.

Static sum coverage consumes a complete sum subject and every completed pattern
premise. Missing coverage without a final wildcard is `NonexhaustiveSum` at the
transition subject's first byte. Subject failure and coverage are mutually
exclusive by premise. A category, duplicate, or arity failure suppresses
coverage, so repairing it may reveal `NonexhaustiveSum` in the next checking
round. Scalar transitions have no static coverage requirement; an unmatched
runtime scalar traps as `NonExhaustiveTransition`.

The Delta implementation's current `Resolved | Complete` progress carrier is
too broad: `Resolved` conflates unavailable subject information, subject
incompatibility, and wrong arity. D57 implementation replaces that conflation
with separately reconstructible name-resolved, subject-admitted, semantic-
identity, and complete-pattern stages. Known boundary/record owners in case
position receive the category judgment above rather than silently producing no
fact. Any structurally impossible field result from a sum-case-only lookup is
removed through a narrower carrier or treated as an internal invariant failure,
never ignored as source recovery.

## D58 — The complete Delta compiler selects one measured Gamma resource profile

The current canonical Beta-written Gamma compiler continues to admit 256
procedures, 1,024 non-builtin call rows, 1,024 global states, and 1,024 global
edges until the replacement profile is measured and published atomically. The
incomplete `delta_compiler.gamma` already consumes 116 procedures, 965 call rows,
739 states, and 586 edges before its production entry, returned-`Bytes`
preflight, D19 adapters, and final publication exist. The call-row adjacent gate
therefore describes a real staging obstruction, not Gamma language invalidity.

Final capacity is measured from the complete canonical Delta compiler rather
than projected from the current calls-to-states ratio. A noncanonical staging
Gamma compiler may give the global count tables a deliberately roomy provisional
capacity so every required Delta component can be authored as ordinary Gamma
source. That staging artifact proves no published edge and cannot replace the
canonical compiler, weaken output preflight, or hide an instruction plan or
Alpha bytes outside the Gamma source.

Once the complete source exists, measurement covers the conjunctive compiler
profile: procedures; global call rows; global and per-procedure state/edge
counts; every state- and edge-derived initialization table; labels, fixups,
emitted tape, and the execution bounds exercised by maximum cases. For each
independently provisioned authored-structure count, the selected provision is
the least power of two for which measured demand occupies at most 75 percent.
Derived corruption guards continue to derive from their owner, and the emitted
tape remains governed by D23 rather than this headroom rule. Limits remain
independent: conjunctive means every limit is measured and jointly satisfied,
not that unrelated tables receive one numeric capacity.

Per-procedure state and edge limits require separate justification because they
size initialization/reachability work and bound its fixed point. Global state
storage grows linearly, but the current state-name collector scans prior global
rows and therefore has quadratic maximum work; an enlarged global-state limit
must retain an executed maximum-work gate or first narrow that scan to the
current procedure. Calls and global edges retain their own measured work gates.

Derived storage is never treated as an independent magic constant.
`INIT_BLOCK_REQUIRED`, `INIT_BLOCK_GENERATED`, and
`INIT_BLOCK_TERMINATOR` are sized from procedure capacity plus global-state
capacity. `INIT_IN_WORK` and `INIT_REACH_WORK` are sized from the per-procedure
state limit plus the entry block. `INIT_EDGE_GENERATED` and
`INIT_EDGE_SOURCE_BLOCK` are sized from global-edge capacity. Every dependent
guard, address, and exact/adjacent case moves with its owning limit.

Changed tables move together into a new aligned compiler-private region above
the existing fixup table rather than repacking the dense low-memory table block.
Memory scarcity is not the reason for the selected limits; bounded maximum work
and avoiding repeated atomic profile rebuilds are. The final revision rebuilds
the Gamma compiler tape, source/tape admission subject, memory-map documentation,
and canonical `Incomplete(limit, requested)` gates as one change while leaving
Beta instructions, Gamma semantics, GCOUT resource identities, and the
AlphaBootstrapV2 tape maximum unchanged.

Emitter or instruction-plan consolidation remains ordinary source-quality work.
It is accepted only when Gamma-authored checked source demonstrably reduces total
source and proof complexity. It neither gates the measurement revision nor
licenses preserving 1,024 by code golf.

## D59 — One flat OCREQ v1 profile is shared at the wire, not in private representation

D18's logical Omega compilation subject and invocation and D25's outer framing
are realized by one flat, byte-exact `OCREQ` version-1 profile. The subject and
invocation acquire no independent nested versions: representation changes move
the outer version atomically. Normative checked tables publish every field and
row in order, every little-endian width and extent, every zero-based graph
index, every closed numeric tag, and every reserved slot. All reserved slots
must be zero. Counts, lengths, indices, and coordinate-producing input extents
remain at most `INT32_MAX`. Validated language names use their specified UTF-8
grammar; snapshot paths, file contents, and link targets remain length-framed
raw bytes. Rust object layout, enum order, serde, pointers, host paths, and
private re-encodings are never wire inputs.

The invocation's 32-byte subject commitment is SHA-256 over the exact byte
sequence
`"omega.ocreq.subject.sha256.v1\0" || subject-section-bytes`. It is not a hash
of a reconstructed object. The compiler first validates the complete outer
frame and the subject's canonical byte representation, then recomputes this
commitment. Package-key identities remain independently recomputed from their
structural package name and source lineage; the subject commitment does not
replace those local identity checks.

Framing precedes capacity. A decoder validates the identity and reserved bytes,
rejects every encoded `u32` high bit, and establishes exact end from the known
request extent before applying any request or compiler ceiling. The canonical
Epsilon implementation is the worked arithmetic rule: compare section lengths
against remaining extents subtractively and never form a potentially trapping
`16 + subject_length + invocation_length` sum. A short stream claiming a huge
section is malformed `Reject`; a structurally exact huge request may then be
`Incomplete`. Only after framing and ceilings does validation proceed through
canonical fields, package identities and order, graph, snapshots, admissions,
subject commitment, source, build, and later compiler phases.

D25's common `OCOUT` contract is retained rather than relaxed. Adjacent checked
tables assign every Reject, Incomplete, and InternalFailure code, its admitted
coordinate spaces, and fixed diagnostic phase. Common phase precedence,
canonical coordinate ordering, and source anchors select the same published
diagnostic when `D` and `C` process the same request under the same profile.
This agreement is a useful differential oracle but does not replace either
compiler's refinement proof. Unknown codes, illegal coordinate/code pairings,
nonzero reserved slots, and noncanonical tails reject.

The shared 40-byte frame retains `u64` physical fields, but OCREQ v1 normalizes
every public limit, requested amount, and primary coordinate into
`0..INT32_MAX`, with the high 32 bits zero and every selected limit strictly
below `INT32_MAX`. For a nonnegative mathematical quantity `x`, publication is
`min(x, INT32_MAX)`. The Epsilon implementation realizes this through a private
`Exact(nonnegative i32) | Overflowed` domain and pre-addition or
pre-multiplication tests; it never performs trapping arithmetic and catches the
result. An Omega implementation may compute exactly in a wider domain and
saturate only at publication. The wire constrains observable bytes, not private
arithmetic.

An intentionally narrower bootstrap compiler, if selected later, cannot reject
valid Omega outside its coverage as invalid source. Its coverage is a disclosed
resource profile of named scalar provisions; exceeding one yields
`Incomplete(resource, limit, requested, coordinate)` and no language verdict.
Where a language construct is provisioned by a count, a zero limit and the
actual positive requested count honestly express its absence. Differential
comparison requires the same profile on both compilers. This rule keeps the
wire compatible with a future bootstrap subset but does not itself amend D6's
current requirement that `omega₀` implement full Omega; changing that
refinement scope remains a separate owner decision.

The normative field, outcome, and resource tables are checked projections of
constants embedded independently in both offline compiler artifacts, never
host files consulted at runtime. Their remaining numeric assignment and
implementation are ordinary closure work under this ruling: neither compiler
may publish a complete V1 boundary until the tables, exact/adjacent vectors,
unknown-tag rejection, phase selection, bounded arithmetic, and no-partial-
publication gates agree.

## D60 — Alpha is raw tape; Beta is the textual assembly rung

Alpha owns only the audited VM, raw instruction encoding, and tape execution
semantics. It has no textual source language. The former Alpha assembly syntax
is Beta, and its canonical implementation is the directly retained
`source/beta/compiler/beta_assembler_bytecode.tape`. The readable
`assembler.beta` must reconstruct that exact tape byte-for-byte.

No platform-specific Beta assembler executable is retained. A caller loads the
one raw assembler tape into the selected Alpha VM for the duration of an
invocation. Stamping remains replaceable loading plumbing and creates no second
native implementation or artifact identity.

The former language rungs shift upward without changing their semantics:

```text
former Beta  = Gamma
former Gamma = Delta
former Delta = Epsilon
```

Accordingly, the baseline chain is Alpha VM → Beta assembler → Beta-written
Gamma compiler → Gamma-written Delta compiler → Delta-written Epsilon compiler
→ Epsilon-written Omega `D` → Omega-written `C`. Existing decision numbers keep
their ruled content at the renamed semantic rung; wire identities shift with
their compiler edge: `BCOUT` becomes `GCOUT`, `GCREQ`/`GCOUT` become
`DCREQ`/`DCOUT`, and the generated Epsilon-compiler boundary becomes `ECOUT`.
The exact magics are part of those versioned profiles, not inferred from stale
filenames or constructor order.

The split does not add a second source compiler beneath Beta. The assembler
tape is the cold-start program. Native Alpha remains the only per-platform
binary, and every artifact above it remains platform-independent tape.

## Dependency order

1. retain and check the direct Beta assembler tape;
2. finish the Beta-written Gamma compiler edge and common tape boundary;
3. publish the Gamma-written Delta compiler tape;
4. implement and publish the Delta-written Epsilon compiler tape;
5. compile the Epsilon-written Omega source closure `D` into `omega₀`;
6. compile the Omega-written source closure `C` with `omega₀` into `omega`; and
7. optimize or natively realize tapes without changing the semantic chain.

## D61 — Bootstrap topology is a measured choice, not a fixed spine

D6 and D60 describe the currently implemented baseline. They no longer settle
that every named rung must survive. A candidate may directly admit and audit an
exact Alpha seed program, interpret an early source language, connect two
nonadjacent baseline rungs, move the checker, or delete Beta, Gamma, Delta, or
Epsilon when the complete audit becomes smaller.

The candidate families, common workloads, and measurements are defined in
[Bootstrap chain alternatives](../../design_briefs/bootstrap_chain_alternatives.md).
The comparison counts the native root, admitted tapes, semantics, compilers,
interpreters, checkers, proof edges, resource contracts, sidecars, and permanent
validation together. Current implementation, corpus use, prior investment, and
hypothetical reuse establish no presumption of retention.

Several bounded prototypes may coexist while measuring alternatives. They do
not become parallel authorities. The selected design promotes at most one
canonical chain; losing permanent semantics, artifacts, gates, and sidecars are
deleted or retained only as explicitly nonauthoritative, deletion-bounded test
or construction tools.

A directly audited tape is a legitimate root subject. Lower-language pedigree
may help reconstruct it but is not required merely to manufacture authority.
The tape instead owes an instruction-level audit against Alpha semantics and
the exact small evaluator/compiler claim made for it.

## D62 — The functional evaluator lattice replaces the baseline

D62 exercises D61 and supersedes D6 and D60 for live topology. The selected
chain is:

```text
audited Alpha VM + directly audited Beta evaluator tape
  -> Beta-written Gamma compiler
  -> Gamma-written Delta compiler
  -> Delta-written Omega compiler D
  -> Omega-written Omega compiler C
```

Alpha semantics and native seeds remain unchanged. The former Beta textual
assembler is renamed Alpha Tape Assembly and retained only as off-chain tooling
under `tools/alpha/tape-assembly/`. It may reconstruct tapes but supplies no
language edge or authority premise. The former imperative Gamma language and
its compiler are deleted. The former Delta language is renamed Gamma; the
former Epsilon language is renamed Delta; Epsilon is no longer a live owner.

Beta is a strict first-order functional S-expression calculus with checked
integers, immutable bytes, immutable tagged constructors, exhaustive matching,
conditionals, immutable local binding, first-order calls, mutual recursion,
proper tail calls, bounded implementation profiles, sealed input, and returned
values. It has no closures, higher-order values, mutation, raw memory, macros,
polymorphism, general garbage collector, continuations, exceptions, modules,
packages, interactive evaluator, or ambient effects.

Each intermediate language exists only to deliver the compiler immediately
above it and a named small tool such as the derivation checker. Intermediate
self-hosting is not a goal or evidence requirement. Only Omega closes a useful
self-host edge when `omega0` compiles the production Omega-written closure `C`.

The Beta evaluator is directly admitted root material and owes instruction-
level audit against Beta semantics. Readable Alpha Tape Assembly source may aid
construction and review, but exact reconstruction does not prove the evaluator.
The derivation checker moves to an ordinary Beta program and receives only the
calculus required by selected compiler-edge certificates.

No stable migration, compatibility adapter, alternate compiler, or retained
old artifact is required. Missing new edges remain explicit until implemented;
Git is the archive for the removed chain.

## D63 — Trusted Beta replaces opaque Gamma-evaluator admission

D63 supersedes D62 for live topology and reinstates D60's essential ruling:
the imperative tape-assembly language is a trusted rung named Beta. The selected
chain is:

```text
audited Alpha VM + admitted Beta compiler tape
  -> Beta-written Gamma evaluator
  -> Gamma-written Delta compiler
  -> Delta-written Epsilon compiler
  -> Epsilon-written Omega compiler D
  -> Omega-written Omega compiler C
```

The implementation experiment under D62 reached a 12,716-byte Alpha tape and
2,023 lines of readable source while still lacking declaration tables, general
calls, constructors, `match`, and proper tail calls. That result falsifies the
practical premise that the completed functional evaluator can be reviewed as an
independently understandable opaque tape. Calling its readable assembly
optional would hide, rather than remove, the source-level trust dependency.

Beta therefore owns the exact deterministic source-to-Alpha-tape relation in
`source/beta/LANGUAGE.md`. Its cold-start implementation is the admitted
`source/beta/compiler/beta_compiler_bytecode.tape`; the authoritative readable
`source/beta/compiler/beta_compiler.beta` reconstructs that tape
byte-identically. Native Alpha remains the only per-platform binary.

The functional language specified under D62 moves intact from Beta to Gamma.
Its evaluator source is
`source/gamma/evaluator/gamma_evaluator.beta`, and its request identity is
`GAMMAREQ` v1. The former Gamma and Delta language owners move to Delta and
Epsilon respectively. Their immediate compiler sources are
`delta_compiler.gamma`, `epsilon_compiler.delta`, and
`omega_compiler.epsilon`. Compiler boundary identities shift with those edges:
the Delta edge owns `DCREQ`/`DCOUT`, and the Epsilon compiler application owns
`ECOUT`.

The Beta compiler's self-reconstruction does not prove its semantics, but it
binds the readable trusted-language subject to the admitted Alpha tape. The
source-to-tape relation, Alpha behavior, independent differential, and exact
reconstruction remain separate obligations. No compatibility aliases or
duplicate old owners are retained; Git records D62's abandoned topology.

## D64 — Beta minimizes by shared structure, not compressed authority

The admitted Beta compiler is minimized from a 6,418-byte hex-only baseline to
a 2,706-byte Alpha tape while preserving a readable two-pass assembler. Its
17,019-byte, 602-line Beta source reconstructs that tape byte-identically.

The selected reductions remove duplicated mechanisms rather than encode hidden
knowledge: both passes share one scanner; source bytes are validated while
loaded into a bounded region; persistent compiler state uses a documented high-
register ABI; labels occupy bounded contiguous exact-name rows; duplicate and
reference lookup share one linear byte comparison; words and registers share
one lowercase hexadecimal parser; `db` shares mnemonic recognition; and each
NUL-terminated mnemonic row carries the NUL-terminated list of operand widths
consumed by both passes.

The language narrows to lowercase hexadecimal words/registers, lowercase label
identifiers `[a-z_][a-z0-9_]*`, and printable `db` strings with only `\\0`,
`\\\\`, and `\\"` escapes. Only whitespace may separate `db` from its string.
These forms cover every retained Beta customer. The admitted implementation
checks its 1-MiB source region, 65,536-row label region, and Alpha raw-tape
output ceiling before advancement.

The experiment rejects one-pass fixup chains, label hashes, compact name
identities, removal of conventional commas/comments/ASCII whitespace, and an
explicit mnemonic control-flow trie. They either enlarge proof invariants,
weaken readability, or save too little. New Alpha compare-immediate opcodes
would save about 380 Beta tape bytes and more in later Beta programs, but do not
yet justify revising both native seeds, Alpha semantics, listings, and
conformance. That option remains measurable rather than selected.

## D65 — Beta uses asserted addresses instead of symbolic labels

D65 supersedes D64's symbolic Beta syntax. The compiler remains readable Alpha
tape assembly, but every control target is a lowercase hexadecimal word and a
block boundary may assert its exact output offset:

```text
0x107: ; source_reject:
  jmp 0x107                 ; -> source_reject:
```

Assertions emit no bytes and reject unless their value equals the running
output length. Comments carry human block names; they have no semantic role.
Every numeric jump or call in retained source repeats its target's comment name,
so reviewers can compare the literal address, assertion, and generated tape
without trusting a symbol resolver.

This removes identifier semantics, label rows, duplicate/unresolved-label
checking, fixups, and the count pass. The admitted compiler emits once while
checking its source and output bounds. A late failure may leave a stdout prefix;
invocation plumbing publishes only status-zero output and otherwise removes the
temporary destination.

The resulting 16,812-byte, 458-line source reconstructs a 2,135-byte Alpha tape,
down from D64's 2,706-byte symbolic compiler. The addressed Gamma evaluator
produces the same Alpha tape as its former symbolic source and passes the same
behavior suite. Address changes are intentionally unpleasant and cascading;
that maintenance cost is accepted for this nearly frozen trust rung because the
source makes physical control-flow identity explicit.

The migration renderer used to calculate address updates is nonauthoritative,
ignored build scaffolding. It may propose a diff, but it is not retained source,
a compiler stage, or a trust premise. Exact assertions, independent assembly,
self-reconstruction, and downstream behavior validate the checked-in result.

## D66 — Gamma chooses unary functions and pairs over general algebraic data

The 42-case Gamma evaluator baseline was 69,958 addressed-Beta source bytes,
2,026 lines, and 12,735 Alpha tape bytes. It implemented one unary entry
function and the expression primitives but not declaration census, calls,
constructors, `match`, or proper tail calls. The following isolated experiments
were measured against that exact baseline:

| Experiment | Evaluator tape | Change from prior checkpoint |
| --- | ---: | ---: |
| bounded unary function rows and exact entry lookup | 13,057 | +322 |
| isolated unary calls using ordinary bounded frames | 13,838 | +781 |
| heterogeneous pairs plus narrowed identifiers | 14,599 | +761 |
| one shared NUL-row reserved-form table | 13,630 | -969 |

The selected Gamma core therefore has exact 76,430-byte, 2,231-line Beta
source and a 13,630-byte Alpha tape. Its functions have exactly one parameter.
Nested immutable `Pair` values carry heterogeneous argument tuples, lists,
tagged records, and trees. `pair-first` and `pair-second` validate private node
shape; applying `=` to a pair is an authored trap. Gamma has no user-defined
algebraic declarations, `match`, arbitrary function arity, or proper-tail
implementation requirement.

This decision rejects an explicit CEK-style evaluator machine for now. The
ordinary-call candidate completed 170,000 recursive countdown calls and
reported `Incomplete` at 180,000 through the checked 16-MiB evaluator-stack
collision. That count is diagnostic rather than a language limit, but it shows
the bounded implementation has ample depth for the bootstrap customer without
adding continuation tags and a central return dispatcher. Calls isolate lexical
environments with an explicit binding-base word; stack exhaustion never becomes
a Gamma value or semantic rejection.

The customer check is executable rather than hypothetical. The retained
4,357-byte, 96-line Gamma program `tests/gamma/fixtures/delta0_compiler.gamma`
compiles a compact directly programmed control-flow graph with physical state
assertions, register immediates, moves, arithmetic, jumps, conditional
transitions, and halt. It emits an exact 35-byte Alpha countdown tape, which is
stamped and executed. Incorrect state assertions and unknown operations produce
quiet authored traps. The prototype uses deliberate division-by-zero as its
minimal fail-closed rejection carrier; it does not claim to be the canonical
Delta language or a typed C-power systems language.

Reopen unary arity or generic algebraic data only when a representative Delta
compiler becomes materially larger or less auditable because of nested pair
plumbing. Reopen an explicit evaluator machine or proper tail calls only when a
named checker/compiler workload approaches bounded call storage or needs
language-level tail-space reasoning. A follow-up experiment removed division,
remainder, and their signed edge checks while replacing Delta0's fail-closed
division with an existing bounds trap. It saved only 179 tape bytes. Both
operations remain: that 1.3% implementation cost is smaller than the expected
customer complexity of quotient loops, and decimal parsing plus alignment work
are credible near-term uses. Reopen individual arithmetic or byte primitives
only against complete customer source rather than feature count alone.

## D67 — Gamma is a concatenative compiler machine

D67 supersedes D66's unary Lisp-and-pair Gamma. The Beta implementation made the
audit cost visible: 2,231 physical source lines, 1,879 instruction/data lines,
233 address assertions, and a 13,630-byte Alpha tape. Only about 39% of tape
bytes implemented expression semantics; request handling, parsing, structural
validation, names, immutable byte ropes, traversal, and storage containment
dominated the root.

The selected challenger retains compiler mechanics directly: 64-bit words, an
explicit checked stack, fixed zero-initialized cells, sealed input, append-only
byte/word output, named words, ordinary calls, and tail `jump`/`branch` control.
It removes expression trees, lexical environments, pairs, immutable byte graphs,
general structural validation, and returned value construction. Its exact
Beta implementation is 738 physical lines, including 107 address assertions,
and emits a 4,289-byte Alpha tape.

The customer comparison did not move complexity downstream. The same Delta0
addressed-CFG compiler fell from 4,357 bytes and 96 lines of Lisp Gamma to 1,914
bytes and 81 lines of concatenative Gamma. Both versions emit the identical
35-byte Alpha countdown tape, which is stamped and executed. The selected total
review surface is therefore 819 lines rather than 2,327.

Gamma definitions are mutually visible source spans. Ordinary word calls retain
only a source cursor/end continuation. `jump` and `branch` replace the active
word, so a tested 100,000-transition loop uses constant continuation storage;
10,000 ordinary recursive calls also complete. Fixed bounds cover request,
definition rows, stack, cells, continuations, output, and Alpha's hidden stack.
Malformed definitions reject before execution; reached unknown body words trap.
Output may precede a later failure and remains authoritative only under status
zero.

This trades high-level purity for direct auditability and compiler-shaped power.
Cells are source-visible bounded mutable state, arithmetic follows Alpha's exact
wrapping/signed rules, and stack effects are part of each builtin contract. The
language has no locals, heap values, algebraic data, computed jumps, or ambient
effects. Reopen a richer value language only when a representative Delta
compiler is materially larger or less auditable because of explicit cells and
stack shuffling. Reopen structural body validation only if unreachable malformed
tokens become a concrete trust problem worth its measured parser cost.

## D68 — Gamma reconstructs its evaluator tape without embedding it

A retained 213-line Gamma program reads the canonical addressed Beta evaluator
source and emits its Alpha tape. It tokenizes comments, separators, registers,
hexadecimal words, numeric address assertions, all 21 used Alpha mnemonics, and
the evaluator's quoted `db` data with `\0`. It contains mnemonic hashes and
opcode signatures, but no evaluator tape bytes and no host assembly call.

The executable gate builds the expected tape with the admitted Beta compiler,
runs the reconstructor under the Beta-authored Gamma evaluator on the same source,
and requires exact equality. Both paths now produce the same 4,289-byte tape.
This is a reconstruction triangle and capability measurement, not a compiler
fixed point: the Beta compiler remains authoritative, and the Gamma program does
not compile arbitrary Gamma into a self-reproducing native compiler.

The experiment exposed a real evaluator defect. `output-word` originally peeled
bytes with Alpha's signed division and remainder, so words with bit 63 set emitted
incorrect high bytes. The evaluator now stores the raw word in reserved scratch
memory and emits eight `loadb` results. A full-width all-ones regression pins the
behavior. That repair reduced the evaluator to 738 Beta lines and 4,289 tape
bytes while making exact self-reconstruction possible.

## D69 — Gamma reaches an experimental native compiler fixed point

The actual self-host experiment is retained separately from D68's evaluator
reconstruction. `source/gamma/compiler/gamma_compiler.gamma` is a 372-line,
11,968-byte Gamma program that compiles concatenative Gamma directly to Alpha.
The current evaluator compiles that source to a 13,834-byte tape `T0`; running
`T0` on the same source produces `T1`, and the executable gate requires
`T0 == T1` byte-for-byte.

The compiler uses two source passes. Pass one records exact source-name spans
and assigns one native address per word. Pass two emits a fixed 1,122-byte
runtime prefix and direct code: literals become `imm` plus stack push, builtins
call fixed helpers, user words use Alpha `call`, `jump` becomes Alpha `jmp`,
`branch yes no` becomes pop plus `jnz yes` and `jmp no`, and word ends become
`ret`. Runtime bytes are visible as `output-word` constants in the Gamma source;
the startup call operand is patched from the measured `main` address.

The fixed point is not only self-comparison. Evaluator-seeded and native `T0`
compilation of the 81-line Delta0 customer both produce the same 3,399-byte
native compiler. That compiler still emits the exact 35-byte Alpha countdown
tape. Representative native tests cover forward calls, branches, cells, sealed
input, and a 100,000-step tail loop.

This experiment demonstrates that Gamma can implement its own translation job
in fewer source lines than the 738-line Beta evaluator. It does not replace the
evaluator: the native compiler tape is more than three times larger, embeds a
runtime prefix, and does not yet claim every profile-v2 malformed-source or
resource observation. It remains a diagnostic side artifact until complete
compiler conformance and whole-chain trust cost favor a compiler edge over the
smaller evaluator edge.

## D70 — Gamma's fixed-point compiler pays its visible audit cost

D69's first fixed point was technically genuine but understated review cost.
Its 372-line source emitted the 1,122-byte native runtime as roughly 140 packed
`output-word` constants, used numeric cell identities throughout compiler logic,
and selected `main` and builtins by 64-bit polynomial hash alone. The fixed-point
equality checked those choices but did not make them understandable or eliminate
possible name collisions.

The cleaned compiler emits every runtime instruction through named Alpha
mnemonic helpers. Each runtime block boundary checks its exact output position,
and the dynamic startup call is visibly patched from `main_address`. All fixed
compiler-state cells have named getters/setters; only the genuinely indexed
definition-row array uses computed cells. Reserved syntax, `main`, and every
builtin compare exact token length and all source bytes using two readable
little-endian ASCII chunks. User-word lookup remains exact byte comparison.

The honest implementation is 532 Gamma lines and 18,617 source bytes. Its native
fixed-point tape is 19,681 bytes, versus D69's 372 lines and 13,834 bytes. Thus
auditability costs 160 source lines and 5,847 tape bytes. The evaluator-seeded
`T0` still equals native `T1` byte-for-byte; seeded/native compilation still
agrees for Delta0, calls, branches, cells, input, and a 100,000-step tail loop.

This remains an experimental side artifact because complete profile-v2 source
rejection and resource equivalence are not proved. Its source-size comparison is
now meaningful: the compiler is smaller than the 738-line Beta evaluator without
hiding a second opaque runtime representation inside packed constants.

## D71 — Representative Delta does not yet justify a functional rung

A noncanonical Gamma-written compiler tests a typed state-machine Delta slice
before changing the normative functional Delta contract. Its 38-line customer
uses one nominal sum, one record, one fixed array, typed machine parameters,
results and locals, named states, constructor assignment, record and array
access, exhaustive transition dispatch, a machine call/return, conditional
looping, and direct Alpha emission.

The compiler performs exact ASCII tokenization and name comparison, duplicate
declaration rejection, nominal type and owner checks, machine-local variable and
state checks, declaration-order sum exhaustiveness, two-pass native address
assignment, and typed Alpha lowering. It is 564 Gamma lines and 22,601 source
bytes. Gamma's self-hosted compiler produces a 19,872-byte native Delta compiler.
Interpreted and native execution emit the same 453-byte sample tape, which runs
to status zero.

Eight malformed twins cover duplicate names, unknown types, cross-machine
variable use, record-field type mismatch, constant array bounds, nonexhaustive
sum transitions, cross-machine state targets, and unknown statements. The
interpreted and native compilers agree on status 2 and exact partial-output
prefixes for every twin.

This slice does not justify inserting a permanent functional language between
Gamma and typed state machines. The observed implementation failures came from
implicit Gamma stack-temporary clobbering and unnamed checker state; Delta's
typed named locals directly address that pain. Explicit states, fixed storage,
and direct CFG lowering were not dominant costs. Approximate line spend was 64
for named compiler state, 76 tokenizer/keywords, 49 names/numbers, 84 symbols
and types, 88 declaration/state census, 45 statement sizing, 36 Alpha/type
helpers, and 122 typed replay/lowering.

The experiment is not the canonical Delta compiler and deliberately restricts
global namespaces, sum representation, storage allocation, array indexes, and
call recursion. Reopen the functional-rung comparison when a representative
larger slice requires recursive syntax values, variable-length collections,
nested scopes, or rich deterministic diagnostics. Those costs are absent here
and could reverse the result. Its latest Gamma implementation is retained under
`tests/delta/state-machine-experiment/`; the test owner retains the
customer and interpreted/native agreement gate.

## D72 — Address target comments use compact spacing

Control-target annotations now follow the instruction after one separating
space instead of padding to a shared column:

```text
jeq re, r0, 0x4f ; -> read_probe:
```

Address assertions retain `0x4f: ; read_probe:`. The compact form keeps the
numeric target and its inert human name in one visual unit and removes horizontal
scanning across whitespace. This formatting changes no Beta tokens or tapes.
The canonical Beta compiler source falls from D65's 16,812 bytes to 14,696 bytes
at the same 458 lines and reconstructs the same admitted 2,135-byte tape. The
Gamma evaluator remains 738 lines and emits the same 4,289-byte tape.

## D73 — Indexed arenas carry Delta's parser and scope workload

The state-machine Delta experiment adds typed copy, sequential byte input/output,
and checked dynamic array reads/writes. The Gamma compiler grows from 564 to 636
lines and from 22,601 to 25,533 source bytes. Its native tape grows from 19,872
to 22,339 bytes; the original representative program grows from 453 to 523 bytes
because every generated program now includes one shared checked-index runtime
helper and authored-trap block.

A new 109-line Delta customer parses a nested declaration stream into fixed
indexed name/depth arrays and a dynamic scope-start stack. It permits nested
shadowing, rejects same-scope duplicates with exact source offsets, detects
unclosed and unmatched scopes, and reports name/scope arena exhaustion through
the generated runtime boundary. Its Alpha tape is 1,919 bytes. Interpreted and
self-hosted-native Gamma compilation produce identical Delta compiler tapes and
identical outputs for both customers.

This closes three D71 revisit conditions without a functional value language:
variable-length logical collections, nested scopes, and deterministic located
diagnostics. They cost 72 additional Gamma lines and remained explicit state
machines over bounded typed storage. The strongest remaining functional-language
challenge is now rich recursive syntax transformation across many node variants,
not tokenization, scope management, or collection representation alone.

The experiment remains noncanonical. It still has one global declaration
namespace, one-word sum values, static machine storage, restricted call recursion,
and a small statement vocabulary. Those restrictions must either become selected
Delta semantics or be expanded against the Epsilon compiler customer before the
functional Delta contract is replaced.

## D74 — Bounded arenas carry recursive syntax transformation

The unchanged 636-line state-machine compiler now admits arrays whose element
type has exactly one word of storage. Multiword records and arrays remain
rejected. This one-token checker change leaves the native compiler at 22,339
bytes while allowing arena tags to use a nominal sum rather than raw numeric
conventions. A negative twin proves that storing a word in the nominal tag arena
is rejected identically by interpreted and native compilation.

A new 427-line Delta customer parses a five-variant expression tree into parallel
fixed arenas with variable-arity child chains. It validates operator arity,
traverses the arena in explicit postorder, folds literal addition and negation,
selects literal conditional branches in place, and emits canonical preorder
output. The customer has 80 named states and divides into 260 lines of declarations
and parsing, 105 lines of transformation, and 53 lines of serialization. Its
exact Alpha tape is 9,563 bytes. Runtime cases cover mixed and complete folds,
a surviving conditional, malformed arity/framing/tokens, multiple roots, exact
source offsets, and node/depth exhaustion.

This closes D73's remaining implementation-necessity challenge. Rich recursive
syntax transformation does not require a functional intermediate language:
bounded indexed arenas and explicit traversal frames remain sufficient,
deterministic, and locally auditable. The cost is visible rather than absent:
even this small tree language needs 427 lines and 80 states. A functional design
may reduce customer source while increasing the trusted compiler with recursive
values, allocation, and implicit control. Selection should therefore compare
whole-edge cost against the actual Epsilon compiler customer; it should not add
a functional rung merely because recursive syntax exists.

## D75 — Symbolic Alpha encoding favors explicit bounded state

The state-machine experiment adds typed signed division, growing its Gamma
compiler from 636 to 643 lines, from 25,533 to 25,943 source bytes, and from
22,339 to 22,690 native bytes. Existing sample, parser, and recursive-transform
tapes remain byte-identical. Division is required only to stream positive
label targets as eight little-endian bytes; Alpha retains its existing signed
division semantics and traps.

A 539-line, 103-state Delta customer now performs the closed backend job already
present in the incomplete functional Epsilon compiler. Its 22-case nominal sum
covers symbolic labels and all 21 Alpha instructions. Fixed item and label
arenas support exact layout, dense and duplicate label checks, forward-target
resolution, complete prevalidation, and output only after every structural
check succeeds. One all-opcode case pins exact widths, high-bit immediate bytes,
and every target operand shape. Negative cases cover duplicate, missing, and
extra labels, undefined targets, unknown and truncated records, trailing input,
empty payload, and item-arena exhaustion.

The state-machine customer is 539 lines and 12,635 bytes and compiles to a
10,842-byte tape. The corresponding retained functional Alpha declarations and
implementation occupy 834 lines and 39,426 bytes in
`source/epsilon/compiler/epsilon_compiler.delta`. The explicit version avoids
the functional backend's persistent 20-level label trie, balanced byte rope,
and post-encoding replay by retaining bounded mutable arenas and withholding
all output until symbolic validation is complete.

This is favorable evidence, not a canonical replacement. The customer profile
is 128 symbolic items and 256 labels; the retained functional implementation
targets the complete 1,048,572-byte Alpha envelope and independently replays
the raw payload. The next experiment must scale the arena profile and connect
real Epsilon semantic lowering. Within the tested profile, however, the actual
backend workload is about 300 source lines smaller in state-machine Delta, so
neither recursion nor symbolic encoding currently pays for a functional rung.

## D76 — Fixed arenas carry the complete Alpha payload profile

The symbolic encoder's bounded checkpoint is expanded without changing its
representation or publication discipline. All twelve item-field arenas and two
label arenas now contain 1,048,572 words. They consume 117,440,064 bytes; with
Delta's 1 MiB static base, the final arena ends at byte 118,488,640, below
AlphaBootstrapV2's 256 MiB memory bound. Label counts and identities use
little-endian `u32`, removing the prior 256-label restriction.

Typed wrapping multiplication and signed less-than branching are added beside
division for label decoding and payload bounds. The Gamma compiler grows from
643 to 661 lines, from 25,943 to 26,802 source bytes, and from 22,690 to 23,403
native bytes. The encoder grows from 539 to 574 lines and from 12,635 to 13,602
source bytes; its generated tape grows from 10,842 to 11,772 bytes. Existing
state, parser, and recursive-transform tapes remain exact.

The gate resolves a forward label numbered 300, emits one exact 1,048,572-byte
payload formed from 95,324 eleven-byte equality branches and eight returns,
and rejects the adjacent 1,048,583-byte request before output. Four-byte label
count framing, exact input end, dense definitions, all instruction forms, and
high-bit immediate bytes remain covered.

This removes D75's profile-size reservation. The state-machine backend is still
260 lines smaller than the retained 834-line functional backend while covering
the selected Alpha payload limit. It spends fixed semantic memory instead of
persistent trie and balanced-rope structure and relies on complete symbolic
prevalidation instead of replaying bytes it just emitted. The remaining honest
comparison is actual Epsilon semantic lowering and whether an independent raw
tape replay adds enough trust to justify its implementation cost. Payload scale,
recursive syntax, and symbolic target resolution no longer independently argue
for a functional Delta rung.

## D77 — Typed record rows replace parallel compiler arenas

Full-profile fixed storage need not expose a parallel-array convention to every
compiler customer. The Delta experiment now permits arrays of fixed one-word-field
records and adds checked `index-field-set` and `index-field-get` statements.
The checker requires the selected field to belong to the array element record,
requires source or destination type equality with that field, computes the
record stride from its declared shape, and retains the shared dynamic-index
bounds trap. Ordinary whole-element indexing remains restricted to one-word
elements.

The Alpha encoder replaces twelve item-field arrays and two label arrays with
one twelve-word `AlphaItemRow` arena and one two-word `AlphaLabelRow` arena.
Its semantic memory extent remains exactly 117,440,064 arena bytes and
118,488,640 bytes including Delta's static base. A malformed twin that stores
a `word` into the nominal `AlphaKind` field rejects identically under interpreted
and native compilation.

Typed rows grow the Gamma compiler from 661 to 709 lines, from 26,802 to 28,913
source bytes, and from 23,403 to 25,104 native bytes. The encoder falls from 574
to 552 source lines because fourteen array declarations and bindings collapse
to two owned schemas, although longer explicit field names increase source
bytes from 13,602 to 13,869. Runtime stride and field-offset setup grows its
tape from 11,772 to 12,610 bytes. The exact maximum payload and every prior
runtime result remain unchanged.

This is the preferred state-machine representation for an Epsilon compiler:
typed row arenas make schema ownership, field meaning, and nominal tag types
locally auditable without adding heap values or recursive runtime machinery.
The cost is 48 trusted compiler lines and 838 customer tape bytes. That trade is
accepted for source auditability; native compactness is secondary on a frozen
bootstrap rung. Actual Epsilon semantic lowering remains the final unresolved
comparison before changing normative Delta.

## D78 — Actual helper shape blocks premature Delta promotion

The retained Epsilon compiler quantifies the next language decision. Its 505
definitions have this parameter-arity distribution: 95 have arity 1, 162 have
arity 2, 109 have arity 3, 60 have arity 4, 28 have arity 5, 32 have arity 6,
and 19 have arity 7 through 13. Maximum arity is 13. Across definitions, 144
parameter spellings are reused; `source` alone appears 279 times. Exact
top-level body scans find 152 direct-recursive definitions before considering
mutual recursion.

The state-machine experiment has one exact global namespace, one parameter and
one result per machine, and static nonrecursive call storage. These restrictions
were useful while measuring control, arenas, recursive syntax, and Alpha encoding,
but they are not an auditable representation of the actual Epsilon customer.
Immediate translation would replace ordinary helper parameters with prefixed
global variables and replace recursion with unrelated hand-built state, making
source-size comparison meaningless.

Normative functional Delta therefore remains unchanged. The next experiment
must add owner-scoped members and machine-scoped variables, then compare two
purpose-built call models on an actual Epsilon parser/checker slice: bounded
typed call frames versus explicit typed traversal-frame arenas with nonrecursive
helpers. General recursion, closures, heap allocation, and ambient stack growth
remain out of scope. Promotion waits until one model preserves deterministic
bounds and improves whole-customer readability without erasing the 709-line
compiler advantage established by D71 through D77.

## D79 — Scoped bounded calls do not rescue State Delta helper readability

The State Delta challenger now has exact owner-scoped lookup. Top-level types
and machines remain global; fields and cases use their nominal owner; parameters,
the result, locals, and states use their machine owner. Repeated names across
owners are accepted, while same-owner duplicates reject. Calls declare and
repeat an arity from zero through thirteen, resolve each ordered parameter row,
and check every one-word argument and result type. Aggregate parameters and
results reject rather than being partially copied.

Every machine has a fixed software-frame extent. Frames grow upward from the
one-MiB static base, Alpha return addresses grow downward in eight-byte steps,
and each call preflights the two prospective extents before publication. A
three-argument recursive sum executes and unwinds correctly. A separate
80,000,024-byte recursive frame admits three live entries and halts with status
2 before the fourth can overlap the return stack. Arities zero and thirteen execute;
arity fourteen, mismatched call arity, aggregate parameters, oversized frames,
and same-owner duplicate names reject identically under interpreted and native
Gamma execution.

The trusted compiler grows from 709 to 815 Gamma lines, from 28,913 to 32,916
source bytes, and from 25,104 to 29,105 native bytes. Owner scoping accounts for
8 lines and 362 native bytes; arity, frames, recursion, and exact one-word checks
account for the remaining 98 lines and 3,639 native bytes. Existing frame-relative
customers grow to 2,278 parser bytes, 11,038 transform bytes, and 14,505 encoder
bytes. The full-profile encoder's 118,488,640-byte upper extent remains below
Alpha's 256-MiB memory bound.

The actual Epsilon helper shape is the stronger discriminator. The closest
representable parser kernel ports `ascii_between`, `is_digit`, and recursive
decimal accumulation with repeated names and four arguments. Its helper code is
48 State Delta lines and ten named states; the corresponding retained
Functional Delta definitions occupy nine lines. The State version must consume
sealed input instead of carrying the original immutable `Bytes`, index, and end
because this challenger still lacks first-class byte views and recursive values.
Bounded calls therefore solve the scoped storage problem without producing an
auditable translation route for the real 505-helper Epsilon compiler.

Normative Functional Delta remains unchanged. State-machine Delta remains useful
evidence for bounded arenas and direct Alpha lowering, but this call-frame model
does not win the whole-edge readability comparison and is not promoted.

## D80 — Scalar Functional Delta reaches an executable tape-density milestone

A noncanonical current-Gamma compiler now accepts the scalar recursive core of
normative Functional Delta and emits Alpha directly. The subset has `Int`
functions, zero through thirteen parameters, mutually visible declarations,
lexical `let`, `if`, arithmetic, equality and less-than, nested calls, and direct
recursion. Bounded named compiler contexts retain nested call, conditional, and
binary-expression state rather than hiding it below recursive Gamma calls.
Interpreted Gamma and Gamma's native fixed point produce byte-identical results.

The experiment compiler is 565 Gamma lines and 23,693 source bytes; its native
Alpha tape is 22,214 bytes. A nine-line, 198-byte Functional Delta accumulator
recursion compiles to 842 Alpha bytes and exits 15. Its exact State Delta
counterpart is 29 lines, 522 source bytes, and 771 Alpha bytes and also exits 15.
Functional Delta therefore expresses this workload in one third the physical
lines, while State Delta emits 71 fewer bytes. Their source-line-to-tape ratios
are respectively 93.56 and 26.59 bytes per line; bytes per line measure density,
not total quality.

The Functional compiler is currently 250 Gamma lines and 6,891 native bytes
smaller than D79's State compiler, but that is not a complete compiler comparison.
The State challenger already covers nominal data, fixed arrays, typed rows,
direct I/O, and a full-profile symbolic Alpha encoder. The Functional experiment
still lacks algebraic data, constructors, exhaustive `match`, immutable `Bytes`,
complete static checking and diagnostics, checked integer overflow, arbitrary
arity, proper tail calls, application profiles, transactional publication, and
exact resource outcomes. It cannot compile `epsilon_compiler.delta` and does not
produce the canonical `delta_compiler_bytecode.tape`.

D80 therefore establishes direction, not selection closure: Functional Delta's
source compactness now has executable Alpha evidence, while the decisive
whole-compiler ratio waits for the missing language and profile surface.

## D81 — Typed source elaboration works for one Delta schema

A 239-line, 9,616-byte Gamma program now validates the complete scalar
accumulator-recursion schema used by D80 and emits one canonical Gamma source
artifact. Its 9,526-byte native tape agrees with interpreted Gamma. The
elaborator owns no Alpha opcode, instruction width, address, fixup, or tape
publication rule. Fixed output chunks carry their decoded ASCII beside them,
and the gate requires byte equality with the retained five-line, 223-byte Gamma
artifact before invoking the existing Gamma compiler.

The expansion represents `n` and `acc` on Gamma's visible value stack, selects
base and step words with `branch`, and performs recursion with tail `jump`.
The resulting 1,366-byte Alpha tape emits byte 15. A renamed source with start
1,000 normalizes to the same private words plus one changed canonical literal,
executes 1,000 tail steps, and emits byte 20. Binder, callee, local, and literal-
bound mutations reject before output. No Gamma or Alpha primitive was added.

This is a successful mechanics proof under the relevant criteria: the
intermediate artifact is readable and independently compilable, semantic state
is visible in the lower language, expansion is deterministic and bounded, and
backend authority is reused rather than interleaved with higher-language logic.
Final Alpha size is recorded but is not the success criterion.

D81 does not yet select a macro-extension topology. The elaborator recognizes
one useful schema, whereas D80's 565-line direct compiler accepts a broader
scalar language. The next discriminator must cover that same scalar surface,
including nested expressions, calls, lexical bindings, and diagnostics. If it
remains favorable, Gamma must then elaborate to canonical Beta before the
selected chain changes. Only those two gates justify replacing direct
higher-rung Alpha backends repository-wide.

## D82 — General scalar Delta elaboration removes one direct backend

A separate 548-line, 21,180-byte Gamma elaborator now covers the same scalar
Functional Delta constructs as D80's direct compiler: signed literals,
variables, lexical `let`, `if`, all seven scalar operators, mutually visible
functions, forward and nested calls, direct recursion, and arities zero through
thirteen. Its 19,238-byte native tape agrees under interpreted and native Gamma.
It contains no Alpha opcode, layout, address-fixup, or tape-publication logic.

The elaborator emits source-order private function and expression words plus a
visible two-cell frame convention. The retained recursive receipt is 25 Gamma
lines and 1,267 bytes; the full-surface receipt is 77 lines and 4,324 bytes.
The existing Gamma compiler turns those receipts into 2,498-byte and 5,884-byte
Alpha tapes, which execute with observed result bytes 15 and 21. Negative
literal normalization emits 255. Malformed declarations, references, arities,
types, and truncated expressions reject before publication.

This route is 17 Gamma lines and 2,976 native bytes smaller than D80's direct
scalar compiler while moving all Alpha authority into the existing Gamma
compiler. The receipts are independently compilable and expose frame, call,
branch, and expression structure, though source-order numeric word names make
the full receipt closer to readable compiler IR than hand-authored Gamma.
One profile difference remains: all list expressions share one 15-level nesting
bound, whereas D80 has separate 15-level call, binary, and conditional bounds.
D82 is positive evidence for selective typed elaboration, subject to resolving
or accepting that aggregate bound.

## D83 — Universal prior-rung output fails at Gamma-to-Beta capacity

A 724-line, 23,050-byte Gamma elaborator emits canonical mnemonic Beta for the
complete current Gamma source surface. Its 26,599-byte native tape validates all
reached words and transfer targets before output. Interpreted and native
elaboration agree. Beta assembly is byte-identical to direct Gamma compilation
for five retained subjects: the full builtin/control fixture, both generalized
Delta receipts, the Delta0 compiler fixture, and the 532-line Gamma compiler
itself. The latter expands to 2,618 Beta lines and 63,144 bytes and assembles to
the exact 19,681-byte Gamma compiler tape.

The Beta receipts improve local auditability over Alpha bytes: instructions are
mnemonic, runtime and authored definitions are named, and every block carries an
address assertion. They remain partly disassembly-like because Beta has numeric
targets, repeats the complete Gamma runtime, and prints every word in full-width
hexadecimal. The elaborator is 192 lines and 6,918 native bytes larger than the
direct Gamma compiler, so it does not improve compressiveness at this boundary.

More importantly, the current profiles are not domain-equivalent. A valid
135,009-byte Gamma program with 15,000 literal/drop pairs compiles directly to a
421,123-byte Alpha tape. Its readable Beta expansion reaches Gamma's 1,048,572-
byte output ceiling and exits `Incomplete`; Beta independently admits only
1,048,576 source bytes. Any textual representation of near-limit Alpha output
has an inherent expansion factor, so cosmetic hexadecimal compaction cannot
restore the complete direct domain under those bounds.

The selected bootstrap spine therefore does not change. D82 permits source
elaboration to compete at individual boundaries, but D83 rejects the universal
rule that every rung must emit the immediately previous language. A future
proposal must either revise and re-audit the Gamma-output and Beta-input
profiles, define a compact readable lower representation, or accept a smaller
Gamma source domain; none occurs implicitly.

## D84 — Textual transport grows; Alpha artifact capacity does not

D84 takes D83's first explicit option. Generic Gamma output and Beta source
input each rise to 16 MiB. The change is transport-only: AlphaBootstrapV2 keeps
its exact 1,048,572-byte raw-tape maximum and one-MiB stamped hole. The direct
Gamma compiler and Gamma-to-Beta elaborator independently preflight their
predicted Alpha output against that unchanged maximum before emitting anything.
Beta likewise retains its `0xffffc` assembled-output ceiling.

The Beta source buffer grows from `0x100000..0x200000` to
`0x100000..0x1100000`; it does not overlap compiler code or scratch state. The
Gamma evaluator and compiler-generated Gamma runtime permit `0x1000000` generic
output bytes. Beta remains a 458-line compiler with a 2,135-byte tape; its source
grows from 14,696 to 14,698 bytes and reconstructs the new tape byte-identically.
The Gamma evaluator remains 738 lines and 4,289 tape bytes. The explicit Alpha
preflight grows the Gamma compiler from 532 to 533 lines and its fixed point
from 19,681 to 19,756 bytes.

The strengthened boundary witness contains 37,408 `0x0 drop` pairs. Its
336,681-byte Gamma source elaborates to 2,772,595 bytes of readable Beta, which
assembles to the same 1,048,547-byte Alpha tape as direct Gamma compilation.
One additional pair predicts 1,048,575 Alpha bytes; both routes reject with no
published prefix. Thus D83's observed capacity blocker was a profile choice,
not an inherent failure of prior-rung source output under Alpha's domain.

D84 supersedes D83's capacity-based rejection of the macro-extension topology.
It does not by itself select that topology. Gamma-to-Beta still costs 725 Gamma
lines and a 26,674-byte tape versus the direct compiler's 533 lines and 19,756
bytes, while producing a more inspectable Beta receipt. Full promotion also
still requires Delta-to-Gamma elaboration for algebraic data, exhaustive
`match`, immutable `Bytes`, proper tail calls, application profiles, and exact
resource outcomes. The remaining decision is whole-chain auditability and
compressiveness, not transport feasibility.

## D85 — Gamma emits canonical Beta; Beta alone encodes Alpha

D85 selects the Gamma-to-Beta architecture proven by D83 and made
profile-equivalent by D84. The canonical compiler source is the 725-line
`source/gamma/compiler/gamma_compiler.gamma`. It validates Gamma, assigns final
addresses, enforces the Alpha artifact bound, and emits mnemonic addressed Beta.
It contains no Alpha opcode-byte encoder. The trusted Beta compiler remains the
only selected source-language implementation that encodes Alpha instructions.

The compiler's own canonical expansion is retained as the 3,490-line,
84,796-byte `gamma_compiler.beta`. Running `gamma_compiler.gamma` under the
Beta-authored Gamma evaluator reproduces that receipt; Beta assembles it to the
exact 26,674-byte `gamma_compiler_bytecode.tape`. Running the native tape on the
same Gamma source reproduces the same receipt and tape. Thus the selected
compiler reconstructs without the former direct Gamma-to-Alpha compiler.

The former 533-line direct compiler moves to
`tests/gamma/gamma-to-beta-experiment/direct_compiler.gamma`. It remains only as a differential
comparator. The promoted compiler compiles that comparator through Beta into a
disposable tape; promoted and direct routes then agree on Delta0, the complete
retained Gamma corpus, and D84's 1,048,547-byte near-limit witness. The direct
source supplies no selected bootstrap premise and is deletion-bounded by
stronger checked source-to-source correspondence.

Future selected compiler edges follow the same default ownership rule: Delta
emits canonical Gamma, Epsilon emits canonical Delta, and Omega emits canonical
Epsilon. Their final executable artifacts are obtained by composing the selected
lower compilers. This ruling does not pretend those missing full elaborators
exist: current Delta-to-Gamma evidence covers the scalar Functional Delta
surface only. A higher compiler may depart from prior-rung source output only
after a concrete auditability or profile discriminator defeats elaboration.

## D86 — Beta data is one fixed Alpha word

D86 removes Beta's quoted `db` strings and their independent quote, escape,
separator, and byte-publication scanner. `dw HEXWORD` now emits exactly one
eight-byte little-endian Alpha word through Beta's existing token, hexadecimal,
and word-emission paths. The width is Alpha's fixed 64-bit word, never a host
property.

Bootstrap tables are packed explicitly into these words. The Beta mnemonic
table remains one dense NUL-terminated byte stream at address `0x660`, with one
trailing zero byte. Gamma's short `main`, colon, and semicolon tokens each occupy
one padded word at independently asserted addresses and are compared using
explicit lengths; its builtin names remain one dense NUL-terminated table over
the exact logical range `0x1048..0x10d3`, with padded storage ending at the
asserted address `0x10d8`. Padding is therefore inert and no unrelated string is
admitted into either scan range.

The trusted Beta implementation falls from 458 lines / 14,698 bytes / 2,135
tape bytes to 388 lines / 12,639 bytes / 1,792 tape bytes. The Beta-authored
Gamma evaluator grows from 738 to 753 lines and from 4,289 to 4,312 tape bytes
because three short tokens receive explicit padding. Its complete 29-case gate
passes. The test-owned Gamma reconstructor simultaneously falls from 213 to 186
lines and reproduces the exact 4,312-byte evaluator tape. This accepts trivial
table verbosity and 23 inert bytes in exchange for deleting a distinct trusted
string language and parser.

## D87 — The scalar Delta-to-Gamma compiler becomes the selected foundation

D87 promotes the 550-line Gamma source at
`source/delta/compiler/delta_compiler.gamma` from experiment ownership to the
selected Delta compiler owner. It validates and elaborates the executable
scalar Functional Delta slice to canonical Gamma; the selected Gamma and Beta
compilers then compose that receipt to Alpha. Interpreted and native compiler
execution reproduce the retained scalar receipts, and the resulting programs
execute with the expected observations.

This is an implementation promotion, not admission of the complete Delta edge.
The source still lacks nominal algebraic data, exhaustive `match`, immutable
`Bytes`, complete static checking, proper tail calls, application profiles, and
exact resource outcomes, and therefore cannot compile
`epsilon_compiler.delta`. No `delta_compiler_bytecode.tape` is admitted yet.
The former direct-to-Alpha Functional compiler and State Delta compiler remain
test-owned comparisons; neither supplies selected authority.

## D88 — Typed functional Gamma is evaluated directly by Beta

D88 supersedes D67 through D70 and D87 as selected-topology decisions. The
concatenative Gamma evaluator/compiler and its Gamma-written Delta compiler were
functionally sufficient, but the Delta implementation made their audit cost
visible: compiler state became manually numbered cells, source generation
became packed ASCII words, and small Gamma words expanded into call-dominated
Beta.

The measured 666-line scalar/effect seed expanded to 3,230 Beta lines and 2,842
Alpha instructions. Its 1,678 calls occupied 15,102 of 22,762 tape bytes. The
same tokenizer required 168 generated instructions and 88 calls versus 38
instructions and no calls when written directly in Beta. Counting the
Beta-written evaluator, Gamma compiler, source-augmentation lowerer, and seed
left 2,337 authored lines above the common Beta compiler.

The challenger makes the typed scalar/effect language formerly called Delta0
the Gamma rung. A 921-line addressed Beta evaluator implements its typed scalar
functions, lexical `let`, conditionals, integer operators, forward calls,
sealed input, indexed byte reads, and byte output in a 4,788-byte Alpha tape. It
executes the unchanged 85-line source augmenter, produces its exact richer
source receipt, and evaluates the expanded program to byte 42. Gamma is
therefore evaluated rather than compiled at this boundary; Beta remains the
only Alpha encoder.

The direct evaluator is provisional, not admitted. Calls currently use bounded
recursive contexts: depth 254 succeeds and depth 255 reports `Incomplete`.
Unreachable bodies are not yet statically validated, and checked integer and
complete resource outcomes remain open. These gaps must close without erasing
the measured auditability advantage.

Downgraded material remains beneath the language owners at
`source/gamma/bootstrap/concatenative/` and
`source/delta/bootstrap/concatenative-compiler/`. There is no generic
`source/bootstrap` owner. The selected Delta compiler is open and must be built
in auditable Gamma-authored stages; the downgraded compiler supplies evidence,
not authority.

## D89 — Delta begins with exact nullary algebraic lowering

The first selected Delta compiler stage is 471 lines of typed scalar/effect
Gamma at `source/delta/compiler/delta_compiler.gamma`. It accepts finite
nullary algebraic declarations, erases nominal values to declaration-order
integer tags, and lowers declaration-order exhaustive matches to one generated
lexical binding plus nested scalar conditionals. The reserved `__m` prefix owns
generated match binders. Payload constructors, recursive data, `Bytes`, and the
complete Delta contract remain explicit later stages.

The retained seven-line `Choice` fixture lowers to an exact 159-byte Gamma
receipt and evaluates to byte 9 through the selected Beta-authored Gamma
evaluator. Payload constructors, missing arms, and out-of-order arms reject in
the stage. Ordinary scalar Gamma passes through structurally.

This work exposed a selected-evaluator defect: nested lexical insertion saved a
new value in `r6`, while active-environment duplicate lookup reused `r6` for
name bytes. A second nested `let` therefore stored an identifier byte instead
of its initializer. The evaluator now preserves that value in `r8`; its Beta
source and Alpha tape identities changed while tape size remained 4,788 bytes.
The selected reconstruction, scalar/effect, self-augmentation, and staged Delta
gates all pass.

Nullary sums are the largest algebraic feature that scalar Gamma can represent
without a multiword value convention. The next stage must either establish an
auditable payload representation in Gamma or demonstrate that one additional
Gamma value primitive reduces total trusted complexity. It must not encode
payloads through a lossy integer convention merely to claim feature progress.

The next stage establishes that representation without changing Gamma. A
nominal type whose constructors carry zero or one `Int` field flattens to two
scalar slots `(tag, field0)`. Payload constructors supply both slots; nullary
constructors in the same type supply tag plus zero padding. Function parameters
flatten to corresponding `name__tag` and `name__field0` parameters. A match
stores those slots in generated locals and binds a payload arm through an
ordinary Gamma `let`. Thus `Some 9` is `(1, 9)` and `None` is `(0, 0)` without
loss or a heap primitive.

The selected compiler is now 731 Gamma lines / 28,323 bytes. Its exact payload
fixture lowers to a 221-byte Gamma receipt; both `Some 9` and padded `None`
execute through the selected evaluator and produce the expected match result.
Constructors wider than one field and malformed payload patterns reject. The
next representation boundary is nested nominal and recursive data, not basic
payload transport.

## D90 — One evaluated Gamma substrate carries staged recursive Delta

D90 supersedes D89's temporary flattened payload representation. The new chain
keeps one direct interpretation boundary at Gamma and returns to source
transformation above it. The selected Gamma evaluator now validates every
function body before execution, reuses activations for calls in inherited tail
position, bounds successful output, and provides immutable pairs. Its 1,350-line
addressed Beta source assembles to a 7,368-byte Alpha tape. A 100,000-step
direct-tail witness completes with constant activation and nested-call storage;
malformed unreachable names and arities reject before output.

Immutable pairs cross Delta's recursive representation boundary without
restoring source-visible cells or a general heap API. The selected 796-line
Gamma-authored Delta stage lowers constructors with zero or one `Int` or nominal
field. Payload-bearing values use `(pair tag field0)`; matches evaluate the pair
once and project `first` and `second`. The exact `Option Int` receipt evaluates
to 9, and the recursive unary `Nat` receipt represents three successors as
nested pairs and evaluates to 3. Nullary-only sums retain their smaller direct-
tag representation.

The interpretation boundary scales independently of generated Alpha size. The
selected Delta stage transforms 3,001 functions (66,266 source bytes) into a
66,267-byte Gamma receipt; that exact size and result 199 are gated. Measured
development-host timings were 2.21 seconds to transform and 0.13 seconds to
execute and remain diagnostic only.

`GammaComposedV1` fixes an executable Gamma artifact as the pair of exact
evaluator-tape and exact Gamma-source identities. Invocation only constructs the
already specified length-prefixed request, buffers stdout, and publishes it on
status zero. A late-write-then-trap witness leaves an existing destination
unchanged. This closes executable composition without reintroducing a
Gamma-to-Alpha compiler.

The result is favorable but not an admission. Gamma still needs a complete
capacity proof. Delta still lacks wider constructors, `Bytes`, complete type and
namespace checking, application profiles, and the full Epsilon customer. The
next useful work is widening the pair product representation and adding
immutable bytes in staged Delta, not adding another evaluator rung or returning
to concatenative Gamma.

## D91 — The direct-evaluator chain earns continuation through recursive products

D91 completes the discriminator requested by D90 without claiming a complete
Delta edge. Gamma character literals replace 148 fixed decimal byte writes in
the selected Delta compiler with readable punctuation, whitespace, and letters.
The evaluator grows from 1,350 to 1,472 Beta lines and from a 7,368-byte to an
8,119-byte tape. The selected Delta compiler remains 852 lines; its source grows
by 147 bytes to 34,043 bytes because visible `\s` and quoted punctuation are
slightly longer than some decimal spellings. Only dynamic byte computations
remain numeric.

Delta constructor products are now finite and arbitrary-width: a value is
`(pair tag product)`, and fields form right-nested immutable pairs. The retained
two-field recursive List receipt evaluates to 9. A three-field recursive rope,
written entirely as ordinary staged Delta rather than a Gamma primitive,
supports empty, singleton, concatenation, length, and indexing; indexing the
fixture returns `0x42`, while indexing empty traps. The existing nullary,
payload, recursive Nat, malformed-source, atomic-publication, and 3,001-function
witnesses continue to pass under one fixed 8,119-byte evaluator tape.

This is enough signal to continue the selected architecture. One small direct
evaluator carries increasingly rich source transformers without per-program
Alpha expansion, while generated receipts remain short structural Gamma that a
reviewer can execute independently. Recursive products and Bytes-shaped data do
not require restoring a concatenative machine, mutable source memory, or a new
trusted rung.

This is not admission. The rope witnesses representation adequacy, not Delta's
normative `Bytes` contract. Complete type and namespace checking, checked
arithmetic, exact capacity outcomes, application profiles, a derivation checker,
and the full Epsilon compiler customer remain open. Further Gamma growth must be
justified by a concrete Delta auditability need; the next work belongs in the
selected Delta compiler and its Epsilon customer.

## D92 — Matched direct Delta does not displace minimized Gamma

D92 tests whether Gamma is an unearned intermediate rung rather than assuming
that recursive staged success settles the question. Two conservative evaluator
reductions remove duplicate character-token validation, four unused character
escapes, and one duplicate row-name comparison loop. The selected Gamma
evaluator falls from 1,472 to 1,410 Beta lines and from an 8,119-byte to a
7,690-byte tape. All selected reconstruction, static-validation, 100,000-step
tail, pair, composition, and staged Delta gates continue to pass.

A separate direct Beta Delta evaluator implements the same current structural
profile without Gamma transformation. It directly censuses nominal data,
constructs arbitrary-field recursive values as right-nested pairs, selects and
binds exhaustive declaration-order matches, validates known nominal field
types, constructor arity and pattern shape, and preserves tail position through
matches. Exact Nat, List, Bytes-rope, malformed-source, 3,001-function, and
100,000-node construction/traversal witnesses pass.

The matched measurements are:

```text
                              Lines  Instructions  Labels  Control  Tape bytes
selected Gamma evaluator      1,410         1,151     181      582       7,690
matched direct Delta evaluator 2,019         1,655     262      836      11,004
selected Delta transformer      852 Gamma source lines
```

Direct Delta removes one 852-line higher-level transformer but adds 609
low-level lines, 504 Beta instructions, 81 labels, and 254 control transfers to
the trusted evaluator. It also moves constructor and match meaning out of the
more readable Gamma stage. Neither route yet implements Delta's complete type
relation, checked arithmetic, normative `Bytes`, or application profiles, so
those shared obligations do not favor the direct prototype.

Gamma therefore remains selected. Its evaluator is still on the cusp of
comfortable auditability and should receive further conservative simplification,
but direct Delta is not a trust reduction at matched evidence. The next semantic
work remains in the Gamma-authored Delta compiler and its concrete Epsilon
customer; no additional Gamma primitive is justified by this experiment.

## D93 — Interpreted Forth fixes expansion but not total audit cost

D93 revisits concatenative Gamma without its former source-to-Beta compiler.
One fixed Beta interpreter executes both the Forth-Gamma-authored Delta compiler
and each emitted Forth-Gamma receipt. The experiment reconstructs its symbolic
source from the retained 753-line evaluator and first reproduces the exact
4,312-byte historical tape, so the comparison starts from known behavior rather
than a new interpreter design.

Two customer-driven forms remove the former compiler's worst incidental
opacity. `value name` and `to name` replace 49 pairs of numeric fixed-cell
accessors. `text "..."` emits bounded literal source, replacing 356 packed
`output-word` and `output-byte` operations while introducing no string value or
heap. These additions grow the interpreter to 890 Beta lines and a 5,145-byte
tape. The rewritten Delta compiler is 1,451 Forth-Gamma lines.

The matched static measurements are:

```text
                              Lines  Instructions  Labels  Control  Tape bytes
functional Gamma evaluator    1,410         1,151     181      582       7,690
Forth-Gamma interpreter          890           723     122      312       5,145

functional Delta compiler        852 lines, 80 definitions
Forth-Gamma Delta compiler      1,451 lines, 555 definitions
```

The total authored route is 2,341 Forth lines versus 2,262 functional lines.
The Forth compiler still contains 204 explicit branches, 171 jumps, 77 stack
shuffles, and 20 dynamic cell operations. The trusted root is materially
smaller, but the next-rung implementation remains more fragmented and requires
manual reconstruction of implicit stack effects.

Pure interpretation passes recursive Nat and List, a profile-adapted
three-field rope, the staged malformed cases, empty-rope trapping, and a
100,000-node proper-tail construction/traversal. The rope adaptation renames
helpers reserved by the older compiler and replaces one unreachable Gamma I/O
trap with pure Delta division by zero; it answers representation adequacy, not
exact-source equivalence.

Scale and static assurance remain decisive deficits. Measured interpreted
compilation takes 2.38 seconds for 101 functions, 11.98 seconds for 301, and
119.29 seconds for 1,001; the selected 3,001-function source exceeds a 600-second
timeout. More importantly, the interpreter does not validate unreachable word
names or declared stack effects before execution. Adding indexed declaration
lookup and a whole-program stack-effect contract is the remaining earned-rung
test and will increase the trusted interpreter.

The experiment is retained but does not displace functional Gamma. It proves
that interpretation solves the old generated-Beta pathology and makes a
Forth-like rung plausible; it does not yet make the complete chain easier to
audit. Selection remains unchanged until static assurance and scale are solved
without erasing the interpreter's root-size advantage.

## D94 — Functional Gamma shrinks by centralizing existing invariants

D94 follows D93's negative result by reducing the selected functional evaluator
without changing Gamma meaning. Tail and ordinary calls now share target lookup,
argument evaluation, context allocation, and exact arity checking. Tail and
ordinary `let` forms share binder parsing, initializer evaluation, and lexical
insertion. Three single-use token predicates are inlined at their only owners.

The larger reduction replaces fifteen predicate wrappers and three repeated
expression-head dispatch chains with one explicit classification table. Each
row visibly pairs packed ASCII with a documented class and length; no indirect
jump or computed control target is introduced. Validation groups binary and
unary classes, ordinary evaluation maps binary classes directly to the existing
operator body, and tail evaluation distinguishes only `if`, `let`, known
builtins, and calls.

```text
                              Lines  Source items  Labels  Control  Tape bytes
selected Gamma before D94     1,410         1,151     181      582       7,690
selected Gamma after D94      1,254         1,005     160      459       6,545
```

The evaluator removes 156 lines, 146 source items, 21 labels, 123 control
transfers, and 1,145 tape bytes. Exact reconstruction, all expression forms,
unreachable-body validation, nested ordinary calls, 100,000-step proper tails,
immutable pairs, composed publication, and the complete staged Delta workload
remain checked.

This materially weakens Forth-Gamma's only decisive advantage. Its interpreter
is now 364 rather than 520 lines smaller, while its complete authored route is
235 lines larger and still lacks whole-program stack-effect validation. Gamma
remains selected. Further reductions should continue to centralize duplicated
semantic invariants; they must not trade explicit control for opaque tables or
weaken static validation.

## D95 — Epsilon repetition does not yet justify broadening Delta

D95 tests five proposed Delta mechanisms against the exact 8,733-line
Delta-authored Epsilon compiler. A structured top-level-form inventory finds
seven optional declarations, 25 parse-outcome declarations, 26 recursive list
types, 23 reverse functions, seven catalog lookup results, nine catalog
traversals, 29 span accessors, three candidate types, and six candidate merge
functions.

Each candidate receives an intentionally impossible gross ceiling: every
identified Epsilon declaration and helper disappears, while the corresponding
Delta language, compiler, runtime, and proof machinery cost zero lines.

```text
generic option/result       96 lines
generic list/reverse/count 265 lines
generic catalog/map        206 lines
source span accessors      164 lines
candidate minimum fold      77 lines
combined ceiling           808 lines (9.2%)
```

Constructor shapes prevent several nominal substitutions. Option payloads are
both `0/1` and `0/3`; parse outcomes are `1/2` and `2/2`; catalog results use
four shapes and preserve owner, field/case, state, and data-shape custody. A
generic map would additionally require generic key equality and one indexed
representation while most owner-qualified traversal policy remained in
Epsilon.

Two current-Delta kernels test abstractions that need no hypothetical generic
semantics. Replacing repeated expression `(start, end)` fields with a nominal
`SourceSpan` grows the kernel from 15 to 19 lines and from 597 to 759 source
bytes, although its generated Gamma receipt falls by 95 bytes. Extracting one
three-way offset comparison across census, type-formation, and final-diagnostic
candidate policies grows the kernel from 54 to 59 lines and its receipt from
3,290 to 3,487 bytes. Equal-offset behavior is phase policy: retain-left,
conflict-on-kind, and reason-union respectively.

Generic lists remain the only credible candidate. Twenty-five of 26 recursive
types have the ordinary empty/cons shape, with 167 lines of specialized reverse
functions and 14 lines of specialized counts. A fair implementation requires
parametric ADTs and parametric recursive functions; an erased list would weaken
Delta typing and a declaration macro would not remove traversal duplication.
Reopen this candidate only after complete selected Delta type checking can
measure its actual Gamma implementation and proof cost.

The current Epsilon size is therefore not primarily explained by these five
missing mechanisms. Most growth remains Epsilon-owned parsing, diagnostic
ordering, identity custody, type formation, resolution, control checking, and
Alpha encoding. Delta remains unchanged.

## D96 — Gamma pair references retain evaluator provenance

D96 repairs the selected evaluator to enforce Gamma's existing private-pair
contract. The prior implementation recognized a pair by heap range, node
alignment, and an in-memory marker alone. After allocating the first node, the
ordinary integer literal `33554432` therefore forged its address; arithmetic
could produce the same word. `first` and `second` accepted either value even
though the language permits pair references to be produced only by `pair` and
does not expose addresses.

Runtime values now carry one private kind word alongside the payload through
the temporary stack, lexical rows, arguments, returns, tail transfers, and pair
fields. `pair` alone creates the pair kind. Projections require that kind before
checking the node's physical range, alignment, and marker, then restore the
selected field's kind. Integer literals and operators create scalar kind;
conditions, operators, compiler effects, and `main` require it. This also
restores D66's ruled authored trap for pair equality while preserving nested
pairs and pair values across ordinary and proper-tail calls.

The explicit representation cost is 60 Beta lines and 335 Alpha-tape bytes:

```text
                              Lines  Source items  Labels  Control  Tape bytes
selected Gamma after D94      1,254         1,005     160      459       6,545
selected Gamma after D96      1,314         1,058     164      474       6,880
```

The selected gate pins rejection of literal and arithmetic address forgeries,
pair-valued equality and conditions, and preservation through a proper-tail
relay. Exact reconstruction, the 100,000-step tail witness, composed
publication, and the staged Delta workload remain required. This is an
enforcement correction, not a new Gamma feature; the complete capacity proof
remains open.

## D97 — Gamma evaluator closes its bounded outcome profile

D97 closes the `GAMMA-EVALUATOR` implementation task without broadening Gamma.
The exact profile now names every retained representation and effective limit:
16,777,216 request bytes, 4,096 functions, 65,536 active bindings, 524,288
tagged temporary values, 255 nested expression lists, 256 live call contexts,
257 reachable function frames, 5,033,164 immutable pairs, and 67,108,864
successful output bytes.

Function, environment, value, frame, context, pair, and output allocation all
preflight the complete next extent before mutation. Function and environment
insertion formerly reported `Incomplete` after filling their last physical
slot; their checks now occur before address derivation, so the published exact
counts complete and the adjacent insertion returns status 3 without an
out-of-range store. The call-context check likewise admits its complete 256-row
logical profile before refusing the adjacent context. Exact/adjacent gates pin
the function, syntax-depth, and ordinary-call boundaries; the request edge and
larger linear arenas follow from the same checked extent arithmetic in the
exact hashed source.

The hidden Alpha return stack is contained independently. Census admits at
most 255 nested expression lists, ordinary evaluation admits at most 256 live
non-tail contexts, and proper-tail paths add no Alpha recursion. The evaluator
call graph has no other recursive cycle. Even the conservative bound of 16
Alpha return slots per expression level per active Gamma frame plus 512 fixed
helper slots consumes 8,425,472 bytes, while the pair heap reserves 33,554,432
bytes below Alpha's initial stack pointer.

The outcome audit also found that dynamic `/` and `%` had leaked Alpha's
illegal-instruction trap for zero and `INT64_MIN / -1`. The evaluator now
prechecks both cases and returns Gamma authored-trap status 2; all four
division/remainder cases are gated. Framing or static source defects remain
status 1, every bounded-storage refusal is status 3, evaluator contradictions
are status 4, and nontermination remains divergence rather than a fabricated
resource result.

The final exact subject is 1,325 addressed-Beta lines and a 6,934-byte Alpha
tape. Canonical Beta reconstruction, unreachable-body validation, the
100,000-step proper-tail witness, pair provenance, composed atomic publication,
and the complete staged Delta workload remain green. Beta-root audit and the
Gamma derivation checker remain open independent tasks; they do not reopen the
evaluator's language or resource behavior.

## D98 — The admitted Beta root is a finite decoded subject

D98 closes `BETA-ROOT-AUDIT` over the exact cold-start compiler rather than
treating self-reconstruction as semantic proof. The bound subject is the
12,640-byte, 388-line `beta_compiler.beta` source with SHA-256
`3ea0b6d4d8651bddf2aaeb2176009706a0119942c8c2a072e03f6f8876eef53a`
and its 1,792-byte Alpha tape with SHA-256
`b5c3b23c945a250d03e16e66126b4b783573bb8d15139de94a2c8f69fc6ac24f`.

The tape contains 1,632 instruction bytes followed by a 160-byte immutable
mnemonic table. Independent decoding finds 257 Alpha instructions; every
instruction is reachable from address zero, and all 90 branch/call targets land
on instruction boundaries below the table. The readable source partitions the
tape through 53 exact address assertions, 257 instruction items, and 20 `dw`
items. The published audit groups the control flow into entry/read, program
scan, tokenizer, hexadecimal word/digit/nibble, mnemonic lookup, operand
emission, and `dw` emission regions and maps each to the corresponding Beta
rule.

The memory map is closed: tape `0x000000..0x000700`, one scratch word at
`0x080060..0x080068`, and a checked 64-MiB source buffer at
`0x100000..0x4100000`. Output is streamed and successful artifacts are bounded
to `0xfffffc` = 16,777,212 bytes. The call graph is acyclic and needs at most
six Alpha return addresses. Source pointers, table reads, and scratch access
therefore remain disjoint and in range; the compiler contains no `div`, `mod`,
computed control, heap, or other route to an Alpha trap or undefined memory.

This audit corrects two stale written numbers: the source is 12,640 rather than
12,639 bytes, and the current Beta compiler publishes `0xfffffc`, not the old
`0xffffc`, output maximum. The current Alpha seeds and Alpha semantics likewise
own a 16-MiB stamped hole and 16,777,212-byte raw-tape maximum. D98 supersedes
only the obsolete one-MiB capacity clauses in D23 and D84; their topology and
transport findings remain historical. No opcode or Beta language form changes.

The retained gate independently binds and partitions the source, reconstructs
the Beta relation, decodes the raw tape from Alpha's opcode table, checks the
closed mnemonic table and full control-flow reachability, then runs exact
self-reconstruction. The independent Python relation remains diagnostic, not a
bootstrap stage or authority. The finite source/tape pair, written Alpha/Beta
semantics, published operation audit, and audited native Alpha realizations are
the trust boundary.

## D99 — Delta global declarations are checked before staged emission

D99 adds the first whole-program census to the selected Gamma-authored Delta
compiler. Before emitting any Gamma byte, the stage now requires the normative
top-level shape: zero or more nonempty data declarations followed by one or
more function declarations, with exactly one `main`. Type names are unique
among types, constructor names are globally unique among constructors, and
function names are unique among functions. The three namespaces remain
separate, so `(data Token (Token Int))` is accepted rather than accidentally
colliding a type with its constructor.

The census retains exact source-byte names in three persistent bitwise tries.
Each byte contributes all eight bits, and terminal identity is explicit; there
is no hash, collision assumption, mutable table, or host lookup. Rebuilding
only the traversed immutable path preserves Gamma's value model while avoiding
repeated whole-source scans. A rescan prototype exceeded 30 seconds on the
3,001-function fixture and a linked-row replacement exceeded 60 seconds. The
selected trie completes the same staged transformation in about 8 seconds on
the development host, within the retained 30-second per-evaluation gate.

The selected source is now 1,004 lines and 39,769 bytes, with 90 definitions and
353 lexical `let` binders. The 66,266-byte scale source still produces its exact
66,267-byte Gamma receipt and byte 199. Duplicate type, constructor, and
function declarations, data after a function, empty data, and missing `main`
now reject through the selected evaluator. A 200-byte identifier also compiles,
covering the current 56-byte maximum in the complete Epsilon source closure
without approaching Gamma's non-tail-context boundary.

This pass also removes a false purity witness. The retained Bytes-shaped rope
had used Gamma's `(read (input))` as an unreachable trap, although Delta has no
effects. It now uses Delta's authored division-by-zero trap; the source and
receipt shrink from 789/1,040 to 782/1,033 bytes. This correction does not claim
normative `Bytes` or complete expression checking. Those, checked arithmetic,
local scopes and types, application profiles, canonical failure ordering, and
proper-tail closure remain part of `DELTA-COMPILER`.

The old Forth-Gamma and direct-Beta experiments do not implement this new
global census. Their historical matched comparisons remain valid at D92/D93,
but current aggregate line totals are no longer matched evidence and do not
reopen the selected topology.

## D100 — The staged Delta edge enforces its source-byte envelope

D100 moves Delta's normative textual-ASCII boundary into the selected
Gamma-authored compiler. A constant-space prepass admits exactly HT, LF, CR,
and bytes `0x20..0x7e` before tokenization or output. NUL, another control byte,
DEL, and a high byte are retained rejection witnesses; admitted tab and CR/LF
whitespace compile to the same canonical Gamma receipt as spaces and LF.

This closes only the byte envelope. Identifier spelling, reserved words,
integer-literal range, closed expression forms, and full static typing remain
open compiler work. The exact selected subject is now 1,022 Gamma lines and
40,278 bytes, with 93 definitions and 354 lexical `let` binders. The global
census, 200-byte identifier, recursive products, Bytes-shaped rope, and
3,001-function witnesses remain green. The retained Forth-Gamma and direct-Beta
experiments do not implement this prepass and therefore remain unmatched
historical evidence rather than alternate authority.

## D101 — Delta names and signed literals stop inheriting Gamma syntax

D101 gives the selected stage Delta's own lexical atoms instead of passing any
Gamma token through. Type and constructor names must begin uppercase; function,
parameter, `let`, pattern-binder, and value names must begin lowercase or `_`;
all remaining bytes are ASCII letters, digits, or `_`. The rule is enforced at
global declarations, nominal field and result types, parameters, local binders,
patterns, expression atoms, and application heads.

`data`, `def`, `if`, `let`, `match`, `eq`, `lt`, `Int`, `Bytes`, and the five
closed `bytes_*` builtin names are reserved rather than available for authored
declarations or locals. A small exact packed-byte predicate centralizes these
fixed spellings; every candidate's length and every byte are compared, so the
encoding is not a hash admission. Arithmetic punctuation remains admissible
only as a one-byte application head.

Decimal atoms are now scanned into a negative accumulator with a preflight at
`-922337203685477580` and final digit 8. This admits exactly
`-9223372036854775808..9223372036854775807` without overflowing Gamma while
checking, and rejects a sign without digits, nondigit suffixes, and either
adjacent out-of-range value. Exact minimum and maximum witnesses compile.

The change exposes and removes a second invalid rope fixture assumption. Its
user-defined helpers had occupied the reserved `bytes_empty`, `bytes_single`,
`bytes_length`, `bytes_concat`, and `bytes_get` names. They are now `rope_*`,
matching the already-correct Forth comparison, and the pure authored
division-by-zero trap remains. The exact source/receipt pair is 767/1,018 bytes.

The selected compiler is now 1,192 Gamma lines and 45,840 bytes, with 113
definitions and 364 lexical `let` binders. This is still a staged subset:
scope resolution and duplicate-local checks, closed form arities, static type
relations, checked runtime arithmetic, normative `Bytes`, application profiles,
failure ordering, and proper-tail closure remain open. The Forth and direct-Beta
experiments lack the new lexical contract; their smaller current aggregate is
not a matched architectural comparison.

## D102 — Delta calls resolve through the checked global census

D102 turns the global function trie from a duplicate set into an emission-time
symbol table. Each terminal stores declared arity plus one, leaving zero as the
unresolved sentinel. The whole-program census validates and counts every
parameter list before retaining that payload, returns the immutable global
context, and the emitter threads the same checked value through every nested
expression and match arm. Lookup remains exact source-byte trie traversal; it
does not rescan the program or consult a host table.

Every application now has an exact argument count. User calls, including
forward and mutual calls, use the retained declaration arity. `if` requires
three arguments; integer operators require two; and each closed `bytes_*` form
has its normative zero-, one-, or two-argument shape. Too few or too many
arguments reject rather than being inherited by Gamma parsing.

This also closes the staged purity leak without reserving ordinary names.
Undeclared `(input)`, `(read ...)`, `(write ...)`, `(pair ...)`, `(first ...)`,
and `(second ...)` cannot resolve as Delta functions and reject. A source may
still declare an ordinary pure function named `read`; a retained witness does
so and evaluates to byte 7. Non-`main` definitions and calls receive one
injective `__d_` Gamma prefix, while `main` retains the evaluator entry name.
This prevents Gamma builtin dispatch from capturing a valid Delta declaration.
Delta owns declarations and meaning, not a blanket ban on coincidentally
Gamma-like spellings.

The 3,001-function transformation remains below 10 seconds on the development
host. Its canonical receipt grows from 66,267 to 78,271 bytes because 3,000
non-entry definitions and the selected call carry the prefix. The exact
selected subject is now 1,276 Gamma lines and 49,000 bytes, with 120 definitions
and 379 lexical `let` binders. Local-variable resolution,
duplicate active binders, expression/result type relations, checked runtime
arithmetic, normative Bytes lowering, application profiles, failure ordering,
and proper-tail closure remain open. The retained Forth and direct-Beta
experiments have neither this symbol table nor matched lexical coverage and do
not reopen selection.

## D103 — Derived generic lists fail authored-source break-even

D103 implements D95's remaining candidate rather than assigning generics a free
cost. A 292-line Gamma-authored two-pass elaborator accepts:

```text
(list Type Empty More Element reverse_name count_name)
```

and emits ordinary monomorphic Delta data, reverse, and count declarations. `_`
omits either helper. The first pass emits all data declarations and the second
emits helpers and ordinary definitions, preserving D99's required top-level
order. The exact 25-line Epsilon specification expands to 25 list types, 22
reverse functions, and three count functions. All 50 generated forms are
alpha-equivalent to the current Epsilon source; the only normalization is the
irrelevant count-parameter spelling. A smoke program composes through the
selected Delta compiler and executes generated reverse/count helpers to result
2.

The real replaceable family excludes the three-field Alpha trie and the
policy-bearing control-ledger reversal:

```text
explicit Epsilon family          248 lines / 11,525 bytes
Gamma list elaborator            292 lines / 13,200 bytes
derived Epsilon specifications    25 lines /  3,565 bytes
derived route total              317 lines / 16,765 bytes
net                               +69 lines / +5,240 bytes
```

The implementation is favorable to the feature: it avoids generic type
parameters, generic checking, generic representation, and instantiation
semantics. Despite that, it loses raw authored size, moves implementation into
the lower rung, and adds a source-transformation relation. A fused Delta
implementation would need to cost fewer than 223 Gamma lines merely to tie line
count, while also extending type, constructor, arity, match, and generated
helper resolution.

At the D101 checkpoint, expansion of all 25 specifications took 0.55 seconds;
the selected compiler took 188.10 seconds to lower one synthetic source with
all 25 instantiated types. D99's trie-backed global census removed the earlier
global-name cliff, but other whole-source rescans remained. This timing is
diagnostic and does not affect Delta meaning; D102's later call-resolution
change is not included in that measurement.

Generic lists therefore fail the current earned-feature test. All five D95
candidates are rejected, and Delta remains unchanged. Reopen derived or
parametric lists only if another independently justified feature supplies most
of the required compiler machinery and a fused implementation demonstrably
falls below the 223-line break-even including its proof obligations.

## D104 — Lowering names are outside the authored Delta alphabet

Compiler-generated match temporaries now begin with `$`: `$m` holds a nullary
tag, while `$v` and `$p` hold the evaluated nominal value and its payload.
Gamma admits these names, but Delta's identifier grammar does not admit `$` at
all. The two namespaces are therefore disjoint by construction. Lowering no
longer relies on a supposedly reserved authored prefix such as `__m`, and an
exact match witness retains and reads an authored `__m63` local at the source
offset that the old lowering also named `__m63`.

This is a compiler hygiene property, not a new Delta reservation. A future
generated name should use the same outside-the-source-alphabet rule; expanding
Delta's identifier alphabet to include `$` would require revisiting this
decision first.

The global census also uses its existing persistent exact-name trie to reject
duplicate parameters within each function. This closes the first local-scope
ambiguity before emission without imposing a name-length bound or host lookup.
Duplicate active `let` and pattern binders, unknown local references, and the
complete expression/result type relation remain open.

The exact selected subject is now 1,280 Gamma lines and 49,175 bytes, with 120
definitions and 377 lexical `let` binders. Generated-name shortening changes
only canonical receipt text: the nullary, payload, recursive, List, and rope
receipts are respectively 165, 230, 251, 328, and 1,056 bytes. Their evaluated
outcomes are unchanged, and the 3,001-function witness remains 78,271 bytes.

## D105 — Delta local names resolve in immutable lexical environments

The selected compiler now performs a scope-validation pass after the complete
global census and before emitting any Gamma. Each function starts with an exact
byte-indexed trie of its parameters. A `let` initializer is validated against
the outer root and its body against a newly extended root. Each match arm is
likewise checked against its own extension of the outer root. The persistent
structure makes lexical pop structural: siblings, conditional branches, and
disjoint arms retain the unextended root and may reuse a spelling.

Every bare value atom must resolve in the active local environment. A global
function name used as a value therefore rejects, while `(f f)` remains valid
when the head resolves to the global function and the argument resolves to an
active local. A later parameter, `let`, or pattern binder may not duplicate an
active local. Retained witnesses cover an invisible self-reference in a `let`
initializer, nested conflicts, duplicate pattern binders, outer/pattern
conflicts, escaped binders, legal sibling reuse, legal arm reuse, and the
function/local grammar distinction.

This closes name scope only. The environment does not yet carry types, so
argument, initializer, branch, match-arm, and declared-result type relations
remain open. The exact selected subject is now 1,457 Gamma lines and 56,262
bytes, with 131 definitions and 427 lexical `let` binders. The 3,001-function
transformation remains below 14 seconds on the development host and produces
the unchanged 78,271-byte receipt.

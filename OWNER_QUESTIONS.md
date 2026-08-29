# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Last pruned: 2026-08-29.

## Q1 — Strict SSH trust and credential authority

### Context

Package source requests admit HTTPS, SSH URLs, and SCP-like SSH locators. The
resolver seals Git configuration, selects and hashes one exact SSH client, uses
batch mode, disables user SSH configuration, and requires strict host-key
checking. It still consumes the invoking user's default known-host and key
files. The strict resolver contract requires explicit host-trust evidence and a
closed credential-provider class before an accepted source receipt can claim
that ambient authority was excluded.

### Problem statement

No trusted command/resolver input currently supplies SSH host trust or
credentials. Treating the user's default files or agent as implicit authority
would make resolution depend on ambient mutable state that is absent from the
source question and receipt. Letting `build.omg` or dependency source choose
those values would grant untrusted package code transport and secret authority.
Persisting private key material in `omega.lock` would expose secrets while still
failing to define which process may use them.

### Proposed direction

Require trusted command infrastructure to provide one explicit resolver-owned
SSH authority input. It binds the requested host to exact known-host evidence
and selects one closed credential-provider class, such as a specifically opened
key capability or an explicitly designated credential broker. The fetch helper
receives only those capabilities; home-directory discovery and an ambient agent
remain disabled. The resolver receipt records commitments to the host evidence,
provider class, and effective endpoint, never secret bytes. This authority is
deployment input rather than package source, dependency identity, or a portable
producer claim.

### Alternates

- Acceptable for the first strict release: admit only HTTPS to accepted
  resolution while SSH remains available solely through the clearly diagnostic
  resolver path.
- Acceptable: support an explicitly selected SSH agent or platform credential
  broker as a distinct provider class, provided its identity and authority are
  bounded and receipt-visible rather than inherited.
- Tempting but wrong: inherit `~/.ssh`, a default agent, or system Git/SSH
  configuration and call strict host-key checking sufficient custody.
- Tempting but wrong: let a package, dependency declaration, repository, or
  `build.omg` select host trust or credential material.
- Tempting but wrong: serialize private keys, tokens, or reusable credentials in
  `omega.lock` or source-resolution evidence.

## Q2 — Close the Delta v1 semantic contract

### Context

Delta is the independent C-like compiler-host language accepted by the
Gamma-written Delta compiler and used to author the first full Omega compiler
implementation `D`. `source/delta/LANGUAGE.md` now separates Delta from the
deleted Beta-written Delta-to-Gamma route, but a corpus audit found choices that
change source validity or observable meaning. Implementing them ad hoc inside
`delta_compiler.gamma` would make the compiler, rather than the language
contract, define Delta.

The deleted native compiler prototype demonstrated the intended plain core:
records, fixed arrays, `i32`/`u8`, receiver machines, states, arithmetic,
strings, and Console calls. It does not require packages, attributes,
contracts, proof syntax, range types, or general domains. The broader test
corpus does exercise several of those unsettled forms.

### Problem statement

The following decisions must be closed together:

1. `Incomplete` is currently listed as a `DeltaV1` result and later called not
   a Delta result. Decide whether it is a language observation, an
   execution-profile outcome, or an outer compiler/checker status. Enumerate
   every exact reject code/offset and trap kind so failure is deterministic.
2. Decide whether keywords are reserved or contextual. The grammar reserves
   `state`, `transition`, `machine`, and others, while
   `contextual-state-identifiers.delta` uses them as identifiers. The reserved
   set also currently omits `use`, `requires`, `ensures`, and `assert`.
3. Either define or remove v1 `use`, attributes, domains, `requires`,
   `ensures`, and `terminates by`. Existing tests additionally use field
   domains, a special `result` binding, and `i32 in 0..N` range types that the
   grammar does not admit. Define whether `min`/`max` are reserved builtins,
   shadowable builtins, or ordinary declarations.
4. Define the sealed boundary ABI: whether bare `read_byte`/`write_byte` are
   sugar for `self.console`, the type/lifetime of decoded string literals, and
   their conversion to `&[u8]` for `write_line`.
5. Define the outcome of a scalar transition with no matching arm and no `_`.
6. Reconcile one Delta translation unit with the package-resolved closure `D`:
   select either one canonical packed/resolved unit supplied as compiler input
   or a real Delta module/import model with an exact closure owner.

### Proposed direction

Keep Delta v1 deliberately small and sufficient for `D`:

- make `Incomplete` an explicit verifier/compiler resource outcome outside
  source execution, while `Exit`, `Reject`, `Trap`, and `Diverges` are Delta
  observations; bind private-capacity failures to `Incomplete` without partial
  artifact bytes;
- reserve the complete keyword set and rewrite tests that pin the deleted
  translator's contextual-keyword behavior;
- omit `use`, attributes, range types, proof-oriented contracts, and
  `terminates by` from v1 unless a concrete `D` implementation need is shown;
  retain only arithmetic-domain placements that receive complete rules;
- define string literals as immutable call-scoped byte views, and define bare
  byte I/O as exact sugar for the single threaded Console capability;
- trap deterministically on a nonexhaustive scalar transition; and
- give the Delta compiler one already resolved, canonically packed translation
  unit, leaving package resolution outside Delta semantics but inside exact
  source custody.

This direction minimizes the Gamma compiler and keeps package/proof semantics
out of Delta without weakening its ability to host a robust compiler.

### Alternates

- Acceptable: retain contracts and arithmetic domains if their complete static,
  dynamic, failure, and result-binding rules are fixed now and needed by `D`.
- Acceptable: use contextual keywords if every grammar position has an
  unambiguous deterministic resolution rule and the complexity is justified.
- Acceptable: make `Incomplete` part of a larger profiled evaluation judgment,
  provided it is no longer simultaneously denied as a Delta result and its
  relation to divergence/exhaustion is exact.
- Tempting but wrong: implement whatever the 75 historical positive files happen
  to accept and call that the Delta specification.
- Tempting but wrong: retain the old translator's private capacities, exit
  codes, or Darwin output behavior as language rules.

## Q3 — Select one typed executable Gamma contract

### Context

Gamma is the safe definitional rung used to write the Delta compiler. The
current repository implements two disconnected surfaces:

- `interp.beta` accepts untyped `def* EXPR` programs and evaluates a final
  expression; and
- `typeck.beta` accepts typed `data* def*` programs, checks every definition,
  and has no executable entry or final expression.

The intended Delta compiler must be typed and executable. Gamma is otherwise
pure and currently has no source-level byte-I/O effects. A Beta-written
Gamma-to-Alpha compiler therefore cannot choose its entry, byte-stream ABI,
outcome mapping, or fuel meaning without adding language semantics.

### Problem statement

Fix one canonical contract for:

1. typed executable grammar and erasure/evaluation after type checking;
2. a unique entry declaration;
3. a pure compiler ABI mapping sealed Alpha stdin bytes into a Gamma value and
   the accepted Gamma result into exact tape/stdout bytes;
4. malformed source, type error, trap, private exhaustion, fuel exhaustion,
   divergence, and the no-partial-artifact rule; and
5. whether the current 50,000,000-call evaluator fuel is language meaning or a
   verifier-selected resource-profile parameter.

The contract must support arbitrary constructor arity and realistic functions
with more than Beta's four register arguments. Those are implementation ABI
requirements, not reasons to extend Beta or Alpha.

### Proposed direction

Use one typed `data* def*` source language with a distinguished declaration of
type equivalent to:

```text
main : Bytes -> CompileOutcome
```

`Bytes` and `CompileOutcome` are ordinary closed Gamma data types fixed by the
compiler-entry profile. The generated Alpha runtime alone reads sealed stdin,
constructs `Bytes`, invokes pure Gamma `main`, and serializes the selected
outcome. Successful artifact bytes are exact; rejection, trap, or private
resource exhaustion publishes no partial tape. Call fuel and heap/tape limits
are explicit resource-profile parameters and cannot change Gamma meaning.

The Beta-written compiler then type-checks before emission, erases types into a
defined runtime representation, uses a custom arbitrary-arity Gamma frame ABI,
preserves proper tail calls, and emits Alpha tape directly. `interp.beta` and
`typeck.beta` remain semantic oracles/components only while they expose distinct
failures.

### Alternates

- Acceptable: retain a final typed expression rather than a named `main`, if
  its type and byte-stream adapter are equally unique and explicit.
- Acceptable: define a different closed `Bytes`/outcome carrier, provided the
  mapping to sealed input, exact output, and failure status is canonical.
- Tempting but wrong: compile the untyped interpreter language while describing
  Gamma as the typed safety rung.
- Tempting but wrong: publish an interpreter plus serialized AST as the final
  compiler architecture; it duplicates the evaluator in every tape and leaves
  the compiler dependent on interpreter capacities and dispatch cost.
- Tempting but wrong: make Alpha I/O effects directly callable from arbitrary
  Gamma source merely to avoid defining the compiler-entry adapter.

## Q4 — Fix Beta block formation and definite-initialization reachability

### Context

Beta procedures have one entry block and an ordered state-machine CFG. A call
binds only parameters; every other local becomes initialized when its `let` is
executed. The Alpha-written compiler currently proves only that a referenced
slot was declared earlier in source order. A jump may skip that initializing
store, so source-order lookup is not the required every-path property.

The implementation audit produced a bounded forward must-analysis over the
entry plus at most 1,024 state blocks. It records per-block reads-before-write,
writes, reachability, fallthrough, and per-transition write prefixes, then
intersects initialization at joins. That machinery fits below the existing
1 MiB compiler source buffer and needs no new Beta feature.

### Problem statement

Two language-formation choices remain unstated:

1. `LANGUAGE.md` recursively includes `state` as a statement in any block,
   while `SEMANTICS.md` describes a flat entry block followed by ordered state
   blocks. Decide whether nested states and loose ordinary statements after the
   first state are valid and, if so, exactly how they become blocks and acquire
   fallthrough edges.
2. Define the static reachability criterion for initialization. In particular,
   decide whether every guarded `to S when e` contributes both its target and
   false-continuation successors even when `e` appears constant, and whether
   reads in syntactically unreachable blocks are checked.

These choices change which Beta programs are well formed. They cannot be
selected solely inside `beta_compiler.alpha`.

### Proposed direction

Keep Beta's state graph flat and mechanically auditable:

- a procedure body consists of ordinary entry statements followed by zero or
  more sibling `state` declarations;
- a state body contains ordinary statements but no nested `state` declaration;
- no loose ordinary statement follows the first sibling state declaration;
- sibling state blocks fall through in source order, while `return` and an
  unconditional `to` terminate the remaining path in their block;
- a guarded `to` always contributes both target and false-continuation edges
  for definite initialization, without constant folding or call reasoning; and
- only blocks reachable from that procedure's entry under those syntactic
  edges are checked for reads-before-initialization. Every procedure is analyzed
  independently, including one with no callers.

This gives one deterministic, terminating byte-vector must-analysis and keeps
semantic acceptance independent of optimizer sophistication.

### Alternates

- Acceptable: permit nested states if flattening, name scope, source order, and
  fallthrough are specified without depending on compiler traversal accidents.
- Acceptable: require every syntactic block, including unreachable blocks, to
  be initialization-safe; this is simpler but deliberately rejects more dead
  source and must be stated as formation rather than runtime meaning.
- Tempting but wrong: infer a constant guard or callee result in the bootstrap
  compiler to recover initialization, making well-formedness depend on an
  increasingly sophisticated evaluator.
- Tempting but wrong: zero-initialize generated frame slots and call the gap
  closed; that changes Beta's written local semantics and hides skipped stores.

## Q5 — Select the canonical Beta compiler outcome carrier

### Context

The canonical Alpha-written Beta compiler now publishes its complete Alpha tape
only after two checked passes and fixup resolution. Every current failure leaves
artifact stdout empty, but the boundary still identifies failure with numeric
Alpha halt values: malformed-source paths expose parser phase numbers, source
capacity uses another number, and internal replay/fixup failures use another.
`TASKS_BOOTSTRAP.md` requires typed `Reject` and private-budget `Incomplete`
outcomes rather than treating host process status as the semantic contract.

This question concerns compilation itself. Status 250 for a generated Beta
program's data-stack exhaustion and status 251 for its invalid raw-memory access
are runtime containment outcomes and must not be conflated with compiler
failure.

### Problem statement

Select one closed compiler-boundary result and its exact Alpha realization:

1. Define the cases, at least successful artifact, malformed/invalid source,
   private producer exhaustion, and compiler invariant failure.
2. Decide which rejection reason, source offset, resource kind, limit, and
   requested amount are observable, and how they are represented within
   Alpha's sealed stdin/stdout/halt observation model.
3. Preserve raw tape bytes as the exact successful artifact while ensuring a
   failed run cannot be mistaken for a partial tape. No shell wrapper or host
   script may supply the missing type distinction.
4. Classify identifier, syntax-nesting, procedure/state/edge/call/slot tables,
   internal labels/fixups, private tape extent, and source extent consistently
   as language rejection, profiled `Incomplete`, or internal failure.

### Proposed direction

Use a proof-level closed sum such as:

```text
BetaCompileOutcome =
    Complete(Bytes)
  | Reject(phase, source_offset, reason)
  | Incomplete(resource, limit, requested)
  | InternalFailure(code)
```

`Complete` alone publishes the raw Alpha tape. Every other case publishes no
artifact bytes. Bind an exact Alpha-level encoding of the selected case and
fields, then make gates decode that encoding into the sum; a Unix shell's
truncated exit status is only a realization detail, never the definition.
Malformed or statically invalid Beta maps to `Reject`, checked private ceilings
map to `Incomplete`, and disagreement between the two compiler passes or an
impossible fixup/table condition maps to `InternalFailure`.

### Alternates

- Acceptable: publish a canonical tagged diagnostic byte sequence on failure,
  provided it is unambiguously not an artifact and no partial tape precedes it.
- Acceptable: keep failure stdout empty and use a compact halt-word encoding if
  every required field and the host-realization projection are exact.
- Tempting but wrong: assign a few undocumented process exit numbers and call
  them typed outcomes.
- Tempting but wrong: prepend a success tag to Alpha tape and thereby change the
  canonical artifact bytes or require a stripping stage.

## Q6 — Compose the exact Alpha-to-Beta edge within checker capacity

### Context

The first bootstrap edge must establish exact correspondence between the
78,109-byte `beta_compiler.alpha` source and its 20,977-byte Alpha tape. The
generic checker binds both raw subjects and can check a balanced trace, local
assembly grammar, widths, label uniqueness, absolute fixups, and complete
source/tape exhaustion without an assembly-specific kernel rule.

The selected certificate shape required one closed
`VERIFY(source, tape, trace) = ACCEPT` equality discharged wholly by
computation and reflexivity. Compiler-scale prototypes established a hard
implementation conflict. Dynamic balanced cutting accepts 714 canonical leaves
and fails at 715. Structural recursion traverses all 6,467 leaves in 0.704
seconds, but adding local parsing exhausts the arena; even a content-free visit
of every raw source byte fails. Sequential state threading instead exhausts the
generated semantic stack. The checker reclaims normalization scratch only after
each complete equality decision, so a single root conversion retains every
branch temporary.

A checker-native carrier control split the same source into 112 named equality
decisions, visited every byte, and composed their checked propositions with
`use`; it accepted in 1.192 seconds. This establishes that per-equality scratch
reclamation is viable. It does not establish the required exact boundary chain
or any assembly semantics.

### Problem statement

Choose how exact compositional work becomes the one admitted root edge without
weakening subject identity, partition/exhaustion, grammar, label, fixup, or tape
equality. This is an architecture choice: further trace compression or another
local parser cannot change the lifetime of temporaries inside one equality.

### Proposed direction

Permit one fixed artifact-owned proof to check bounded, subject-bound chunk
equalities by reflexivity, then derive the single root edge equality through the
existing checked equality congruence/`eqelim` rules. Every chunk must expose
exact source and tape boundary states; composition must prove adjacency, order,
unique ownership, root start/end, and full exhaustion. Chunk goals, theory,
subjects, and the final proposition remain owner-fixed. No host result, status
code, hash, generated receipt, or producer assertion becomes a premise.

This changes proof composition, not the trusted calculus or assembly meaning.
It also uses the checker's existing sound scratch-reclamation boundary instead
of adding an assembly-specific evaluator path.

### Alternates

- Acceptable if kept fully generic: implement sound branch-local reclamation or
  garbage collection in every checker implementation, prove that live normal
  forms cannot reference reclaimed nodes, and retain the single-reflexivity
  certificate shape.
- Tempting but wrong: raise or bypass an undocumented memory bound until this
  one certificate happens to fit.
- Tempting but wrong: split the source into independent local claims without
  checked boundary-state composition and call their conjunction the edge.
- Tempting but wrong: restore the deleted status ledger, add an assembly
  primitive, trust a producer receipt, compare hashes, or weaken exact total
  partitioning.

## Q7 — Own the ranked native-fuel sponsor entry

### Context

The exact ranked-`u32` countdown now reaches directly metered final-image,
format-43 installation, and source-free native-artifact custody on Linux x86-64
and AArch64. Transfer-runtime encoding and replay already retain exact activation
slots, interrupted/saved/restored state, transfer/resume bytes, sponsor-stack
demand, relocations, and full unrelocated/final text fingerprints. Ranked
transfer admission also requires the activation record to save the actual ABI
rank carrier (`rdi` or `x0`).

The runtime binder requires the sponsor symbol to be an existing nonempty text
function in the metered object. The admitted ranked artifact deliberately owns
exactly one semantic function: the countdown itself. Naming it as sponsor would
make exhaustion call the countdown under an unrelated sponsor ABI and is not a
valid execution model. Appending an unowned compiler helper would contradict
the exact one-function artifact and hide a new authority edge.

### Problem statement

Choose which owner supplies the sponsor entry and how that ownership joins the
ranked image without turning runtime scaffolding into a second semantic source
tree. This blocks honest native rank 0, 1, and 3 execution/schedule comparison;
it does not block direct metered publication.

### Proposed direction

Bind the transfer runtime to an admitted installed sponsor route owned outside
the ranked semantic object. The installation/external-root join should name the
exact sponsor artifact, calling contract, target, and provision, while the
compiler-owned transfer stub remains the only appended runtime text. Preserve
the one-function ranked semantic identity and require source-free replay to
prove the final call target is exactly that admitted sponsor entry.

### Alternates

- Acceptable: define one compiler-owned sponsor body as an explicit, separately
  identified runtime artifact with a closed ABI and proof/replay contract, then
  compose it with the ranked image rather than laundering it into the semantic
  object.
- Acceptable for the first measurement only: use an already admitted fixed
  sponsor fixture as differential-test scaffolding, provided no result is
  reported as production installation authority.
- Tempting but wrong: use the countdown entry itself as sponsor.
- Tempting but wrong: append an anonymous helper, magic host callback, script,
  or test-only trampoline and treat successful execution as chain evidence.

## Q8 — Semantic loci for the remaining dangerous-authority classes

### Context

Package review classifies dangerous authority only after an exact checked
semantic identity rejoins compiler/toolchain provenance. The existing catalog
covers every currently declared compiler-owned authority surface:
`FilesystemHost`, machine control, port I/O, interrupt control/publication,
root-memory provision, and process/console authority. Package-controlled names,
paths, aliases, and lookalike declarations deliberately classify nothing.

The package policy also requires closed classes for network access, dynamic
loading, signing, secrets, executable installation, and DMA/IOMMU. Omega does
not currently declare compiler-owned semantic surfaces for those authorities.
Some may ultimately be services; others may be provider, installation,
intrinsic, or representation mechanisms rather than boundary traits.

### Problem statement

Choose the exact compiler-owned semantic locus that establishes each remaining
dangerous class. Without that identity, package review cannot distinguish real
authority from a same-spelled ordinary package declaration, and adding enum
tags alone would create policy vocabulary with no sound producer.

### Proposed direction

Define each authority at the narrowest semantic axis that actually grants or
exercises it: a service declaration for reached runtime authority, an exact
provider or compiler intrinsic for supplied execution, and an installation or
representation mechanism for artifact/runtime authority. Attach the closed
risk class as compiler-owned metadata to that identity and project the pair
into package evidence. Ordinary-package authority classifications remain
consumer policy bound to an exact accepted declaration identity and normalized
schema; package source cannot self-classify.

This need not force every class into a boundary trait. If Omega discovers that
one class has no distinct semantic surface in the language, omit it rather than
inventing a nominal declaration solely for package review.

### Alternates

- Acceptable: introduce one explicit compiler-owned authority catalog keyed by
  stable semantic identities across services, intrinsics, providers,
  installation, and representation mechanisms, provided each producer proves
  the exact identity join.
- Acceptable during staged language growth: keep absent classes unimplemented
  until their real semantic surfaces land, while existing surfaces remain
  exactly classified and unknown authority-bearing mechanisms fail admission.
- Tempting but wrong: classify a declaration because its package-controlled
  name contains `Network`, `Secret`, `Loader`, or another suspicious word.
- Tempting but wrong: create placeholder boundary traits for every policy class
  before the language has a use for those boundaries.
- Tempting but wrong: treat a package-wide role or a reviewer/model verdict as
  the authority identity.

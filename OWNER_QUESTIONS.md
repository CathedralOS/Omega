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

## Q6 — Own the ranked native-fuel sponsor entry

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

## Q7 — Semantic loci for the remaining dangerous-authority classes

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

## Q8 — Own proof-only FloatMeaning equality and source correspondence

### Context

The Float catalog already defines exact binary32/binary64 meaning projection:
finite values map to nonzero rationals, signed zero and infinity remain
distinct, NaN payloads erase only in proof meaning, and cross-format projection
rejects. Checked and Terminal evidence retains exact projection invocations,
operators, operands, formats, equality coordinates, tables, and provenance.
The wider proof/`Real` connection now reaches the proof-kernel boundary rather
than lacking an executable Float model.

### Problem statement

The proof kernel accepts equality only over its existing scalar-term carrier;
it has no proof-only `FloatMeaning` or general `ProofValueId` term. Two
independently authored meaning projections also do not retain one shared
landed-source identity, while Terminal equality rows currently have neither an
exact contract owner nor an evidence-provenance lane that can authorize their
coalescing. Implementing any one of those choices privately would decide which
proof terms exist and when two authored projections denote the same value.

Choose together:

1. the kernel term that carries proof-only `FloatMeaning` values;
2. the accepted equality rule for that sum, including NaN-payload erasure and
   signed-zero distinction;
3. the exact source-coordinate identity and coalescing rule for independently
   authored projection invocations; and
4. the contract/evidence owner that Terminal replay must bind before such an
   equality can discharge an obligation.

### Proposed direction

Add a closed proof-only semantic-term carrier whose Float child is the existing
`FloatMeaning` sum, not a runtime scalar or tagged ABI. Bind every projection
term to the exact checked source value/projection occurrence and its canonical
Float table identity. Kernel equality compares the semantic sum structurally
under the documented payload erasure; coalescing is permitted only when the
retained landed-source identity and projection contract are identical.
Terminal evidence names that contract owner and source correspondence
explicitly, and the verifier independently reconstructs both before invoking
the equality rule.

### Alternates

- Acceptable: introduce a general proof-value term encompassing other
  proof-only sums, provided its equality rules are closed per carrier and do
  not create a runtime representation.
- Acceptable: avoid source-occurrence coalescing by retaining one explicit
  theorem/contract application that relates the two projections, provided its
  owner and complete evidence provenance survive Terminal replay.
- Tempting but wrong: encode `FloatMeaning` as a runtime scalar, compare raw
  float bits, or collapse signed zero merely because NaN payloads erase.
- Tempting but wrong: equate independently authored projections by matching
  operator names, format labels, compact fingerprints, or coincidentally equal
  values without a shared source/contract owner.

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

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Last pruned: 2026-09-05.

## Q1 — Compiler-inserted spill-access fault semantics

Omega must compile ordinary register-pressure programs by relocating live
values through compiler-owned spill storage. Chapter 16 currently requires
operation- or platform-triggered faults to remain inside explicit
`crashes Trap` ceilings, while the optimizer semantic contract forbids
introducing an observable trap or exit change. Neither contract says whether
a fault caused only by realizing an otherwise-valid value in a
compiler-selected stack slot is an Omega program observation.

Choose the semantic boundary for compiler-inserted spill loads/stores:

- the target/runtime must establish sufficient spill storage before entering
  the checked invocation, making admitted spill accesses non-faulting in the
  language model and treating establishment failure as an outer activation or
  deployment failure;
- each possibly faulting spill access is a platform-triggered `Trap` site that
  must enter inferred/published crash ceilings with retained guard and
  provenance; or
- a versioned target realization profile explicitly selects between those
  contracts and becomes part of optimization, frame, and publication custody.

This decision blocks only conversion of the validated abstract spill schedule
into real memory operations and frame/probing code. Logical spill choice,
abstract slot coloring, reload-value allocation, and non-authoritative frame
planning may proceed without making a trap claim.

## Q2 — Mutable structural-parameter native identity

Omega structural parameters currently select a native ABI from their value
shape without incorporating `MutableBorrow` or `WriteOnlyBorrow`. A small
record can therefore arrive directly in registers and be staged into a local
Unit-frame home. A verified field store updates that local carrier and an
immediately following projected call observes it, but a caller or alias cannot
observe the mutation after return. That bounded closed-use route is not a
general realization of borrowed mutable identity.

Choose the native identity contract for mutable structural parameters:

- mutable and write-only structural borrows are always represented by an
  indirect referent placement, while owned values retain shape-selected
  by-value ABI treatment;
- a checked copy-in/copy-out contract is added with explicit alias, partial
  write, crash, and return-path semantics; or
- Omega defines these structural parameters as invocation-local value
  carriers despite their borrow spelling, which would require reconciling the
  language-level meaning of mutation and aliases.

The first option is the proposed direction. Treating a staged copy as though it
were the referent is tempting but wrong; ordinary post-return or concurrent
alias observation would distinguish them. This decision blocks general native
structural-field stores and writeback. It does not block the explicitly bounded
direct named-dynamic fixture whose stored value is consumed by the following
projected call before the Unit returns.

## Q3 — Ranked receiver-subplace transfer identity

The product compiler requires ranked cyclic control to preserve a mutable
receiver subplace across a backedge. The current ranked source and checked plan
synthesize target `self` from source `self` as a whole receiver and retain only
source/target parameter positions. Neither layer has a receiver projection path
or a rule that identifies a projected referent as the next state's receiver.

Choose the authored and semantic identity of that transfer:

- keep `self` whole and carry `&mut self.field` as a separate explicit state
  parameter, with the receiver and subloan both present in the cyclic frontier;
- allow a transition to rebind the target state's `self` directly to a
  projected source subplace, defining the required nominal-type and lifetime
  relationship; or
- keep the target receiver whole but add explicit external root-to-receiver
  provenance, so the ranked carrier records that `self` denotes a subplace of
  an enclosing owner without transferring a second parameter.

These choices produce different checked frontier, alias, cleanup, ABI, and
native replay obligations. This decision blocks only the first projected
receiver-subplace ranked-countdown slice. Whole-receiver ranked countdowns and
ranked work that does not require receiver projection may proceed unchanged.

## Q4 — Portable filesystem control and lifecycle authority classes

D45's six portable filesystem facets determine 36 of the current 50 raw
`FilesystemHost` requirements, including conservative all-facet unions for
flag-polymorphic open operations. Fourteen real operations remain semantically
underdetermined; assigning them from readable method names would violate D45,
and the existing class definitions do not state whether their effects count as
one of the six filesystem authority classes or as explicit empty dispositions.

Choose the portable classification for these cohorts:

- `read_link`: filesystem content read, metadata query, or both;
- `sync` and `sync_data`: content write (and, for `sync`, possibly metadata
  mutation), or an explicit empty class set because they only request
  durability for authority exercised by earlier writes;
- `lock_file`, `lock_file_ex`, and `unlock_file`: metadata mutation, an
  explicit empty set, or a new coordination/locking facet;
- `close`, `find_close`, and `close_handle`: an explicit empty set for resource
  release, or the conservative facets of authority that may be exercised while
  finalizing the underlying object;
- `seek`, `get_osfhandle`, and `duplicate`: an explicit empty set for
  descriptor-local state/alias manipulation, or a filesystem facet that owns
  descriptor position and duplication; and
- `get_last_error` and `errno`: an explicit empty set for thread-local error
  observation, or metadata query because the values are reached through the
  filesystem boundary.

This decision blocks only completion of the portable permission table and
eventual removal of the transitional broad `Filesystem` row. It does not block
the explicit 36-requirement consumer-policy cohort, exact schema/requirement
custody, or engineering work on syscall/import mechanism identities and target
classification. Raw descriptors continue to establish operation classes only,
never object confinement.

## Q5 — macOS GUI publication destination

The Mach-O GUI publication contract in
[`calling_plans.md`](wiki/design_briefs/calling_plans.md) states in the present
tense that the compiler report retains an app-bundle executable receipt
separately from the flat executable receipt. Both consume the same sealed
publication and container, their destination paths stay distinct, the bundle
slot rederives the canonical
`<build>/<sanitized-project>.app/Contents/MacOS/<executable>` path from the
report root and flat output, and swapping two otherwise matching receipts
rejects.

The validator for that contract is implemented and runs on every native
compilation: `omega-compiler/src/compiler/native_checked.rs:23` calls
`has_consistent_executable_publication_custody`, which reaches
`executable_publication_pair_matches` at
`omega-compilation-report/src/lib.rs:116`. Nothing produces the receipt it
validates. `ExecutablePublicationDestination::MacOsAppBundle` is constructed
exactly once outside its own declaration, at `lib.rs:1088`, inside the
`#[cfg(test)]` module opening at `lib.rs:958`; the production producer was
removed with the legacy StateGraph route in `f6b3e65350` (2026-08-28). The pair
validator therefore takes its `bundle: None` early return on every real
compilation and returns `true` without comparing anything, so the outward report
attests publication custody for a destination the compiler never writes.

The destination is not hypothetical. All four GUI samples — `image_viewer`,
`window_app`, `window_demo`, and `windowed_calculator` — bind
`macos_arm64::ProgramEntry`, and a flat Mach-O executable is not launchable as a
macOS GUI application.

Choose the publication destination contract for macOS GUI outputs:

- the compiler publishes the bundle, which restores a producer for the bundle
  receipt slot and makes the retained pair contract load-bearing on every macOS
  GUI build;
- the compiler publishes only the flat executable and bundle assembly belongs to
  a later deployment owner, in which case the receipt slot, the destination
  discriminant that currently enters the installation-evidence digest at
  `lib.rs:104-107`, and the brief's app-bundle paragraphs are removed together;
  or
- the bundle slot remains in the report as a non-authoritative forward
  declaration, documented as unproduced, with the brief amended to describe a
  planned rather than a current capability.

The second and third options both amend a settled design brief, which is why an
implementer cannot close this by choosing one. Until the owner rules, the
retained pair contract and its validator stay as they are.

This decision blocks only the macOS GUI publication destination and the retained
receipt pair. It does not block flat Mach-O emission, the ad-hoc code signature,
the sealed-container and installation-replay custody the flat receipt already
exercises, or any non-Darwin target.

<a id="anonymous-integer-division-and-remainder"></a>

## Q6 — Anonymous integer division and remainder

Systems code uses compile-time arithmetic to size buffers and partition capacity.
For an odd capacity, `let half: u32 = 4097 / 2;` needs one language-defined
result before landing. The compiler must know whether to truncate, retain a
fraction, or reject rather than infer a rule from the destination's width.

Chapter 5 defines fixed-width `/` as truncation toward zero and `%` as the
corresponding dividend-sign remainder. Its separate two-phase constant rule
defines anonymous arithmetic as exact unbounded integer arithmetic, with `Rat`
for decimals, but does not specify a non-integral anonymous quotient or the
sign convention for anonymous remainder.

Should `3 / 2` truncate to the unbounded integer `1`, produce exact rational
`3/2` which cannot land in an integer destination, or reject until an operand
is explicitly landed? For `(0 - 3) % 2`, should the anonymous result be `-1`,
`1`, or require an explicit landing before remainder is defined?

These are existing arithmetic spellings, not a proposed new surface. Current
context-free integer evaluators use truncating `BigInt::div_rem`; that
implementation is not a ruling. The shared anonymous landing evaluator admits
only exactly divisible `/` and does not choose signed remainder semantics.

Only expansion of those ambiguous anonymous operations is owner-blocked.
Fixed-width division/remainder, exactly divisible anonymous division, and
anonymous addition, subtraction, multiplication, and their destination coverage
remain implementable under the existing rules.

# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
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

Bootstrap design exploration is delegated engineering work, not an owner
question merely because it compares a different implementation or language
facility. Apply [whole-chain minimization](bootstrap/MINIMIZATION.md): identify
the next compiler/checker customer and compare complete audit cost. Experiments
remain non-authoritative; changes to the trust boundary or required assurances
must be surfaced before relying on them.

## Open questions

1. **Bare boundary-trait instance fields on a program-entry receiver.** The
   language guide ratifies `console: Console;` as an instance binding
   (chapter 1 Hello World, chapter 19 service bindings), and 136 maintained
   samples spell their Console field that way against 4 using
   `console: Service<Console> in Bound`. The [entry contract](wiki/spec/build/entry_roots.md)
   defines occurrence evidence only for `Service<R> in Bound` receiver
   fields, so the hosted receiver bridge has no establishment row for a bare
   instance field and rejects the entry ("macOS hosted receiver bridge lost
   exact contract, storage, or entry custody"); TASKS.md forbids admitting an
   erased field without a row. Witness (macOS ARM64, ab5f28700d): unchanged
   `samples/cli/basics/number_guess` fails there, and the same program with
   only that field respelled `Service<Console> in Bound` compiles and exits
   70. Decision needed: (a) a bare instance field on the entry receiver is
   establishable, which requires Psi eligibility to record provider-backed
   erased fields as a positive witness that settlement and the bridge check
   (no erased-field fallback); or (b) `Service<R> in Bound` is the only
   establishable receiver spelling, in which case chapter 1's Hello World and
   the 136 samples migrate (precedent: cli_mvp 70f1ebe6cd, generic_counters
   838a868432) and the guide's instance-binding examples stay valid only for
   non-entry data. Motivating customers: SAMPLE-CORPUS `number_guess` and
   the `cli/basics` cohort; ENTRY-CONTENT-ROOTS owns the implementation
   either way.

2. **Do fixed-token operator operands auto-borrow a place into a reference
   parameter?** [Expressions](wiki/spec/language/expressions.md#operators)
   makes operator selection operand-directed over normalized operand shapes,
   gives an attached receiver position zero "with its exact
   ownership/access mode", and otherwise puts the first ordinary parameter
   there; [indexing](wiki/spec/language/expressions.md#indexing-and-ranges)
   says `[]` selects an ordinary operator. Neither says whether a record
   place operand (`self.buffer: Buffer`) matches an ordinary
   `items: &Buffer` parameter, the way a `&self` receiver is taken on a
   place. Today only collection shells adapt (`[T; N]`, `&[T; N]`,
   `&mut [T]` into `&[T]`, per
   `typed-trees/src/typed_trees/declarations/operator/indexing.rs`); a
   `Buffer` place never matches `&Buffer`, and the checking test
   `indexed_operand_access_preserves_shared_collection_and_owned_index`
   (wiki/drafts/known_baseline_failures.md) fails only because f1f9f898e2
   stopped a wildcard re-seed that had hidden the mismatch. Decision
   needed: (a) operand position zero of a fixed-token use takes a shared
   loan of a place when the declared parameter is `&T` and the place is
   `T` (an implicit `&` limited to that position, matching attached
   receivers), or (b) operand shapes are exact and the author spells
   `(&self.buffer)[index]`, in which case the fixture is respelled and no
   typing change is needed. Recommendation: (b), since every other operand
   position and ordinary call already requires the explicit borrow and the
   spec names no implicit loan outside attached receivers.

3. **Board hygiene: items the spec does not ask for, or that duplicate
   another item.** A read-only pass on 2026-09-17 traced every TASKS.md
   item to `wiki/spec`; 64 of 75 rest on a normative clause and twelve are
   named by the spec itself. The remainder need an owner decision because
   removing or merging a board item is a scope choice, not engineering:
   - **CRASH-GUARD-COST**: no spec text asks for checker throughput work;
     the mandated experiment ran and was inconclusive (1.5%), the prototype
     was discarded, and the cache-dedup slice landed. Recommend removal.
   - **CANARY-CORPUS** and **SAMPLE-CORPUS**: two triage umbrellas over the
     same "fix what the suite reports" activity, the first listing the
     second as a dependency; neither defines a corpus the spec names beyond
     `wiki/drafts/rust_compiler_completion.md` RC-REPRESENTATIVE-PROGRAMS.
     Recommend one corpus row with that command as its acceptance.
   - **FFIVAL**: a one-line host-gated canary run that restates
     REGISTERED-CALLBACK-LIFETIME's remaining acceptance and the
     platform-gated verification bullet. Recommend removal.
   - **MACOS-APPLICATION-PUBLICATION**: its own first line says the
     [contract](wiki/spec/build/macos_application.md) is landed; what
     remains is host- and dependency-gated acceptance already listed under
     platform-gated verification. Recommend demoting to that bullet.
   - **BOUNDARY-ISSUANCE** (four-line stub sequenced "after conservation
     closes", same spec section) into **CONSERVATION-CONTRACT**;
     **BOUNDED-INSTALLATION-REACH-ROWS** (only open work is the carrier
     COMPONENT-SUBSTRATE must supply) into **COMPONENT-SUBSTRATE**;
     **FILESYSTEM-RELEASE-CONTRACT** (same
     [permissions](wiki/spec/build/permissions.md) clause and the same
     `FilesystemOrdinaryReleaseContract` evidence row) into
     **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**;
     **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP** split into **CML4**
     (cleanup hook) and **PROOF-RELEVANCE-MIGRATION** (erased ownership).
     Recommend the merges.
   - **SYMBOLIC-MATERIALIZATION**: the mandate
     ([plans](wiki/spec/layouts/plans.md#derived-consumers)) stands, but its
     notes accreted recursive direct-sum and nested sum-array layout work no
     spec text asks for, and the only stated remaining acceptance is a
     host-gated Linux aarch64 rerun. Recommend trimming to the mandate plus
     one platform-gated line.
   - **EXTERNAL-ENTRY-STACK-EPOCHS** and **TR3-TR8** both own stack leases
     and epochs ([entry stacks](wiki/spec/resources/entry_stacks.md) versus
     [storage](wiki/spec/resources/storage.md)) with no stated boundary;
     either could absorb the other's work. Recommend one boundary line or a
     merge.
   - **OMEGA-PRODUCT-COMPILER-SOURCE**: the spec assumes an Omega-written
     compiler ([compiler request](wiki/spec/build/compiler_request.md)) and
     AGENTS.md names `source/psi` and `source/omega`, but no spec clause
     prescribes the two-sibling-package split or the `build.omg`/`main.omg`
     entrypoints. Keep the item; decide whether that split is spec (add the
     clause) or an owner architecture decision recorded here.

4. **Receiving-policy selection surface** (named decision:
   `receiving-policy-selection`). The
   [two-axis containment contract](wiki/spec/build/permissions.md#two-axis-containment)
   and [receiver-owned requirements](wiki/spec/proofs/publication.md#receiver-owned-requirements)
   settle that the receiver "selects a versioned policy package and its exact
   concrete configuration through ordinary package review and pinning",
   independently of the offered program, and that accepted package permission
   rows must not be mirrored into that receiving policy. No contract spells how
   a consuming project names that policy package: the
   [build declarations](wiki/spec/build/declarations.md) project only
   `package`/`application`/`member`/`depend*`/`artifact_only`, the lock and
   acceptance contracts describe only package rows, and the compiler's
   `PreparedLocalProjectNativeRequest::with_receiving_terminal_authority_permission_policy`
   has no caller outside tests, so `operations/compile_project.rs` defaults to
   the empty deny-by-absence policy. Product requirement: the documented
   `cli_mvp` CLI command, Cathedral's native smoke
   (`tools/x86-empty-page-table-canary/native-smoke`) and Squalr's native
   acceptance all pass package review and stop at "receiving terminal-authority
   policy omits the accepted permission for `Console::exit_process`" (TASKS.md
   `cli_mvp` row and Process-exit contract at e2447086af). Which surface should
   select the receiving policy?

   - (a) A projected root-build declaration naming a policy package by ordinary
     source selection, for example `builder.receiving_policy(Source::Path {
     location: "../console-policy" })`, reviewed and pinned like a dependency
     edge but never imported as product or build code; the policy package's
     concrete rows are ordinary Omega data in that package. This fits "ordinary
     package review and pinning" and keeps the axis distinct from accepted
     package rows. Recommended default.
   - (b) A compiler invocation input (`--receiving-policy <file>`) carrying the
     rows directly. Simplest, but the policy then bypasses package review and
     pinning, which the contract requires.
   - (c) Reuse the PCC receiver policy package selected for
     [proof publication](wiki/spec/proofs/publication.md) as the single
     receiver policy (physical permissions as one section of it). Coherent, but
     PCC-PRODUCT-PUBLICATION is unimplemented and this couples ordinary native
     realization to the proof-product route.

   Until answered, TWO-AXIS-TERMINAL-AUTHORITY-REVIEW's receiving-axis input
   and the `cli_mvp` CLI acceptance are design-blocked; the package axis
   (proposing and accepting the permission row) is not.

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.

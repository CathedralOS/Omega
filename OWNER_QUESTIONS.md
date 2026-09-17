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

5. **Is a token-bound proof-machine `requires` a formation obligation at
   the selecting use?** (named decision: `proof-operator-requires-formation`).
   `core/nat.omg` states that subtraction is partial at formation with its
   premise carried by the operator contract, and
   `fail/proofs/nat_exact_subtraction_requires_order` pins that a bare `-`
   on `Nat` without a prior `used <= total` fact rejects ("cannot prove
   `used <= total` -- the `requires` of `Nat::subtract`"). That premise is
   enforced today only by the separate operator-contract prover on the
   `operator` declaration form. The
   [executable-supply contract](wiki/spec/language/expressions.md#executable-supply)
   retires that form: `Nat::subtract` and `Nat::less_or_equal` have no
   compiler catalog identity, so they become declaration-owned bodies
   (`machine - Nat::subtract(left: Nat, right: Nat) -> Nat requires right
   <= left; { saturating_sub(left, right) }`), and a token call then "retains
   the same declaration/body association" as the named call. But
   `typed-trees-to-checked-trees/src/checks/contracts.rs` deliberately
   exempts proof-machine-to-proof-machine calls from the `requires` prover
   ("a call between proof machines denotes a mathematical application whose
   value does not depend on the callee's requires"; keeping the prover on
   such calls refuses sound requires-bearing induction), and the named call
   `Nat::subtract(total, used)` is accepted today without the order fact.
   [Mathematical bindings](wiki/spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use)
   and [citation and induction](wiki/spec/proofs/contracts.md#citation-and-induction)
   settle recursive citation but not whether a proof application's declared
   `requires` is a formation-time obligation at the use site. Product
   requirement: a prototype migration (reverted) compiles both Nat canaries
   including the fail fixture, so the partial-subtraction premise would be
   silently dropped for every core `Nat` consumer. Options:

   - (a) Formation obligation: a proof-to-proof application proves the
     callee's `requires` at the site whenever its facts are decidable
     there, keeping the induction exemption only for the recursive
     component's own contract under its proved ranking edge. Preserves the
     `nat.omg` rule and the fail fixture; implementation in
     `checks/contracts.rs` plus the migrated bodies. Recommended default.
   - (b) Exempt, like every named proof-to-proof call: migrate the pair,
     delete the fail fixture's rule and the harness assertion, and restate
     `nat.omg` so partiality is documented rather than enforced.
   - (c) Keep only these two declarations on a retained bodyless form
     with the operator-contract prover, contradicting the one-supply rule.

   Until answered, the `core/nat.omg` satisfier pairs stay on `operator`
   and OPERATOR-MACHINE-SUPPLY's map marks them design-blocked.

6. **Which explicit binder selects an indexed domain's index operation
   contract?** (named decision: `open-index-operation-selection`). The
   [executable-supply contract](wiki/spec/language/expressions.md#executable-supply)
   retires bodyless root `operator` slots and states that "any required
   conformance/provider selection is explicit, never an implicit search
   among visible satisfying machines"; [licensed normalization](wiki/spec/proofs/contracts.md)
   requires "an explicitly selected conformance with checked operation and
   law slots", and [domains](wiki/spec/language/domains.md) require "the
   exact selected checked algebra". Today the PDI3 open-index operation is
   the bodyless root slot `operator + IndexAlgebra::plus(left: u64, right:
   u64) -> u64;` supplied by an implicit unique-satisfier search
   (`validation/src/value_custody/type_references/open_index_expressions.rs`
   collects every machine whose conformances name `IndexAlgebra::plus` and
   requires exactly one, then `structural_judgment.rs` demands its proved
   AC algebra), and `fail/generics/open_index_unlicensed_algebra` pins
   "requires one exact proved associative/commutative algebra instance,
   but 0 were found". `IndexAlgebra` is a bare path prefix, not a declared
   type or domain, and the operand tuple is bare `u64`, so the operator
   families rule gives the declaration no semantic home and the machine
   form rejects it as a compiler-owned-family injection. Product
   requirement: `pass/generics/{open_computed_quantity_result,
   open_index_local_fact}` and their two fail controls, and every
   `domain<T, const I: u64> T::Indexed<I>` computed index. Options:

   - (a) A token-bearing trait requirement (`trait IndexAlgebra { machine +
     plus(left: Self, right: Self) -> Self; }` with `u64 satisfies
     IndexAlgebra` realized by the provider) selected by an explicit
     conformance named on the indexed domain declaration, for example
     `domain<T, const I: u64> T::Indexed<I> using IndexAlgebra;`, with the
     AC law slots on the same trait. Fits the trait row of the supply table
     and keeps selection at the declaration. Recommended default.
   - (b) The same trait selected at each use site through an explicit
     proof-static binder on the computed index expression. Finer-grained
     but repeats the selection at every index.
   - (c) Keep these slots on a retained bodyless root form with the
     implicit search, contradicting the explicit-selection rule.

   Until answered, the four `IndexAlgebra::plus` fixtures stay on
   `operator` and OPERATOR-MACHINE-SUPPLY's map marks them design-blocked;
   this is distinct from question 5, which concerns formation-time
   `requires` rather than the selection binder.

7. **Which boot protocol issues the AP startup vector, and who owns it?**
   (named decision: `ap-startup-protocol-ownership`). The [executable
   installation contract](wiki/spec/build/executable_installation.md) says AP
   startup "installs a compiler-produced low-memory trampoline and invokes a
   target boot protocol" but names no protocol; no spec, board, or source text
   mentions INIT/SIPI, a startup IPI, the local APIC ICR, or
   `EFI_MP_SERVICES_PROTOCOL`, and the xAPIC/x2APIC register facts are recorded
   as Cathedral's `local_apic` package (15221af38f), which the firewall keeps
   package-owned. `external-roots` already owns the trampoline placement
   ledger, the start edge and the quiescence edge (344063c651), whose 4 KiB
   vector geometry and real-mode arrival regime are INIT/SIPI-shaped but only
   by inference. Decision needed for x86-64: (a) the vector is issued by an
   INIT/SIPI sequence through the local APIC ICR, which makes those register
   facts a compiler-owned `target`/`program-entry-plan` leg like the UEFI Boot
   Services rows; (b) it is issued through firmware
   `EFI_MP_SERVICES_PROTOCOL.StartupThisAP` while Boot Services are live, which
   needs a located-protocol row the entry plan does not carry; or (c) a
   Cathedral-owned provider consumes the ledger's
   `SecondaryProcessorStartupInvocation` carrier and mints
   `SecondaryProcessorStartupReceipt`, the compiler owns no protocol edge, and
   the remaining compiler work is sealing that receipt's issuance (today
   `SecondaryProcessorStartupReceipt::from_provider` is public). Motivating
   customer: Cathedral's multiprocessor boot, whose secondary processors cannot
   start until one of these edges exists; AP-BRINGUP owns the implementation
   either way.

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.

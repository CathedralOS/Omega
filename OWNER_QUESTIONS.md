# Owner Questions

Only the owner answers, closes, removes or rewrites an entry here, including
one that an agent believes the specification already answers. Agents add
questions and may research existing ones, but they report their findings and
citations to the user and leave the entry, the specification and the boards
unchanged; they never edit the specification or a task to settle a question.

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Psi representation and encoding design is delegated; human review is deferred
under [the current policy](omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit).
Missing IR operations or a schema change needed for accepted behavior are
implementation work, not owner questions.

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

Application ports, including Squalr, are implementation customers, not a separate
class of owner questions. Port shape, API adaptation, missing library operations,
and compiler rejections stay with the implementer under the existing contracts.
Escalate only a specific unresolved language, architecture, or trust rule, citing
the contracts that leave it open. The application may motivate that question;
its name or a workaround does not establish one. Do not ask the owner to approve
ordinary port choices or treat a workaround as permission to reduce acceptance.

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

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

### 1. How should users select interpreted execution and a possible future JIT?

AOT remains the established native execution path. How should users explicitly
trigger standalone Terminal Psi interpretation? If we add a JIT later, should
it be another explicitly selected execution mode, or should an opted-in runtime
automatically compile hot code? Keep `--both` as a separate native/interpreter
comparison choice rather than conflating comparison with engine selection.

The concrete customer is running an ordinary Omega program through the chosen
engine, including running it without first generating a native executable.
The current CLI exposes native `omega run` and comparison through `--both`, but
no interpreter-only option. This asks for the user-facing selection policy, not
an implementation change or a decision that JIT must be built.

Contracts checked: [embedding's engine/target separation](wiki/spec/build/embedding.md#target-environments-and-execution-engines)
already treats interpreted and native execution independently from OS targets;
[separate Terminal consumption](wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)
allows compilation to stop at Terminal Psi for later interpretation or native
lowering. The embedding execution contract says no JIT is required, and its
[implementation ownership](wiki/spec/build/embedding.md#authority-evidence-and-ownership-of-implementation)
treats the CLI as a library client. These establish independent interpretation
but do not settle a standalone CLI selection surface or a future JIT trigger
policy. Any future automatic JIT policy would be an explicit new design choice,
not an implicit requirement of the existing interpreter contract.

### 2. Must a guard on a copy of an array element be rejected after the element is overwritten and restored?

`tests/omega/fail/contracts/scalar_copy_guard_after_source_restore` is a reject
twin that main now accepts. Its `inspect` machine reads `main.items[selector]`
twice (`copied`, then `saved`), writes `0` to the element, writes `saved` back,
and only then tests `copied >= 48 && copied <= 49` before transitioning to
`bounded`, which `requires main.items[selector] >= 48` and `<= 49`. The twin
expects `cannot prove requires contract`. On base `60f7e3cd4ee` a focused corpus
run of the fixture reports `checked`, not `rejected`. The investigation traced
this to #985 (`0020604823c`, 2026-10-31): the checker proves both requirements
because `copied`, `saved` and `main.items[selector]` all denote the stored
contents at state entry, and the restore writes back exactly that value. The program meets `bounded`'s
requirements on every execution. Must it still be rejected? Is the twin's oracle
a language rule, or a limit of the earlier analysis that the stronger one
legitimately lifted?

Option A, the twin is stale: accepting is correct. The fixture moves out of
`fail/contracts/` (to `pass/` or deleted) and its golden rows are re-recorded.
The language then admits a guard on a scalar copy across a write that restores
the source, when the written value is provably the copied one. Sibling controls
that change the stored value (`scalar_copy_guard_after_source_write`,
`scalar_copy_guard_after_selector_write`) keep rejecting; four soundness probes
still reject too (aliasing write through another index, restore of a different
value, re-read after a write, store of a late read into `main.saved`).

Option B, the twin states a rule: the program must be rejected. The analysis
must stop relating `copied` to the element once any write to the element
intervenes, even a value-equal restore. That requires deliberately weakening
the stored-value tracking in the scalar copy guard route
(`current_values::proves_call`, `guard_observations.rs`) for restored values,
rejecting a program that provably satisfies its contracts, and a written rule
to cite.

Evidence, from the investigation on base `661760196df`: the call-site proof is
`captured_boolean::judgment` -> `captured_contract_proven` ->
`current_values::proves_call`, where the three values share one atom,
`completed-scalar:IndexedValue{main.items, selector}`. #985 switched
`numeric_observations` from `declared_integer_field_read` (declared fields only)
to `integer_storage_read` (any fixed-integer storage read, array elements
included). The golden row (`rejected`, `@ccdb99ca2214`) was recorded by #889
(`c64728d90e8`) before #985 merged. The twin's shape comes from `10aeeb04a5b`,
whose `guard_observations.rs` header says "a write followed by restoration
still breaks the relation". That sentence is implementation commentary on
the guard copy-relation route, not a language rule.

Contracts checked:
[primitive reads](wiki/spec/terminal-psi/fact_validity.md) (line 28) allow
an equality between a copied result and an exact reaching stored value and
say later writes cannot change either captured value, but name "missing
stores" as preventing introduction; `copied` and `saved` have no reaching
store, only state-entry contents. [Entry hypotheses](wiki/spec/terminal-psi/fact_validity.md)
(line 67) bind "structural entry paths" and say current body values are not
entry snapshots; whether a body read of an unchanged entry path may be
identified with that path is not stated. [Mutation and subject
identity](wiki/spec/language/state_contracts.md#mutation-and-subject-identity)
(lines 175-198) says copies "transport facts about the captured value" and
that overlapping destination facts are invalidated at a write, then the new
value is established; it does not say a value-equal restore must lose facts
about the copy. [Lifetimes](wiki/spec/language/lifetimes.md) (line 101) says
integer snapshots are not live storage. No passage names scalar copy guards or
requires a restore to break a relation. These establish that facts follow
captured values and storage footprints but leave the restore case open.

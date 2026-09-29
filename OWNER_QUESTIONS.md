# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

## Q: how do toolchain-settled providers express marshalled syscalls?

`ProviderBinding::Syscall { number }` is the only realization channel a
toolchain-settled provider row can carry, and it admits only integer-class
parameters — every bundled host service that carries a path or slice
(`Filesystem::open`, `open_at`, `read_at`, `canonicalize`, `read_dir`,
`TimeHost` clock names) resolves to zero provider rows at terminal-authority
review, so no program reaching those hosts can produce a native product.
This is the current wall on the headless Squalr run (`tools/native_probe_resume.md`
in the Squalr repo) and on every FilesystemHost/TimeHost customer.

The syscall route cannot grow the vocabulary inside itself: `evaluate_syscall`
admits only ≤8-byte integer-class args into the six syscall registers, has no
slice→pointer extraction, no NUL-terminated path scratch, no injected
constants (e.g. AT_FDCWD), and no −errno decode; toolchain-settled provenance
requires `row_realizations` all-invalid, so a settled row can never name a
realization machine; `canonicalize` has no Linux syscall at all (realpath is
a libc composite); and a checked adapter authored in the library would
circularly need the requirement it adapts.

Two honest settlement forms were identified; the choice is owner-level
because it changes the provider contract surface:

(a) a new `ProviderBinding` variant carrying a marshalling descriptor —
    per-argument `scalar | slice-ptr | path-NUL | injected-const`, result
    errno-decode — consumed by an extended `evaluate_syscall` that
    synthesizes scratch space and the call prologue. Backend-contained, but
    adds codec, identity and review surface.

(b) toolchain-synthesized checked adapter machines admitted as
    `CheckedAdapter` settlement rows — requires a synthesized-body pipeline
    that does not exist today, relaxed adapter admission (the exact-adapter
    gate currently requires `target.is_none()` plus matching package
    identity, while toolchain plans mint `None`), and settlement/dispatch/
    replay learning the new row kind.

Either route spans provider-planning, calling-conventions, admission
settlements, review-evidence replay, and adapter-rows dispatch — a
multi-crate contract change, not a leaf-table extension.

Evidence: bisection on Omega main `df0288f59fd` + bundle `71254d80532`
(call_plan_evaluation.rs:1045 integer-class admission;
provider_settlement.rs:265 all-invalid assertion;
settlements/source_imports.rs Import/Syscall-only admission;
exact_checked_adapter identity requirements).

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

Proposed solutions below are recommendations, not owner rulings. Remove an
entry when the contract answers it; retain any unfinished implementation on
its owning task.

### Q1 - How does an author bound every element of a fixed array?

Named decision: `per-element-array-bounds`.

**Context:** `samples/cli/simulation/game_of_life/main.omg` declares
`board: [i32 [0..=1]; 16]` and `next: [i32 [0..=1]; 16]` — a range-constrained
element carrier. `ShapeCollector::add_type` declines that element on purpose:
`is_unrestricted_nonatomic_primitive` (types/mod.rs:1661) admits only an
arithmetic-policy constraint, with the comment that "ranges and other
qualifications still need their own retained obligations". A 15-line probe
confirms `[i32 [0..=1]; 16]` declines at `signature` while `[i32; 16]` checks.

[Domains and qualification](wiki/spec/language/domains.md) retired the
bracketed scalar-interval suffix in favor of ordinary propositions, and routes
common-field relationships to the data declaration's `where`. `where` clauses
name fields and parameters, not elements — there is no element quantifier. An
interval-only element domain (`i32::AliveCell requires self in 0..=1`) is the
shape the owner already rejected under `scalar-range-contract-spelling`
(6deef33a5c). `dependent_values.md` default-domain obligations likewise speak
to fields and cases.

**Problem:** No current contract spells a bound that holds for every element
of a fixed array. Dropping the bound to `[i32; 16]` weakens the authored
contract the program was written under; keeping it keeps the program declined
before checking. The blocker sits in `terminal_unit/types/`, whose verification
surface is claimed by LOCAL-DATA-CLUSTER-TERMINAL-VERIFY.

**Proposed solution:** Give fixed arrays a constrained-element carrier that
lowers to one retained per-element obligation — either a named element domain
(`[i32 in AliveCell; 16]`, which needs the interval-domain rejection revisited
for this exact role) or elementwise quantification in `where`
(`where forall i, board[i] in 0..=1`). Either needs a Terminal shape for the
element obligation so checking and Omega both replay it.

**Alternatives:**
- Keep the contract silent and require authors to re-establish the bound at
  each element read/write. This declines the authored declaration without
  replacing it and pushes per-element reasoning to every use site.
- Admit the bracketed element suffix under the retired-scalar-suffix rules.
  The spec deliberately routes scalar intervals to propositions; reviving the
  suffix only inside array brackets creates a second spelling for the same
  relationship.
- Model the bound as a proposition over the whole array value in `where`
  (a predicate on the array, not per element). It records the intent but gives
  element reads no fact, so checking still cannot discharge `next[i] in 0..=1`
  without elementwise decomposition.

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

### Q2 - Which declaration surface owns the ELF dynamic-linker path?

Named decision: `elf-interpreter-declaration-surface`.

**Context:** An import-bearing ELF object must emit through
`emit_elf_dynamic_executable` under a `NormalizedElfInterpreterPlan` — the
sealed absolute path of the target's dynamic linker (e.g.
`/lib64/ld-linux-x86-64.so.2`). Every production request site instead
hardcodes `ExecutableImageEmissionRequest::direct(...)`
(`driver/src/native/native_product.rs:38` and
`packages/package-manager/src/admission/terminal_authority.rs:273`), so any
program whose object carries normalized imports rejects at emission with
"import-bearing ELF image emission requires an exact normalized interpreter
input" (`image-emission/src/image_output.rs:182`). The measured case is
`tests/omega/pass/providers/external_boundary_requirement_via_exit`, whose
unique covering satisfiers are external leaves: the object legitimately
carries imports, and no produced interpreter input reaches the writer.

The emission machinery itself is complete: `image-elf/src/dynamic_executable.rs`
plans and validates the whole dynamic-link structure, and
`backend/target/src/elf_loader` already owns interpreter-input validation
(`normalize_elf_interpreter_plan` seals a target/deployment-selected
absolute path, rejecting relative paths and NUL bytes for
LinuxArm64/LinuxX64). What is missing is only the declaration surface that
names the path: no `build.omg` vocabulary, no CLI flag, and no
target-profile default produces an interpreter input today.

**Problem:** The ELF dynamic-linker path is a deployment fact, not a source
fact — the same program binds `ld-linux` on a glibc root and `ld-musl` on an
alpine root. It cannot default from the target profile alone (a profile
does not know the deployment's libc), and hard-coding it into the compiler
would mint a runtime contract no declaration reviewed. Someone must
therefore decide which surface names it — and the answer shapes how much
new vocabulary, evidence, and review surface each alternative costs.

**Proposed solution:** Put the interpreter path in the build declaration as
a target-gated deployment input — e.g. a `builder.deployment` (or
`builder.roots.*`) row spelling the loader path per target — so the emitted
contract travels through the same package review and acceptance flow as
every other emitted fact. `ExecutableImageEmissionRequest::dynamic_elf`
then takes the normalized plan from the minted package evidence at both
request sites; when the object carries normalized imports and no
interpreter input is declared, the diagnostic names the missing
declaration rather than the missing writer arm. Follow-on wiring this
unblocks: (a) the declaration surface supplies
`normalize_elf_interpreter_plan(path, target)`; (b) request sites select
`DynamicElf { interpreter }` when the object has normalized imports
(`RequestedNativeArtifact` needs a dynamic-artifact arm at the consumers,
currently Direct-only at `terminal_authority.rs:291`); (c) emission,
replay, and review replay the declared input — no new emission machinery
needed.

**Alternatives:**
- A CLI flag / environment input on `omega run`/`update`. Cheapest wiring,
  but the path then lives outside package evidence: acceptance, replay and
  cross-invocation review cannot see it, and a caller can bind a loader the
  package never declared.
- A target-profile default (`TargetProfile::LinuxX64 ->
  /lib64/ld-linux-x86-64.so.2`). No new vocabulary, but bakes a libc choice
  into the profile — musl/alpine deployments silently get the wrong loader
  with no declaration surface to override.
- Auto-probe the host's loader. Host-derived, so it fails every cross
  compile (the selected target's deployment is not the host's), and adds an
  ambient-host trust input the proof-carrying contract deliberately
  excludes.

**Caveat:** `external_boundary_requirement_via_exit`'s `expected_native_exit.txt`
(21) is only meetable on `windows_x86_64`: its satisfiers all route `via`
the `PeByName msvcrt.dll` producer, which normalizes to an ELF `DT_NEEDED`
row on linux — structurally valid emission that can never load. Unblocking
the declaration surface gets the fixture to honest emission, not to a linux
run; a linux-native external-binding vocabulary is a separate question.

Evidence: attribution bisection on Omega main `d18d1a24dd2`
(`image_output.rs:167-185` request-shape gate;
`dynamic_executable.rs:208` normalized-plan requirement;
`elf_loader/mod.rs:78` the sealed input that no caller constructs;
`corpus_native_linux_x86_64.txt:727` the pinned rejection row).

### Q3 - How do toolchain-settled provider rows bind per-target constants?

Named decision: `toolchain-settled-constant-binding`.

**Context:** `source/library/std/time_host.omg` declares the canonical raw
clock boundary as toolchain-settled (D11/D12): no package may author
`satisfies` conformances or `select_provider` rows for the slot — provider
settlement mints the realization plan itself, per selected target, from the
toolchain's reviewed realization table
(`realization/provider-planning/src/provider_settlement/canonical_time_host.rs`).
The reviewed linux_x86_64 table covers the three requirements whose
irreducible realization is one named syscall (`monotonic_ticks` and
`wall_clock_raw` bind `clock_gettime` 228; `sleep` binds `nanosleep` 35) as
`ProviderBinding::Syscall` rows, and deliberately leaves the three per-target
constants (`monotonic_ticks_per_second` = 10^9, `wall_clock_units_per_second`
= 10^9, `wall_clock_epoch_offset_seconds` = 0) uncovered — the file's own
contract reads "constants carry no kernel mechanism and stay uncovered until
a constant binding kind exists."

Measured on `tests/omega/pass/time/runtime_time_elapsed_since_exit` (tip
`ed93adc7ef0`, package lock settled via `benchmark.py prepare`): the mint
joins the selected facts correctly — `monotonic_ticks` and `sleep` each
resolve to exactly one selected row — and terminal-authority closure review
stops at
`named-callable(path(TimeHost::monotonic_ticks_per_second),parameters(),result-dispatch())`
resolving to 0 selected provider rows
(`terminal_authority_review/context.rs:163-167`, per-requirement row
resolution). The minted plan is not the wall; the absent row is.

**Problem:** `ProviderBinding` carries seven forms — `Import`, `Syscall`,
`CompilerIntrinsic`, `VtableSlot`, `VtableField`, `TableFunction`,
`CheckedAdapter` (`provider_plan.rs:52-84`) — and none expresses "the
requirement's answer is a reviewed compile-time constant." The import,
syscall, and vtable forms all lower to a call. `CompilerIntrinsic` names a
target realization-machine identity that does not exist for a value.
`CheckedAdapter` names an authored checked machine, which a toolchain-settled
slot cannot mint: conformance provenance requires authored rows it does not
have, and toolchain plans hold `row_realizations` all-invalid. Every consumer
— plan codec, digest encoder, review-evidence encode/recovery, provenance
replay, terminal-authority mechanism joins, trust manifest — matches
exhaustively on the seven forms, so an eighth is a contract-surface change of
the same class the marshalled-syscall question above already rules
owner-level.

**Proposed solution:** `ProviderBinding::Constant { value }` — a binding kind
whose realization emits the constant as the call result with no call emitted
(the constant-result shape the darwin time-host canary already asserts:
frequency and units served with no call). The per-target reviewed tables gain
the constant rows for targets whose values are reviewed (linux_x86_64: 10^9,
10^9, 0); targets without reviewed constants keep the honest zero-row
rejection. Row machinery is unchanged — requirement identity, lifetime
partition, schema validation, uniqueness, digest — so the fork lands inside
existing plan invariants rather than forking plan shape. Downstream:
boundary-call lowering writes the literal into the result slot with no
boundary op, `exact_mechanism` gains a constant-role join, and the codec,
digest encoder, review-evidence encode/recovery, and trust manifest each
gain the variant arm. `u64` covers every reviewed constant today; a wider
value type only matters if a negative or >u64 constant enters a table.

**Alternatives:**
- A plan-level constant side table (`ProviderPlan` gains a constants map
  beside `rows`). Keeps `rows` for leaf mechanisms, but forks plan schema
  plus every row-iteration consumer anyway, and moves the constant rows out
  of the per-requirement accounting closure review replays — the review
  would need a second counting path for the same honest answer.
- `CheckedAdapter` rows over toolchain-synthesized constant-return machines.
  One mechanism covers constants AND composite marshalling, but requires the
  synthesized-body pipeline that does not exist (the marshalled-syscall
  question's option (b)) — heavier than the constant gap alone justifies.
- `CompilerIntrinsic` with a reserved constant-materialization identity.
  Reuses an existing tag but misrepresents the row: the field names a
  realization-machine overload identity, and a constant has none; every
  identity consumer would need a sentinel carve-out.
- Constant-fold the requirement at the call seam with no plan row. Leaves
  the requirement at zero selected rows, so closure review must learn a
  foldable-requirement class — splitting demand-completeness across two
  mechanisms where one row kind answers it.

**Caveat:** This question unblocks only the constants. The three
syscall-bound siblings may still hit the shared marshalling wall downstream:
`clock_gettime` takes a `timespec` out-parameter, and while
`uses_linux_timespec_result` exists in calling-conventions, the file notes
timespec marshalling and result composition are checked-code adaptation — "a
demanded leaf with no transport still rejects at lowering." Whether the
syscall rows realize end-to-end is the marshalled-syscall question's
territory, not this one's.

Evidence: bisection on worktree tip `ed93adc7ef0` (published)
(`canonical_time_host.rs:20-23,67-96` the named uncovered-constant gap plus
per-target table; `provider_plan.rs:52-84` the seven binding forms;
`terminal_authority_review/context.rs:127-168` per-requirement selected-row
resolution; reproduced diagnostic `native artifact terminal-authority
closure review failed: reachable requirement
named-callable(path(TimeHost::monotonic_ticks_per_second),...) resolves to 0
selected provider rows` on `runtime_time_elapsed_since_exit` after
`benchmark.py prepare` settled the lock).

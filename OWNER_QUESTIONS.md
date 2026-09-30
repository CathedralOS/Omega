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

Authored target leaves hit the same wall. std `Clock::sleep`
(`source/library/std/clock.omg`) binds `nanosleep` on both linux targets as
`ForeignBinding::Syscall` with the requirement's `(milliseconds: u32)`
signature. `evaluate_syscall` places that integer in the request-pointer
register, so the linux legs fail instead of sleeping; the header's former
claim of compiler-owned timespec marshalling had no caller behind it.
macos_arm64 needs `poll(NULL, 0, ms)`. A `MachODylibSymbol` `_poll` leaf with
the requirement's signature compiles and runs, but calls `poll(fds=ms, ...)`
and returns at once: a `sleep(2000)` probe exited after 0.11 s. A checked
adapter over the existing `darwin_poll` leaf is refused: "does not refine
`Clock::sleep`: its body reaches boundary service(s) [DarwinClock] outside
the requirement's declared service ceiling [Clock]". [Foreign
bindings](wiki/spec/build/foreign_bindings.md) keeps "foreign constants and
offsets" in checked target code, and the [language
guide](wiki/language_guide/chapter_19_capabilities_effects_boundaries.md) has
the leaf state "the counterparty's actual shape" while "a checked adapter
converts it". The service ceiling refuses exactly that adapter.
`samples/cli/rendering/{bouncing_ball_2d,mandelbrot_zoom,bouncing_particles}`
and `samples/gui/image_viewer` declare a package-local `Clock` that nothing
provides; migrating them to std `clock` needs a leg that sleeps on the host.

For authored leaves, two more settlements exist besides (a) and (b). One is a
provider-private foreign leaf: a bodyless, target-scoped leaf on the
provider's own nominal type in the same package, with no `satisfies`, whose
reach counts as the satisfied requirement's service. That is a new language
rule. The other is a compiler-owned hosted intrinsic for std `Clock::sleep`,
built like Console's `write_byte` and `exit_process`. It needs no new
surface but is large: a new intrinsic identity through review evidence,
stages 00 to 07, both ISAs and image emission.

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


### Q1 - Which declaration surface owns the ELF dynamic-linker path?

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

### Q2 - How do toolchain-settled provider rows bind per-target constants?

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

### Q3 - How does a retained quotient correspondence become executable?

Named decision: `executable-quotient-correspondence`.

**Context:** `Quotient::define` and `Quotient::lift` requests are admitted
proof-only: the checked-termination-gated semantic extractor installs them
on the terminal module as `quotient_correspondences` rows
(`pipeline/05_checked-trees-to-lowered-psi/src/proofs/quotient_correspondence.rs:70-95`),
whose own contract states the table "does not emit or authorize an
executable quotient operation." The two `pass/proofs` compile canaries
(`quotient_define_managed_compile`, `quotient_lift_managed_compile`, added
09-20 to pin the managed admission under package provenance) are recorded
`rejected` on the production leg of every corpus golden:
`Lowering(InvalidTerminalModule(NonExecutableQuotientCorrespondence))` —
reproduced on pin `d52845416` via
`omega inspect-terminal --machine main` on a fixture whose `main` body is
empty: the request scan
(`program_carries_quotient_request`, same file :97-105) finds the
`admitted` machine's `Quotient::define` call anywhere in the program,
installs the batch, and the module gate refuses it.

**Problem:** The gate is authored fail-closed
(`terminal-psi/verification/src/validation/quotient_correspondence.rs:34-36`):
every `ValidationPolicy` except `Representation` refuses a nonempty
correspondence table, and `lower_machine` applies an executable-capable
policy unconditionally
(`pipeline/05_checked-trees-to-lowered-psi/src/machine_lowering.rs:220-233`).
The spec's own words name the wall: "Execution rejects nonempty tables
until executable quotient lowering exists" (`wiki/spec/proofs/quotients.md`,
published quotient correspondence). The table is meanwhile legal module
content — representation replay accepts it and the codec encodes it
(`terminal-psi/codec/src/sections/semantic_module.rs:72`) — so the refuse
is exactly the designed distance between proof-only retention and an
executable operation, not a defect. What does not exist anywhere: a
production route that emits a callee for the correspondence.

**Proposed solution:** Lower retained correspondences into their executable
meaning during machine lowering. The quotient class's runtime carrier is
the representative value, so `define<F,Congruence>` lowers to an
application of `F` to the carrier with the result reclassified — the
correspondence certificate already proves `F`'s application respects the
relation, which is exactly the executable content. `lift<F,Congruence>` is
the same emission composed through the declared adaptation; three-argument
`lift<F,Congruence,Transport>` composes `F` with the transport theorem's
executable witness. The vocabulary fork the owner names: whether the
emitted callee is a direct fold at the `Quotient::*` call site (the
correspondence's certificate supplies the shape — no new operation kind) or
a synthesized terminal machine per retained row (explicit identity,
visible in `inspect-terminal`, but one more machine-emission route).

**Alternatives:**
- A package/publication validation policy that admits proof-only tables
  for roots that never execute. Makes the canaries green without making
  any real quotient program run — the wall moves to the consuming
  application's own execution gate — and `lower_machine` has no
  product-kind plumbing to scope it by today.
- A non-executable module product marker (artifact carries the table,
  marked non-realizable). The honest intermediate shape, but still a
  contract fork on artifact kind, and every downstream consumer still
  hits the same gate.
- Re-tier the canaries to `fail/` pinning the
  `NonExecutableQuotientCorrespondence` fragment until executable
  lowering lands. Honest bookkeeping, but the fixtures' authored intent
  is "compile canary" — the admission they pin already works; the
  module-gate refuse is downstream of their purpose and would make the
  pin wrong once lowering exists.

**Caveat:** Executable `lift` is deeper than `define`: the three-argument
form's ordered-implication transport theorem needs an executable witness,
not only a retained row. Separately, these canaries also hit a different
wall in package-manager territory — `benchmark.py prepare` reports
"unresolved contract obligations; project decisions cannot discharge
proofs," so package-review settlement cannot discharge the Equivalence
satisfies batch today; that is the review lane's gap, not this one's.

Evidence: pin `d52845416` reproductions — `inspect-terminal --machine main`
on both canaries returns
`InvalidTerminalModule(NonExecutableQuotientCorrespondence)`; the
production bridge header
(`05_checked-trees-to-lowered-psi/src/proofs/quotient_correspondence.rs:1-18`),
the policy gate
(`terminal-psi/verification/src/validation/quotient_correspondence.rs:34-36`),
the unconditional execution gate
(`05_checked-trees-to-lowered-psi/src/machine_lowering.rs:211,220-233`),
corpus rows `pass/proofs/quotient_{define,lift}_managed_compile rejected`
in `tests/omega/corpus_outcomes.txt` and all three platform goldens.

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




No numbered questions remain.

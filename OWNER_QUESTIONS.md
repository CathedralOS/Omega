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

### Q1 - What carries a portable service's per-family raw binding?

Named decision: `portable-service-per-family-binding-carrier`.

**Customer:** one portable filesystem API over per-OS raw services, which
[platform services](wiki/spec/build/platform_services.md) makes the shape for
clocks, consoles and networking too. **FILESYSTEM-PLATFORM-SERVICES** rules
that `FilesystemHost` splits into `PosixFilesystemHost` and
`WindowsFilesystemHost`, each fully covered by the provider bound per target.
The raw-seam half of that needs no decision: a program holding
`Binding<FilesystemHost>` directly can hold `Binding<PosixFilesystemHost>`
instead, and eleven red `tests/omega/pass/filesystem` fixtures are exactly
that shape. The portable half has no spelling, and it is where the item's own
customer lives — `samples/cli/systems/file_journal` and four wrapper
fixtures declare the portable `Filesystem`, not a raw binding.

**Existing rules:** `source/library/std/filesystem.omg:377` declares one
portable `pub data Filesystem { host: Binding<FilesystemHost>; … }`. After the
split that field's requirement differs per target, and none of the three ways
to say so exists.
[Machines](wiki/spec/language/machines.md#target-scoped-families) scopes
families to machines only: "A machine declaration prefixed with a target name
is one body of a family keyed by target under one path", so `data` cannot be
target-scoped. Carrying one field per family does not work either, because
`collect_service_fields`
(`omega-rust/omega/compilation/realization/selected-dispatch/src/service_custody/root.rs:408`)
walks every `Binding<R>` field of the entry receiver recursively and demands a
selected Fused provider for each one regardless of reach (the "requires a
selected Fused provider for boundary" rejection at `root.rs:160`) — a
`Filesystem` holding both families would demand a POSIX provider on Windows.
A generic `Filesystem<H>` with `host: Binding<H>` types correctly, since
`Binding<R>` is already generic (`source/library/core/binding.omg`), but there
is no target-scoped type alias to keep the family out of the portable API, and
the spec requires that: "Raw handle types are per family (`PosixDescriptor`,
`WindowsHandle`) and never appear in the portable API".

**Decision needed:** three candidate answers, in the order I would pick them.

1. **Keep one granted portable boundary and make each raw family a boundary
   trait selected beneath its portable implementation.** `Filesystem.host`
   stays `Binding<FilesystemHost>`, where `FilesystemHost` holds only the
   portable operations. A target's selected portable implementation reaches
   its exact `PosixFilesystemHost` or `WindowsFilesystemHost` boundary trait;
   one named implementation supplies that family. Provider selection already
   admits exact boundary traits, so a target selects only the family it has —
   no sentinel rows, no per-family carrier and no new language surface. It
   costs the ruling's exact words only in where the family is carried: the
   portable value holds the portable binding while its implementation depends
   on the selected raw-family binding. A raw-seam program may still hold
   `Binding<PosixFilesystemHost>` directly.
2. **Demand only reached `Binding` fields**, so `Filesystem` may carry one
   field per family and each target demands the one its selected bodies reach.
   This weakens a custody rule that currently holds unconditionally and makes
   the demand reach-dependent — a larger change to admission than to the
   library.
3. **Add target-scoped `data`, or a target-scoped type alias for a generic
   `Filesystem<H>`.** The literal reading of the ruling and the cleanest
   library; the only one needing new language surface.

All three change what `FilesystemHost` means relative to the spec's three
layers, which is why none of them is the implementer's call: (1) makes
`FilesystemHost` the portable raw seam rather than a per-OS one, (2) changes a
custody rule, (3) adds language surface.

**Implementation customer:** `tests/omega/pass/filesystem/{dir_walk_wrappers_exit,windows_wrapper_results_exit,wrapper_lock_metadata_exit,wrapper_open_with_exit}`
and `samples/cli/systems/file_journal`, all currently rejecting with
"selected ProgramEntry Binding field `Main::fs::host` requires a selected
Fused provider for boundary `FilesystemHost`" or its reachable-requirement
twin. The verified member classification, call-site confinement and the ten
Rust consumers the split moves stay on **FILESYSTEM-PLATFORM-SERVICES** in
`TASKS.md`; that work is ordinary implementation once this is answered.

### Q2 - May hosted interpreter adapters use independent Omega ABI libraries?

Named decision: `hosted-interpreter-abi-library-dependency`.

**Customer:** execute a produced Terminal artifact through the shipped
interpreter API without compiling source again. Native/reference comparison
must bind the exact selected foreign implementations, including descriptor
input/output and clocks, without duplicating target locator or ABI rules.

**Existing rules:** [the facade contract](omega-rust/pipeline.md#two-drivers-one-pass-per-target-realization)
assigns Terminal loading, entry establishment, provider/import installation,
host bindings and observations to the interpreter facade. **TERMINAL-RUNTIME-DRIVERS**
places that facade and its host adapters under `psi/interpreter/`.
[Subsystem ownership](omega-rust/pipeline.md#subsystem-entrances-and-supporting-packages)
assigns target/ABI descriptions and retained realization evidence to Omega
backend libraries and also says "Psi must not depend on Omega."

The current `omega/compilation/driver/src/runtime/host_imports/{descriptor_io,clock}.rs`
adapters compare complete normalized foreign locators and independently validate
the evaluated boundary ABI before effects. Moving those adapters into Psi
unchanged would violate the dependency rule. Removing their checks or copying
the target/ABI catalog would violate their existing contract. The provision
model also needs a source-free runtime projection instead of retaining the
source-checking-dependent `program-entry-plan` package; that projection and
producer-owned exact plan/schema join are ordinary implementation, not this
question.

**Decision needed:** allow a narrow dependency from hosted interpreter adapters
to independent Omega target/ABI and mechanism libraries (recommended), while
keeping Psi compilation and the Terminal execution core independent of Omega;
or place target-aware host adapters outside Psi and keep the facade portable
through explicit host bindings. The latter changes the task's adapter placement,
not Terminal semantics. Neither option permits stage IRs, compiler products,
source rechecking or native realization to enter the interpreter.

**Implementation customer:** descriptor input/output and clock programs exercised
through `omega run --both`, plus direct Terminal-artifact embedding. Only facade
and target-aware adapter placement in **TERMINAL-RUNTIME-DRIVERS** waits on this
decision; its remaining storage, input and import families can proceed.

### Q3 - How does an OCREQ v1 package row derive its `PackageKeyIdentity`?

Named decision: `ocreq-package-key-identity-derivation`.

**Customer:** the standalone compiler request's phase 3, which both
compilers must implement identically before either claims the V1 boundary
(**OMEGA-D** in `TASKS_BOOTSTRAP.md`, **OMEGA-PRODUCT-COMPILER-SOURCE** in
`TASKS.md`). D and the Omega-written product compiler C both decode phases
0, 1, 2 and 6 today and stop every well-formed request at
`coverage_request_semantics`. Phases 4, 5, 7 and 8 (graph, snapshots,
admissions, commitment) are fully specified, but a compiler cannot publish
their refusals until it implements phase 3: a request failing both phase 3
and a later phase must report phase 3 under the fixed selection order.

**Existing rules:** [the request contract](wiki/spec/build/compiler_request.md#canonical-subject)
requires package rows "strictly ordered by recomputed `PackageKeyIdentity`"
and says a row's name, lineage and resolutions "supply the structural fields
the `PackageKeyIdentity` recomputation and ordering rules consume", but no
rule is written there. The row carries `name`, a two-valued `lineage_kind`
(`external_local`, `git`) and `lineage`, "canonical lineage identity text"
whose "canonicalization is acquisition-owned".
[Declarations](wiki/spec/build/declarations.md) defines `PackageKey` as the
declared name plus canonical source lineage, without bytes.
[Sources](wiki/spec/packages/sources.md) keeps Git lineage's transport, user,
host, port and path, without a text form. The only implementation is Rust's
package manager (`omega-rust/omega/packages/package-manager/src/declarations/identity.rs`
and `.../source-acquisition/src/identity/git.rs`): SHA-256 over
length-framed fields — `omega-package-key-identity-v1`, the name, then a
variant label (`github`, `gitlab`, `git`, `workspace`, `external-local`) and
that variant's structured fields. Five variants with structured fields do not
map onto the wire's two kinds and one text field, and nothing serializes
lineage as text.

**Decision needed:**

1. **Recommended: define the key over the wire fields.**
   `PackageKeyIdentity = SHA-256(frame("omega.ocreq.package-key.v1") ||
   frame(name) || u32_le(lineage_kind) || frame(lineage))`, with
   `frame(x) = u32_le(len(x)) || x`, ordered by unsigned byte comparison of the
   32-byte digest. Every compiler computes it from the decoded row alone. It
   need not equal the package manager's lock identity; acquisition is then
   responsible for canonical lineage text, and the request carries no claim
   that two spellings name one repository.
2. **Make the request key the package manager's key.** Specify a canonical
   text form for every lineage variant (or widen `lineage_kind` to the five
   variants with structured fields) and the exact Rust framing in the
   contract, so locks, reviews and requests agree on one identity. This
   needs a request revision if the row's fields change.
3. **Order rows without a digest** (by name, then lineage kind, then lineage
   bytes) and drop "recomputed identity" from phase 3, keeping duplicate and
   identity-mismatch checks on those fields.

### Q4 - Are `min`, `max`, `abs`, `clamp` and `sqrt` language functions?

Named decision: `executable-numeric-builtins`.

**Customer:** the Omega-written product compiler resolving the bundled
library (**OMEGA-PRODUCT-COMPILER-SOURCE** in `TASKS.md`):
`core/numeric_conversion.omg` calls `clamp` 31 times, and its saturating
narrowings depend on the result's bound (`let capped: u8 = clamp(value, 0,
127); capped as i8` is a representable conversion only because `capped <=
127`). The corpus and samples call `max` 11 times, `sqrt` 6 times, `clamp`
7 times and `min` twice. The product resolver binds none of them, and the
product checker cannot prove what follows a call without their meaning.

**Existing rules:** [expressions](wiki/spec/language/expressions.md) names
the compiler-known core queries (`address_of`, `size_of`, `align_of`,
`code_layout`) and no numeric function.
[Numeric values](wiki/spec/language/numeric_values.md) defines operators,
policies (`Saturating` clamps an operation's result) and target-selected
instruction lowerings, not callable functions.
[Contracts](wiki/spec/proofs/contracts.md) defines only the proof-only
`embed`. The language guide uses none of these names. The only definition is
the Rust reference compiler: `omega-rust/psi/foundation/symbols/src/builtin`
lists `min`, `max`, `abs`, `clamp` and `sqrt` beside the assembly
intrinsics, and stage 02's `lowering/intrinsic_calls.rs` rewrites
`abs(x)` to `max(x, 0 - x)` and `clamp(v, lo, hi)` to
`min(max(v, lo), hi)`, so `min` and `max` are the primitives.

**Decision needed:**

1. **Recommended: specify `min` and `max` as compiler-known core functions**
   in expressions.md, beside the core queries: two operands of one numeric
   carrier (anonymous literals land on it), the result in that carrier, equal
   to one operand and ordered against both, which is what a checker proves
   with. Define `abs` and `clamp` as their documented expansions, and `sqrt`
   as a float operation lowered like the other target-selected instructions.
   Both compilers then implement the same written meaning.
2. **Make them ordinary core library machines** (per carrier, or generic
   over an ordered numeric bound) whose `ensures` facts state the same
   bounds, rewrite the library's and samples' calls to name them, and retire
   the compiler builtins. No new language surface remains, but every carrier
   needs a machine or the generics need an ordering bound, and `sqrt` still
   needs a target instruction.
3. **Keep them unspecified** in the Rust compiler only. The product
   compiler then cannot compile the bundled library without inventing their
   meaning, so this blocks the product compiler's library closure.

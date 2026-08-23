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

Last pruned: 2026-08-23.

## Q1 — How does a target package declare a nominal foreign endpoint?

The foreign-binding model requires source to cite one namespace-owned
`DllImportId`, for example `Windows::Kernel32::WriteFile`, while raw library and
export bytes live only in sealed, fingerprinted target/link metadata. The
repository does not yet define the declaration that creates that nominal value
or the authored target input that maps it to those raw bytes. An ordinary
`const` cannot construct the opaque ID without reopening free pairing, and
deriving the ID from either strings or the realization machine would contradict
the settled identity rule.

Choose the target-package declaration and metadata-supply surface. It must:

- create one resolved nominal symbol usable as a `DllImportId` expression;
- bind that symbol inseparably to one library/export pair in sealed target/link
  metadata, with no raw strings in ordinary Omega source;
- make ownership, target applicability, duplicate/missing mapping rejection,
  fingerprinting, and package visibility explicit; and
- generalize coherently to `CallingPlanId`, firmware/table IDs, and other
  mechanism-specific nominal values without inventing a string-backed escape.

Recommended direction: a target-package-owned nominal-ID declaration plus a
separate sealed target metadata record keyed by that resolved declaration. Keep
`build.omg` limited to selecting target/provider declarations; it must neither
author linker spellings nor manufacture IDs.

## Q2 — How does a named guarantee declare its result-case guard?

Named `ensures proof: P` outputs and selective proof-output bindings are live
for unconditional guarantees. The settled caller surface also allows a proof
selector inside the matching result arm, but no declaration syntax currently
states that a named guarantee exists only for one result case. The checker must
not infer that association from `P`, visible case facts, or the producer's body.

Choose the source form and normalized identity for a result-case-guarded named
guarantee. It must:

- name one exact case of the machine's declared result sum, with no ambient or
  body-shape inference;
- remain public signature content, so moving a selector between cases is a
  breaking proof-interface change;
- require definite assignment exactly once on ordinary exits producing that
  case, and no assignment on other or crash-only exits;
- make the selector available only in that case's caller arm, after the `;`
  universe separator, while an omitted selector still contributes its fact
  only in that arm; and
- retain the normalized case identity through checked facts, Terminal Psi,
  codec identity, and independent verifier replay.

Recommended direction: extend the existing named `ensures` clause with an
explicit result-case selector, for example `ensures Success => proof: P`, and
normalize `Success` to the exact result-type case symbol. Keep unconditional
`ensures proof: P` unchanged. Do not admit an arbitrary Boolean guard here: the
customer is outcome-specific availability, and general guarded contracts would
introduce a larger proof and compatibility surface.

## Q3 — What source authority expresses variadic `Respects` evidence?

Quotient operations are selected explicitly as
`Quotient::lift<F, Respect>(...)` or `Quotient::define<F, Respect>(...)`. The
compiler already derives the representative operation's ordered runtime
telescope, the pointwise input relation `RA`, and result relation `RR`. What is
missing is a source/core declaration that lets the explicitly named `Respect`
conformance certify `Respects<F, RA, RR>` when `RA` has one entry per runtime
operand.

Choose the declaration and application model for this compiler-derived,
variadic proof interface. It must:

- retain one exact named conformance selected by the quotient owner, with no
  structural proof-machine discovery or visible-unique inference;
- derive operand positions from the normalized representative telescope,
  including attached `self` at position zero, rather than use an arity-specific
  `Respects1`/`Respects2` family;
- make the complete `F`/`RA`/`RR` application and proof rows available to
  checked and Terminal verification without exposing a runtime dictionary;
- support generic representative applications only after their static
  telescope is closed; and
- remain a reusable proof-interface mechanism rather than privileged syntax
  attached only to `Quotient::lift`.

Recommended direction: add a sealed proof-interface binder for a normalized
relation telescope, allowing core to declare one variadic `Respects` trait
whose applications the compiler constructs but whose named conformances remain
ordinary source declarations. Do not encode the telescope as an untyped list,
generate arity-indexed traits, or let the lift operation discover a proof by
shape.

## Q4 — How does a native layout declare a private callback demand?

Registered-callback lowering already maps one nominal static-machine binder to
one native parameter or nested layout place. A nested destination is valid only
when the independently validated native layout declares a typed private slot
for that exact callback requirement. That slot is absent from the semantic
schema and its layout, slot, and requirement identities are compiler-issued,
but no source/library input currently creates the demand.

Choose the target-package declaration and layout-policy input for one private
callback demand. It must:

- declare the native slot independently of the registrar's materialization row,
  so the supply cannot authorize its own destination;
- name one exact signature-free callback requirement and reject overload
  ambiguity without authored numeric identities;
- keep the slot absent from source projection, read, write, serialization, and
  runtime value topology while retaining it in normalized layout identity;
- allow the compiler to derive `LayoutPlanId`, `LayoutSlotId`, and
  `CallbackRequirementId` from resolved declarations and the validated plan;
  and
- support exact missing, duplicate, overlap, wrong-requirement, wrong-layout,
  and replay-drift rejection when the calling plan closes the demand.

Recommended direction: extend the ordinary `Schema`/`Plan` library vocabulary
with a bounded compiler-private slot source whose authored inputs are a stable
native slot declaration and exact callback-requirement path. The compiler
resolves those names into opaque identities during layout evaluation. Do not
put raw IDs or field offsets in source, infer the demand from the callback row,
or expose a callback-address-shaped semantic field.

## Q5 — What exact language contract constitutes Delta v1?

The architectural frame is already settled: Delta must be a stable, independent,
robust C-like compiler-host language before it hosts `omega-bootstrap`; it is
not an Omega subset, although Omega-shaped syntax is preferred where cheap.
Runtime-sized allocation from explicit fixed backing, typed/indexed arenas,
bulk reclamation, and specified exhaustion are permitted. The unresolved ruling
is the literal Delta-v1 contract. The frozen D0 profile proves that the current
slices can use a bounded surface, but it deliberately does not settle Delta's
language. The Rust reference currently accepts scalar spellings whose execution
is still uniformly `i32`, target backends disagree on some arithmetic domains
and division edges, boundary declarations are partly hardwired, and ordered
source concatenation substitutes for a source-unit contract. Those are not a
specification.

Choose one versioned Delta-v1 contract. The ruling must jointly settle:

- the source model: byte/Unicode policy, identifiers, comments and string
  literals, canonical source extension, declaration visibility, and whether v1
  has native modules or only one deterministic length-delimited source bundle;
- the value and representation model: the exact fixed-width scalar set,
  Boolean representation, arrays/slices, record and payload-sum layout,
  zero-initialization, alignment, indexing, receiver/reference rules, integer
  arena handles, aliasing, and the validity of handles across bulk reset;
- expression and control meaning: evaluation order and precedence, exact
  trapping/wrapping/saturating arithmetic including division and shift edges,
  state transitions, loops, calls, recursion, fallthrough, entry, and which
  call/stack ceilings are language-visible resource limits rather than backend
  accidents;
- the allocation and failure model: whether fixed-backing allocation is a
  language primitive or a required library contract, its `reserve`/mark/reset
  and exhaustion behavior, runtime traps versus checked failures, and the
  observable effect of resource failure; and
- the closed compiler-host boundary: exact byte input/output and process-exit
  operations, host-I/O impossibility, declaration/signature validation, and
  whether arithmetic domains, range refinements, contracts, mixed field-plus-
  case data, or any other experimental corpus feature belongs to Delta v1.

Recommended direction: freeze a deliberately small, versioned systems language
around the already self-hosted core: `i32` plus `u8` storage, deterministic
two's-complement arithmetic with explicit Trapping/Wrapping/Saturating domains,
checked arrays and integer-offset arenas, predictable records and payload sums,
state machines/loops/recursion, runtime-sized reservations from one explicit
fixed-backing allocator, one canonical source-bundle format, and a closed
byte-I/O/process boundary. Keep ambient host pointers and allocation, individual
`free`, GC, threads, atomics, native modules, mixed field-plus-case data, and
proof-only/refinement syntax outside v1 unless an identified compiler-host
requirement outweighs their lower-rung assurance cost. Treat backend parameter
counts and storage ceilings as checked implementation-profile limits unless the
ruling intentionally makes them portable language limits.

## Q6 — What is the exact authored `build.omg` dependency API?

The package manager design is settled on `build.omg` as the authored source of
truth for dependency aliases and pins, while `omega.lock` remains
machine-written evidence. The current docs intentionally show only a likely
shape. Full `omega install` editing cannot be implemented without choosing the
actual Build vocabulary, normalized dependency-row identity, and unsupported
source-edit fallback behavior.

Choose the dependency-binding API and edit contract. It must:

- bind one package-local alias to one exact source identity or revision request;
- preserve the kebab-case package identity versus snake_case in-code alias
  convention;
- make the pin source-auditable without turning `omega.lock` into an authored
  manifest;
- define how `omega install` edits simple cases and what patch/proposal it
  emits for unsupported `build.omg` patterns;
- reject alias ambiguity, duplicate aliases, and spelling-equivalent package
  identities (`foo-bar` versus `foo_bar`) fail-closed; and
- leave root/provider selections under the same explicit `build.omg` authority
  model rather than creating a second package language.

Recommended direction: add a small first-class `Build.dependencies.bind(...)`
vocabulary with typed `Source` constructors for local and Git sources, while
keeping the CLI editor conservative: rewrite only canonical/simple rows and
print a proposed patch otherwise.

## Q7 — Does install run dependency `build.omg` immediately or preflight first?

The security boundary is settled: resolver-owned retrieval precedes dependency
execution, and dependency builds never inherit resolver or root-package
authority. What remains open is the install/admission sequence. Running
dependency `build.omg` immediately gives accurate generated-source and provider
evidence but executes newly downloaded build code sooner. Static preflight first
can reject obvious policy violations before execution, but may be incomplete
for packages whose manifest depends on admitted build outputs.

Choose the install-time dependency-build sequence. It must:

- define what evidence can be derived before executing a downloaded
  dependency's `build.omg`;
- specify which build-host providers, if any, may be supplied during install
  admission and how their scopes are derived;
- keep resolver network/archive authority separate from package build network
  authority;
- decide whether generated Omega source is required before manifest
  fingerprinting;
- define failure UX for packages that need build execution to reveal their full
  capability manifest; and
- preserve deterministic lock/update behavior across local path, Git, and
  future archive transports.

Recommended direction: perform a static preflight over fetched source and
`build.omg` dependency rows first, then run dependency `build.omg` only with
package-scoped admitted providers to produce the final manifest. Record both
the preflight verdict and final build observation in lock evidence.

## Q8 — What exact checked evidence defines a package capability manifest?

The package-manager model requires one compiler-derived capability manifest per
resolved package, including public API identity, exported service reach,
dependency aliases, provider requirements and selections, routed
qualifications, build-machine observation, unresolved installation-bound rows,
capability-flow source rows, and trust/admission receipts. The compiler already
emits an executable capability manifest for a selected entry machine, but that
artifact is entry-oriented and intentionally reports no entry reach for
entry-agnostic library checking. Reusing it as the package manifest would lose
the package/public-library boundary that package admission is supposed to
protect.

Choose the package-admission evidence boundary and normalized extraction
contract. It must:

- derive package identity and source identity without hand-authored package
  manifest files;
- define the public API contract identity for a library package independently
  of any selected executable entry;
- compute exported service reach for every published callable or boundary
  surface that dependents can name, not only one `ProgramEntry`;
- retain exact dependency alias rows from `build.omg` once the dependency API
  is settled;
- record build-machine service reach, observation ceiling, realized observation
  class, and receipts without treating build-host observations as semantic
  proof evidence;
- include provider requirements, selected provider plans, provider origins,
  routed qualifications, accepted claims, and unresolved installation-bound
  reach rows in a replayable normalized form;
- emit capability-flow counts and source rows for public authority movement
  (`uses`, `stores`, `acquires`, `returns`, `derives`) with enough provenance
  for reviewer guidance;
- reject open deferred proofs, spoofed boundary traits, fake math, missing
  provider evidence, and unresolved installation-bound rows according to the
  package-admission profile; and
- produce byte-identical manifests for equal source/evidence across local path,
  Git, and future archive transports.

Recommended direction: add a package-admission compilation profile that walks
the checked public package surface and selected build/provider evidence to emit
one normalized package manifest. Keep the existing executable capability
manifest as an entry artifact; do not reinterpret it as package evidence, and
do not allow package authors to supply or patch the derived manifest by hand.

# Compilation products

Product requests, kept outcomes and publication, apart from the compiler
driver. This is not a pipeline phase: it validates invocation requests and
consumes finished artifacts; it never parses source, evaluates a Build, lowers
a program or calls back into compilation.

[`checked_compilation.rs`](src/checked_compilation.rs) owns the immutable checked
product: whole selected-execution results, source sealing, metadata,
generated-source handoffs and the subject of the production report. The
coordinator passes finished stage products in `CheckedCompilationInputs`, as
their producers returned them: selected execution, the shared evaluated
Build, this compilation's Build admission, and provider settlement. Consumers
borrow the record they read, not per-field accessors.
Sealing rechecks the source record but neither accepts the package nor grants
native permissions.
Its [Terminal projections](src/checked_compilation/terminal.rs) build native
proposal requests and Terminal reports from the kept checked records.
Inspection compares results using the same checked provider selections;
stage 00 owns the independent reconstruction. Callback production and native
proposal runs stay visible in the coordinator. Package review uses this product without depending on compiler
orchestration, and still rebuilds its evidence independently.
For a second pass over the same invocation, `RetainedCompilationChecks` carries
the successful Build and Product checker verdicts separately. Psi reuses each
only for its exact program, checkpoint and receipt inputs, and still checks the
invocation's permissions. These optional, invocation-local results stay out of
semantic identity and reports; they neither cache effects nor grant execution.
[`evaluated_build.rs`](src/evaluated_build.rs) owns the shared Build
configuration and execution results, plus the per-compilation Build
admission (application declaration, evaluated machine identity, restricted host
requests) that cannot be shared. Build evaluation produces them; consumers here
do not depend on the evaluator.

[compile_request.rs](src/compile_request.rs) normalizes requests: one root,
the requested product and invocation policy, and explicit target and output
destinations. Callers do not build per-target pipeline configurations;
admission expands the destinations into execution records sharing one
immutable policy, checks staging directories are separate, attaches package
inputs, and checks rollback is allowed for the requested stop.
Its validated target records give the coordinator read-only inputs, and it
accepts the evaluated Build's requirements before production starts.
Normalizing the set of target names belongs to the backend target catalog.

[prepared_source.rs](src/prepared_source.rs) keeps parsed sources and complete
typed frontiers for reuse when inputs are identical. It accepts roots, checks
the live package record on reuse and owns storage slots, but never loads
source or runs a frontend stage. The compiler runs a pending frontier and passes
in the result. The immutable parse checkpoint belongs to the package crate;
its root is the only root identity. Checked requests and
outcomes use this type instead of compiler-owned cache types.

[prepared_build.rs](src/prepared_build.rs) keeps completed typed Build
continuations for reuse within one invocation, keyed by the package crate's
exact Build-input key. The compiler accepts each target before lookup and, on
a miss, runs the frontend and Build itself. Storing never does stage work; each
target checks and seals the reused program itself. Failed continuations
are dropped, and the last target makes no unused copy.

[compile_outcomes.rs](src/compile_outcomes.rs) keeps successful reports and
refusals together, computes their batch commitments, and refuses to extract one
report when others exist. A refusal whose every error names an exceeded
private provision is the [compiler request](../../../../wiki/spec/build/compiler_request.md#validation-and-failures)'s
`Incomplete` outcome (halt value 2), recorded with that provision; any other
refusal is `Reject` (halt value 1). Neither a request nor an outcome grants trust or
receiving permission, or shows that a target is supported.

Start at [compilation_products.rs](src/compilation_products.rs) for
`CompileReport`: one check, build-only, Terminal, retained-native or published
native product and the evidence that kind can carry. Construction and state
changes check that artifact, rollback, application-package and
build-observation records agree. The compiler and package admission use this
same type; reports do not invent lower-level evidence or grant receiving
permission.

`CompileReport::verify_native_source_metadata` checks a retained native result
against the originating compilation's source count, target profile, native
target and application metadata. Compiler orchestration supplies those facts;
this crate checks that they agree without keeping the checked frontend or
adding another receipt type.

[publication.rs](src/publication.rs) is where installation starts. It refuses
check-only, repeated or inconsistent publication before calling
`publish_native` or `publish_terminal`, then publishes completed build
companions.
`platform_custody` stages files; native artifacts own executable and package
receipts; package compilation installs completed outputs.

This crate depends on those crates, not on `compiler`, `build-settings`, the
CLI or the package-manager application. Moving this lifecycle into a backend
artifact crate would mix native mechanics with package and portable products;
generic file utilities stay free of it.

Validation uses the CLI's existing publication tests, including replay of
completed outputs and rejection of tampered ones.
`mbx check -p products` checks this ownership boundary directly.

## Native component export index

Flat native publication stages `omega-components.tsv` beside the executable and
its content-addressed components. Its first line is `omega-native-components-v1`;
each following line has three tab-separated fields: the calling application's
complete `requirement_identity`, the local component filename, and its exact
export symbol. With no components, the index holds only the header. The description
stays `<filename>.component`.

The index describes linkage for build adapters. It is not a proof, a provider
choice, or permission to install or run. Consumers must check their expected
requirements, local file placement, actual exports and artifact restrictions. Row order is not an ABI. Native publication still
validates each sealed component and stages the index with the images that
produced it; it does not reopen source to infer names.

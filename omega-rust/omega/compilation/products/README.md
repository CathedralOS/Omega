# Compilation products

Product requests, retained outcomes and publication, independent of the compiler
driver. This is not a pipeline phase: it validates invocation contracts and
consumes produced artifacts, never parses source, evaluates Build, lowers a
program or re-enters compilation.

[`checked_compilation.rs`](src/checked_compilation.rs) owns the immutable checked
product: whole selected-execution results, source-custody sealing, metadata,
generated-source handoffs and the production-report subject. The coordinator
supplies completed stage products through `CheckedCompilationInputs`, each as
the record its producer returned: selected execution, the shared evaluated
Build, this compilation's Build admission, provider settlement and checked-tree
product selection. Consumers borrow the unit they read rather than per-field
accessors. Sealing rejoins source custody but does not grant package admission
or native authority.
Its [Terminal projections](src/checked_compilation/terminal.rs) assemble native
proposal requests and Terminal reports from retained checked custody. Inspection
uses those same checked provider selections for comparison-custody joins; stage
00 owns the independent reconstruction. Callback production and native proposal
execution stay visible in the coordinator, not inside the checked product.
Package review consumes this product directly, without a production dependency
on compiler orchestration, and still independently reconstructs evidence.
[`evaluated_build.rs`](src/evaluated_build.rs) owns the shared retained Build
configuration and execution-result records, plus the per-compilation Build
admission (application declaration, evaluated machine identity, restricted host
requests) that cannot live in the shared configuration. Build evaluation
produces them; product consumers do not depend on the evaluator.

[compile_request.rs](src/compile_request.rs) owns request normalization:
target-local policies, separate staging directories, package input joins, and
rollback eligibility for the requested stop. Its validated target records expose
read-only inputs to the coordinator. Evaluated Build requirements are admitted
by the same product owner before production dispatch. Exact target-name set
normalization belongs to the backend target catalog.

[prepared_source.rs](src/prepared_source.rs) retains parsed sources and complete
typed frontiers for exact-input reuse. It owns root admission, live package
custody checks on reuse and retention slots, but never loads source or runs a
frontend stage. The compiler explicitly executes a pending frontier and supplies
the completed result. The immutable parse checkpoint remains package-owned;
its root is the sole root identity. Checked requests and outcomes use this same
product rather than depending on compiler-owned cache types.

[prepared_build.rs](src/prepared_build.rs) owns completed typed Build continuations
and invocation-local reuse under the package owner's exact Build-input key. The
compiler admits each target before lookup and visibly runs the frontend and Build
on a miss. Retention never performs stage work; each target independently checks
and seals the reused program. Failed continuations are not retained, and the last
target creates no unused retained copy.

[compile_outcomes.rs](src/compile_outcomes.rs) retains successful reports and
refusals together, derives their batch commitments, and refuses single-report
extraction when siblings exist. A refusal whose every error names an exceeded
private provision is the [compiler request](../../../../wiki/spec/build/compiler_request.md#validation-and-failures)'s
`Incomplete` outcome (halt value 2), recorded with that provision; any other
refusal is `Reject` (halt value 1). Neither a request nor an outcome grants trust,
receiving authority, or evidence that a target is supported.

Start at [compilation_products.rs](src/compilation_products.rs) for
`CompileReport`: one check, build-only, Terminal, retained-native or published
native product and the evidence that kind can carry. Construction and custody
transitions validate artifact, rollback, application-package and build-observation
joins. Compiler and package-admission routes retain this same type; reports do
not manufacture lower-level evidence or grant receiving authority.

Proof-product eligibility is shared by early request admission and direct report
context attachment. Check-only, artifact-only and object products cannot carry
proof requests; Terminal products can request only Psi proofs. Publication freezes
the proof context. A package's pending native requests stay with its native
production flow, not on the intermediate Terminal report.

`CompileReport::verify_native_source_metadata` checks a retained native result
against the originating compilation's source count, target profile, native
target and application metadata. Compiler orchestration supplies those facts;
the product owner checks their joins without retaining the checked frontend or
introducing another receipt carrier.

[publication.rs](src/publication.rs) is the installation entrance. It refuses
check-only, repeated or inconsistent publication before dispatching to
`publication/native.rs` or `publication/terminal.rs`, then publishes completed
build companions. Proof pairs are prepared and verified before installing bytes.
Platform custody owns file staging; native artifacts own executable/package
receipts; package compilation owns completed-output installation.

[production_manifest.rs](src/production_manifest.rs) binds a package-owned
production subject to one artifact. [batch_manifest.rs](src/batch_manifest.rs)
commits the configured target outcomes, not a target-support claim.
[pcc.rs](src/pcc.rs) records published pair sizes and routes receiver checking
to the Psi or native checker according to the decoded envelope.

The package depends on those evidence/product owners, not `compiler`,
`build-evaluation`, the CLI or the package-manager application. Putting this
cross-product lifecycle in a backend artifact owner would conflate native
mechanics with package and portable products; generic file utilities must stay
free of these semantic dependencies.

Validation uses the compiler corpus for production and the CLI's existing
publication tests, including completed output replay and tamper rejection.
`cargo check -p compilation-products` checks this ownership boundary directly.

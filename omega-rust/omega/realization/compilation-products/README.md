# Compilation products

Product requests, retained outcomes and publication, independent of the compiler
driver. This is not a pipeline phase: it validates invocation contracts and
consumes produced artifacts, never parses source, evaluates Build, lowers a
program or re-enters compilation.

[compile_request.rs](src/compile_request.rs) owns request normalization:
target-local policies, separate staging directories, package input joins, and
rollback eligibility for the requested stop. Its validated target records expose
read-only inputs to the coordinator. Evaluated Build requirements are admitted
by the same product owner before production dispatch. Exact target-name set
normalization belongs to the backend target catalog.

[compile_outcomes.rs](src/compile_outcomes.rs) retains successful reports and
rejections together, derives their batch commitments, and refuses single-report
extraction when siblings exist. Neither a request nor an outcome grants trust,
receiving authority, or evidence that a target is supported.

Start at [compilation_products.rs](src/compilation_products.rs) for
`CompileReport`: one check, build-only, Terminal, retained-native or published
native product and the evidence that kind can carry. Construction and custody
transitions validate artifact, rollback, application-package and build-observation
joins. Compiler and package-admission routes retain this same type; reports do
not manufacture lower-level evidence or grant receiving authority.

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

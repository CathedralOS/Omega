# Native compilation

The application first calls [prepare](src/preparation.rs) with retained Terminal
and a `NativeCompileRequest`: component calls, imports, image custody and policy.
Preparation selects native work and exposes each artifact's canonical scalar
demand. The application sponsors scalar completion outside this crate, then passes
the same `PreparedNativeCompilation` to [compile](src/compiler.rs). Compilation
requires every required scalar completion before emitting any component or root image;
it does not call an evaluator, reopen source or repeat preparation. Failures return
the original image request and diagnostics.

Empty and nonempty component rosters follow that same sequence. Callable export
emission remains separate because it produces an independent image, not an
application entry. These are explicit input-preparation and consumption steps,
not competing native compilation routes.

[pipeline.rs](src/pipeline.rs) sequences the body transformations for application
entries, callbacks and component exports: abstract optimization, target lowering,
instruction selection, selected optimization, register allocation, post-allocation
machine construction, frame preparation, encoding, layout, layout optimization,
realization assembly and object emission. There is one implementation of that sequence.
Its internal target boundary lets callers capture runtime layout companions
before instruction selection, preserving error order without a callback hook.
Machine emission owns frame geometry and its byte protocol, then joins the
pipeline's encoded/layout products with the exit contract and source identities.
It no longer drives compiler stages 07-09 behind the pipeline's entrance.

[realization.rs](src/realization.rs) admits the entry and captures runtime
provisions. [Object emission](src/realization/object_emission.rs) uses the shared
pipeline, discovers surviving callbacks and publishes the composed object;
callable emission uses that same pipeline and retains callable authority.
Neither caller independently selects or sequences transformation stages. Native artifact
ownership includes the runtime provisions; the source coordinator does not own
their types.

The production dependency closure admits only native stages, representations,
backend/foundation libraries, Terminal contracts and their independent checking
libraries. It cannot import source IR, a source checker, an interpreter, Build
execution or package orchestration through an indirect dependency. Run
`python tools/check_native_boundary.py` from the repository root to check all
declared configurations. Optional and target-specific dependencies count.

The source coordinator still produces one retained proposal per target. This
facade does not finish the separate work of moving provider selection after a
single target-neutral Psi check and Terminal publication.

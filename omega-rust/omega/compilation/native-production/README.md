# Native compilation

Start at [native_compilation.rs](src/native_compilation.rs): consume a retained
Terminal product and explicit realization policy, prepare its providers, admit
the entry and emit the requested image. On failure the caller recovers the
original image request and diagnostics. No source compilation happens here.

[realization.rs](src/realization.rs) captures the admitted runtime provisions
and drives [object emission](src/realization/object_emission.rs). That operation
chains abstract optimization, target lowering, the
[physical pipeline](src/physical_pipeline.rs) and object publication. Callback
bodies use the same physical pipeline, not another compiler. Native artifact
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

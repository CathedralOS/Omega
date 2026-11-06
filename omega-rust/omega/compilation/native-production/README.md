# Native compilation

The application first calls [prepare](src/preparation.rs) with retained Terminal
and a `NativeCompileRequest`: component calls, imports, image custody and policy.
Preparation selects native work and exposes each artifact's canonical scalar
demand. The application sponsors scalar completion outside this crate, then passes
the same `PreparedNativeCompilation` to [compile](src/compile.rs). Compilation
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
It no longer drives compiler Omega 07-09 behind the pipeline's entrance.

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

## Reviewing a bootstrap environment assumption

Some early hardware preparation runs before the program publishes its interrupt
handlers. The caller may explicitly assume that no asynchronous NMI, machine
check, or physical failure occurs during that exact interval. This is the narrow
platform premise in [bootstrap safety](../../../../wiki/spec/build/hardware_materialization.md#bootstrap-safety)
and [UEFI handoff](../../../../wiki/spec/build/uefi_entry.md#hardware-preparation-and-reporting).
It does not establish CPU configuration, masked ordinary interrupts, exclusive
storage, mappings, stack capacity or absence of software faults.

The normal project API exposes the exact identities without compiling a private
replacement program. On an already admitted package, use the
[review example](../../build-system/orchestration/examples/installation_assumption_review.rs):

```sh
mbx run -p orchestration --example installation_assumption_review -- inspect path/main.omg uefi_x86_64 build/boot-review
cat build/boot-review/catalog.txt
```

This runs ordinary project discovery, Build and Terminal production. The catalog
names the selected program entry, advertised private entries and every retained
installation operation with its owning declaration. `terminal.psi` is that
actual canonical artifact. Choose the intended entry and installation from this
catalog; do not copy numeric IDs from another compilation. A name is displayed
for review and never selects or authorizes the operation.

Write and review an environment description recording the exact loader/emulator,
CPU configuration, launch arguments, injection policy, and the assumptions being
accepted. Keep its bytes with the resulting image. A QEMU launch can establish
its configured CPU and delivery settings, including whether the harness injects
NMI before publication. It does not prove the absence of spontaneous machine
checks or physical failures. Those remain explicit assumptions of this result.
Then create and inspect the record, substituting the two reviewed numeric IDs:

```sh
mbx run -p orchestration --example installation_assumption_review -- accept-bootstrap build/boot-review/terminal.psi uefi_x86_64 ENTRY_ID INSTALLATION_ID environment.md 'reviewer and launch owner' build/bootstrap-assumptions.bin
mbx run -p orchestration --example installation_assumption_review -- show build/bootstrap-assumptions.bin
omega --target uefi_x86_64 --bootstrap-assumptions build/bootstrap-assumptions.bin --report-file build/boot-report.txt path/main.omg
```

`accept-bootstrap` or `accept-interrupt-delivery` hashes the actual description and retains the source/caller provenance;
it does not observe hardware or prove the assumption. Native admission rejoins
Terminal identity, target, advertised entry, selected call closure, installation
and the publication endpoint. A changed program needs a new review. Absence,
wrong target, unrelated entry or wrong installation refuses. Two-target requests
partition their explicitly supplied rows before native admission; neither target
borrows the other's assumptions.

Embedders put bootstrap and installed-delivery records in
`InstallationEnvironmentAssumption`. Use `encode_installation_assumptions` and
`decode_installation_assumptions` from `compilation_products`, then supply the
rows through `NativeProductRequest::installation_assumptions` or
`CompileRequest::with_installation_assumptions`.
Saved native inputs retain those same rows. Recovery returns them separately
from package permissions; the ordinary `NativeCompileRequest` consumes them.
The sealed installation scope retains the requirement, private-entry replay
rejoins the parent's exact row, and the native artifact identity commits it.
`ObjectArtifact::installation_environment_assumptions` exposes the retained
conditions for downstream review. No target or absent permission policy silently
supplies this premise.

This input transport does not itself establish a complete interrupt floor.
The multiple-entry guards, asynchronous stack bounds, publication ordering and
fatal termination still require their own complete native and CPU evidence.

For a fatal exception floor, review a separate installed-delivery condition.
It applies while the named Scope is installed, not to the bootstrap interval.
The environment description must explicitly identify one execution context,
ordinary IRQ routing outside vectors 0–31, true NMI/MCE delivery, and absence of
guest debug injection, tracing and active hardware breakpoints. Host debugger
pauses are distinct from injecting an exception into the guest. NMI/MCE are not
assumed absent; native stack composition and software-fault checks remain
separate requirements.

```sh
mbx run -p orchestration --example installation_assumption_review -- accept-interrupt-delivery build/boot-review/terminal.psi uefi_x86_64 ENTRY_ID INSTALLATION_ID environment.md 'reviewer and launch owner' build/interrupt-delivery.bin
mbx run -p orchestration --example installation_assumption_review -- show build/interrupt-delivery.bin
omega --target uefi_x86_64 --bootstrap-assumptions build/bootstrap-assumptions.bin --interrupt-delivery-assumptions build/interrupt-delivery.bin --report-file build/boot-report.txt path/main.omg
```

The two records have distinct closed condition tags. Neither record can satisfy
the other requirement. Requests without these explicit inputs receive no such
assumptions, and the compile report lists both conditions with their exact
program, target, entry, installation and reviewed-description identities.

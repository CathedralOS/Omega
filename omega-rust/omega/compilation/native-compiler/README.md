# Native compilation

The application first calls
[prepare](src/preparation.rs) with the retained Terminal Psi and a
`NativeCompileRequest` (component calls, imports, image ownership and policy).
Preparation selects native work and lists the scalar values each artifact
needs. It reads only the retained Terminal product and its native proposal;
no Omega stage runs yet. The application computes the values outside this
crate, then passes the same `PreparedNativeCompilation` to
[compile](src/compile.rs), which runs Omega 00 to 09 once, in order. Compilation
needs every scalar before emitting any component or root image; it does not
call an evaluator, reopen source or repeat preparation. Failures return
the original image request and diagnostics.

Programs with and without components follow the same two calls. Callable export
emission stays separate because it produces an independent image, not an
application entry. Prepare and compile are steps of one route.

[pipeline.rs](src/pipeline.rs) sequences the body transformations for application
entries, callbacks and component exports: abstract optimization, target lowering,
instruction selection, selected optimization, register allocation, post-allocation
machine construction, frame preparation, encoding, layout, layout optimization,
realization assembly and object emission. That sequence has one implementation.
Its internal target boundary lets callers capture runtime layout companions
before instruction selection, keeping errors in order without a callback.
Machine emission owns frame layout and its byte format, then joins the
pipeline's encoding and layout with the exit contract and source identities. It does not run Omega stages 07-09 itself outside the pipeline.

[realization.rs](src/realization.rs) accepts the entry and captures what the
runtime must provide. [Object emission](src/realization/object_emission.rs)
runs the shared pipeline, finds the callbacks that survive and publishes the
combined object; callable emission runs the same pipeline and keeps the
callable's permissions. Neither caller chooses or orders stages itself. The
native artifact, not the source coordinator, owns the runtime provisions and
their types.

Production dependencies, direct or indirect, may include only
native stages, representations, backend and foundation libraries, Terminal
contracts and the libraries that check them independently. They must not reach
source IR, a source checker, an interpreter, Build execution or package
orchestration. Run `mbx xtask check-boundary` from the repository root to check
every declared configuration; optional and target-specific dependencies count.

Compiling for several targets reuses one checked Terminal program. The source
and package entry points check each distinct set of source inputs once and
gather every target's declarations before choosing its providers and
planning its layout and calls. Calling policies run from the retained Terminal
code. Native compilation uses that shared program with each target's own
permissions and output, without reopening source.

## Reviewing a bootstrap environment assumption

Some early hardware preparation runs before the program publishes its interrupt
handlers. The caller may explicitly assume that no asynchronous NMI, machine
check or physical failure happens during that interval. This is the narrow
platform assumption in [bootstrap safety](../../../../wiki/spec/build/hardware_materialization.md#bootstrap-safety)
and [UEFI handoff](../../../../wiki/spec/build/uefi_entry.md#hardware-preparation-and-reporting).
It does not establish CPU configuration, masked ordinary interrupts, exclusive
storage, mappings, stack capacity or absence of software faults.

The normal project API shows the exact identities without compiling a separate
program. On an already accepted package, use the
[review example](../../build-system/orchestration/examples/installation_assumption_review.rs):

```sh
mbx run -p orchestration --example installation_assumption_review -- inspect path/main.omg uefi_x86_64 build/boot-review
cat build/boot-review/catalog.txt
```

This runs ordinary project discovery, the Build and Terminal Psi production.
The catalog names the selected entry, the advertised private entries and every
retained installation operation with the declaration that owns it.
`terminal.psi` is the actual canonical artifact. Choose the entry and
installation from this catalog, not from another compilation's numeric IDs. A
name is shown for review only; it never selects or authorizes the operation.

Write and review an environment description recording the exact
loader or emulator, CPU configuration, launch arguments, injection policy and
the assumptions accepted. Keep its bytes with the resulting image. QEMU can fix its CPU and delivery settings, including whether the harness
injects NMI before publication, but cannot rule out spontaneous machine checks
or physical failures; those stay explicit assumptions.
Then create and inspect the record, substituting the two reviewed numeric IDs:

```sh
mbx run -p orchestration --example installation_assumption_review -- accept-bootstrap build/boot-review/terminal.psi uefi_x86_64 ENTRY_ID INSTALLATION_ID environment.md 'reviewer and launch owner' build/bootstrap-assumptions.bin
mbx run -p orchestration --example installation_assumption_review -- show build/bootstrap-assumptions.bin
omega --target uefi_x86_64 --bootstrap-assumptions build/bootstrap-assumptions.bin --report-file build/boot-report.txt path/main.omg
```

`accept-bootstrap` or `accept-interrupt-delivery` hashes the actual description and keeps its source/caller provenance;
it does not observe hardware or prove the assumption. Native admission checks
the record against the Terminal identity, target, advertised entry, selected
call closure, installation and publication endpoint. A changed program needs a
new review. A missing record, wrong target, unrelated entry or wrong
installation refuses. A two-target request splits its supplied rows before
native admission; neither target uses the other's assumptions.

Embedders put bootstrap and installed-delivery records in
`InstallationEnvironmentAssumption`. Use `encode_installation_assumptions` and
`decode_installation_assumptions` from `products`, then supply the
rows through `NativeProductRequest::installation_assumptions` or
`CompileRequest::with_installation_assumptions`.
Saved native inputs keep the same rows. Recovery returns them apart from
package permissions, and the ordinary `NativeCompileRequest` consumes them.
The sealed installation scope keeps the requirement, replaying a private entry
checks the parent's exact row again, and the native artifact identity includes
it. `ObjectArtifact::installation_environment_assumptions` exposes the
conditions for later review. No target, and no missing permission policy,
supplies this assumption silently.

Passing these records in does not by itself give a complete interrupt floor.
Multiple-entry guards, asynchronous stack bounds, publication ordering and
fatal termination still need their own native and CPU evidence.

For a fatal exception floor, review a separate installed-delivery condition.
It applies while the named Scope is installed, not during bootstrap. The
environment description must name one execution context,
ordinary IRQ routing outside vectors 0–31, true NMI/MCE delivery, and absence of
guest debug injection, tracing and active hardware breakpoints. A host debugger
pause is not an exception injected into the guest. NMI/MCE are not assumed
absent; native stack composition and software-fault checks are separate
requirements.

```sh
mbx run -p orchestration --example installation_assumption_review -- accept-interrupt-delivery build/boot-review/terminal.psi uefi_x86_64 ENTRY_ID INSTALLATION_ID environment.md 'reviewer and launch owner' build/interrupt-delivery.bin
mbx run -p orchestration --example installation_assumption_review -- show build/interrupt-delivery.bin
omega --target uefi_x86_64 --bootstrap-assumptions build/bootstrap-assumptions.bin --interrupt-delivery-assumptions build/interrupt-delivery.bin --report-file build/boot-report.txt path/main.omg
```

The two records carry different condition tags, so neither can satisfy the
other's requirement. A request without these inputs gets no such assumptions,
and the compile report lists both conditions with their exact
program, target, entry, installation and reviewed-description identities.

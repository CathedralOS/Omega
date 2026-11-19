# Native compilation

The native driver takes a published Psi program, one target and a
`NativeCompileRequest`. The request supplies the entry, selected providers,
imports, component call inputs, completed scalar values and output policy.
The driver checks the scope and prepares the executable. A full image request
runs through image construction at stage 16. A runtime admission request can return
the provisions needed to interpret the program. [Native entry points](src/compile.rs)
· [Preparation](src/preparation.rs)

| Stages | What passes forward |
| --- | --- |
| 01–05 · Admission and target lowering | The compiler admits the published program, selects providers and lowers the reached bodies through abstract operations to target operations. Target lowering issues the instruction input, semantic obligations and closed private references together. [Target lowering](pipeline/04_abstract-operations-to-target-operations/src/lib.rs) |
| 06–11 · Selection through installation binding | Physical planning allocates registers, plans frames and generated entry code, and measures demanded installed handlers. Installation binding checks the exact operation, body identity, geometry and fault evidence. [Body sequence](src/pipeline.rs) · [Installation binding](pipeline/10_framed-machine-to-bound-framed-machine/src/lib.rs) |
| 12–16 · Encoding, layout and object construction | The driver encodes each body, places its blocks, relaxes eligible branches, places complete functions and constructs their objects. Application entries, demanded callbacks and separately admitted component exports use the common operations. [Body sequence](src/pipeline.rs) · [Callable exports](src/components.rs) |
| 17 · Image construction | The image owner takes the object publication, target image request and selected component inputs and builds the executable and its evidence. [Native artifact](pipeline/16_objects-to-image/native-artifact/src/lib.rs) |

The product coordinator completes the source or received artifact's scalar
inputs before native compilation. Target planning runs the selected calling
policy on the publication. Storage for an identical checked Psi program
can be shared across targets. Each target keeps its own entry, deployment
policy, authority and image. [Product coordinator](../compiler/src/product_execution.rs)
· [Executable preparation](../compiler/src/executable.rs)

File installation writes the finished products and returns their receipts. Independent validators can request retained originals to compare the
physical program, encoding, frames and image. [Product publication](../build/build-output/product-publication/src/lib.rs)
· [Verification](verification/) · [Full pipeline](../../../pipeline.md#connected-program-route)

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

# Psi to native code

The [entry point](psi_to_native.rs) takes a Psi program [authorized by the receiving host](../psi/authorization/authorization.rs), a target, and the receiving host's [reviewed providers and permissions](pipeline/00_psi-to-admitted-native-inputs/host-providers/installation.rs). An image request produces a finished native image. The [caller](../compiler.rs) packages that image. Runtime admission stops as soon as the interpreter's provisions are ready. Production callers enter through `emit` or `admit_published_runtime`. Integration tests enable the `test-support` feature to supply retained inputs, count stage invocations, and exercise the same foreign-adapter compiler that runtime admission uses for a supplied ABI request.

## Follow a compilation

1. [Complete scalar inputs](pipeline/00_psi-to-admitted-native-inputs/scalar-applications/scalar_applications.rs) for this deployment. Build also consumes this operation when it needs the evaluation receipts. Its crate exposes evaluated values and receipts and has no dependency on the native driver.
2. [Stage 00](pipeline/00_psi-to-admitted-native-inputs/admit.rs) matches the [publisher’s provider demand](pipeline/00_psi-to-admitted-native-inputs/provider-demand/demand.rs) to the exact authorized artifact. [Stage 01](pipeline/01_admitted-native-inputs-to-target-bound-program/src/provider_selection.rs) consumes that admitted pair and computes scalar entry reach for the selected target. The [driver](pipeline/mod.rs) prepares the retained program and runs the numbered lowering stages. [Preparation](pipeline/prepare.rs) owns the checks on the retained publication, private entry memory and supplied scalar values. Stage 01 also binds the selected declaration to the target’s physical entry and storage contracts. The resulting [entry contract](pipeline/01_admitted-native-inputs-to-target-bound-program/program-entry-contracts/program_entry.rs) carries the published signature and source commitment through admission. Stage 09 selects the hosted stack extent and stage 16 places it at the final BSS origin.
3. [Realization](pipeline/realization.rs) drives the common body sequence for the application, callbacks, and component exports. Follow [one physical body](pipeline/realization/physical_body.rs) from instruction selection through object construction. Generated foreign adapters have their own representation at each applicable stage. [Image emission](pipeline/16_objects-to-image/image-emission/) consumes the finished objects.
4. [Replay](replay/README.md) consumes retained originals on an explicit validation request. [Frame and encoding checks](pipeline/representations/machine-code/) validate the corresponding machine-code forms. [Admission](verification/README.md) also has callers during ordinary compilation and execution.

The ELF, Mach-O and PE writer tests start from [fixed encoded imports](pipeline/16_objects-to-image/image/src/test_support.rs). The samples include the instruction bytes, each relocation field, register effects and stack use the writer receives.

Production stage consumers are the native entry, packages within the same stage, and the next numbered stage. Other readers have exact package-to-package permissions in the [Cargo boundary check](../../../../tools/xtask/src/boundary/native_stages.rs). For example, file publication reads the finished artifact and its companions. Published-object replay reopens the stages that issued the evidence it verifies.

## Placement during the repair

| Code | Owner |
| --- | --- |
| A transformation of one issued program form | Its numbered directory under `pipeline/`. Facts needed by the next stage belong to the output of their producer. |
| Code that calls several numbered stages | The private driver under `pipeline/`. Root bodies, callback bodies and component exports use its shared sequences. |
| Program records consumed across stages | The relevant form under `pipeline/representations/`. A representation can describe its data and validate its invariants. It cannot drive the next transformation. |
| Target descriptions | [Targets](targets/) owns target data, calling conventions and register descriptions. Target facts can be queried without constructing a program. |
| Existing code with no active caller or unresolved ownership | `_recycling/` retains source for comparison or a rewrite. Cargo paths and Rust module declarations in active packages must resolve outside it. |

The [driver](pipeline/mod.rs) admits the selected providers and entry once. Runtime preparation receives the admitted result, the target and optimization choices. Image preparation also receives the callable plan and image request. The [representation plan](../../../../representation-pipeline.md) describes the isolation check for each adjacent pair.

## Reviewing a bootstrap environment assumption

Some early hardware preparation runs before the program publishes its interrupt
handlers. The caller may explicitly assume that no asynchronous NMI, machine
check or physical failure happens during that interval. This is the narrow
platform assumption in [bootstrap safety](../../../../wiki/spec/build/hardware_materialization.md#bootstrap-safety)
and [UEFI handoff](../../../../wiki/spec/build/uefi_entry.md#hardware-preparation-and-reporting).
It does not establish CPU configuration, masked ordinary interrupts, exclusive
storage, mappings, stack capacity or absence of software faults.

The V1 project API showed the exact identities without compiling a separate
program. On an already accepted package, use the
V1 review example (archived with its orchestration crate):

```sh
mbx run -p orchestration --example installation_assumption_review -- inspect path/main.omg uefi_x86_64 build/boot-review
cat build/boot-review/catalog.txt
```

In a build of that revision, this runs ordinary project discovery, the Build and Terminal Psi production.
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

The [installation conditions](verification/installation-admission/conditions/src/review.rs) library
encodes and decodes reviewed bootstrap and interrupt-delivery records with
`encode_installation_assumptions` and `decode_installation_assumptions`.
The CLI passes these rows to source builds through the corresponding assumption file options.
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

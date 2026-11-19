# Psi to native code

The native entry accepts an authorized Psi program, its deployment target and the receiving host's providers and permissions. Source compilation passes its checked publication and target demand in memory. A saved-program request reopens a portable `.psi` file and authorizes it before entering the same driver. [Native entry](psi_to_native.rs) · [Source and saved inputs](target_inputs/published_application.rs) · [Psi authorization](../psi/authorization/authorization.rs)

## Follow a compilation

| Work | Owner |
| --- | --- |
| Admit the exact program, entry and provider demand | [Stage 00](pipeline/00_psi-to-admitted-native-inputs/admit.rs) receives the program and the [provider installation](pipeline/00_psi-to-admitted-native-inputs/host-providers/installation.rs). [Stage 01](pipeline/01_admitted-native-inputs-to-target-bound-program/src/native_execution.rs) binds the selected entry, calling contracts and reachable bodies. |
| Prepare the target inputs | [Target preparation](target_inputs.rs) checks the retained publication, private entry memory and scalar inputs. Build can also request [admitted scalar values](pipeline/00_psi-to-admitted-native-inputs/scalar-applications/scalar_applications.rs). |
| Lower one selected body | [Stage 02](pipeline/02_target-bound-program-to-abstract-operations/lower.rs) lowers its checked Psi to abstract operations. The [body driver](compile_body.rs) coordinates stages 03–15 for the entry, private callables and component exports, from optimization through object construction. |
| Complete the requested product | The [program driver](driver.rs) returns the interpreter’s provisions at runtime admission. An image request [collects the required bodies](emit_program.rs) and [stage 16](pipeline/16_objects-to-image/) publishes the finished image. [Entry signature and storage contract](representations/program-entry-contracts/program_entry.rs) · [Target arrival ABI](targets/calling-conventions/src/process_entry.rs) |
| Verify an emitted object | An [explicit stage validator](verification/stage_validators.rs) requests originals from the emission and gives them to [published-object verification](verification/published-objects/). Its checks compare the root, callable and component objects against their own retained stage inputs before the caller releases the image. |

The public standalone program file is the portable `.psi` artifact. The historical `OMGNTI41` record captured a native realization proposal for a particular artifact and target. Its reader and writer are retained in [the native derivation archive](_recycling/native-derivation-record/). [Portable Psi encoding](../../../../wiki/spec/terminal-psi/encoding.md)

The program driver schedules specialized work around each body. Component export joins, callback thunks and target ABI adapters have their own inputs. [Component exports](emit_program/components/exports.rs) · [Callback thunks](emit_program/callback_thunks.rs) · [Foreign adapters](foreign_adapters.rs) · [Interpreter provisions](runtime_provisions.rs)

## Where code belongs

| Code | Owner |
| --- | --- |
| Transformation of an issued program form | The numbered stage under [pipeline/](pipeline/). |
| Coordination of several transformations | The program and body drivers at this directory's root. |
| Program records exchanged by stages | The corresponding form under [representations/](representations/). |
| Target descriptions, calling conventions and registers | [targets/](targets/) |
| Independent checks of provider, installation or emitted object evidence | [verification/](verification/) |
| Implementations held for reference or migration | [_recycling/](_recycling/) |

ELF, Mach-O and PE writer tests use fixed encoded inputs with known instruction bytes, relocations, register effects and stack use. The Cargo boundary check records which production packages can read each stage. [Writer fixtures](pipeline/16_objects-to-image/image/src/test_support.rs) · [Stage dependency check](../../../../tools/xtask/src/boundary/native_stages.rs)

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
The ordinary `NativeCompileRequest` receives the reviewed rows for its deployment alongside the package permissions.
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

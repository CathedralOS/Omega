# Psi to native code

The native entry accepts an authorized Psi program, its deployment target and the receiving host's providers and permissions. Source compilation passes its checked publication and target demand in memory. A saved-program request reopens a portable `.psi` file and authorizes it before entering the same driver. [Native entry](psi_to_native.rs) · [Source and saved inputs](driver/inputs/published_application.rs) · [Psi authorization](../psi/artifact/authorization/authorization.rs)

## Follow a compilation

| Work | Owner |
| --- | --- |
| Admit the exact program, entry and provider demand | [Stage 00](pipeline/00_psi-to-admitted-native-inputs/src/lib.rs) receives the program and the [provider installation](pipeline/00_psi-to-admitted-native-inputs/host-providers/src/lib.rs). [Stage 01](pipeline/01_admitted-native-inputs-to-target-bound-program/src/native_execution.rs) binds the [published entry and its calling contracts](pipeline/01_admitted-native-inputs-to-target-bound-program/src/published_application.rs) to the selected target and reachable bodies. Both `--check` and execution check the assembly catalog target of every body the image needs. This includes the functions named by dynamic dispatch tables. Each populated table slot stores a relocated function address even if the entry never invokes the slot. Its [provider selection](pipeline/01_admitted-native-inputs-to-target-bound-program/src/native_execution/provider_bindings.rs) matches the reachable requirements to providers and checks competing external bindings for both programs and exports. |
| Prepare the target inputs | [Target preparation](driver/inputs/mod.rs) checks the retained publication, private entry memory and scalar inputs. Build and the interpreter use the same [scalar input checks](../psi/execution-inputs/scalar-applications/scalar_applications.rs). |
| Lower one selected body | [Stage 02](pipeline/02_target-bound-program-to-abstract-operations/src/lib.rs) lowers its checked Psi to abstract operations. The [body driver](driver/compile_body.rs) coordinates stages 04–15 for the entry and its private callables, from target lowering through object construction. |
| Complete the requested product | The [program driver](driver/mod.rs) returns the interpreter’s provisions at runtime admission. An image request [collects the required bodies](driver/image/mod.rs) and [stage 15](pipeline/15_objects-to-image/) publishes the finished image. [Entry signature and storage contract](representations/program-entry-contracts/program_entry.rs) · [Target arrival ABI](targets/calling-conventions/src/process_entry.rs) |
| Verify an emitted object | An [explicit stage validator](verification/stage_validators.rs) requests originals from the emission and gives them to [published-object verification](verification/published-objects/). Its checks compare the root and private callable objects against their retained stage inputs before the caller releases the image. |

The public standalone program file is the portable `.psi` artifact. The historical `OMGNTI41` record captured a native realization proposal for a particular artifact and target. Its reader and writer are retained in [the native derivation archive](../../../../_recycling/source-rust/subsystems/compiler/psi-to-native/native-derivation-record/). [Portable Psi encoding](../psi/artifact/wire/encoding.md)

The image driver plans reachable private callables and emits callback thunks. Runtime admission prepares target ABI adapters for foreign calls. [Callback thunks](driver/image/callback_thunks.rs) · [Foreign adapters](driver/runtime/foreign_adapters.rs) · [Interpreter provisions](driver/runtime/mod.rs)

## Where code belongs

| Code | Owner |
| --- | --- |
| Transformation of an issued program form | The numbered stage under [pipeline/](pipeline/). |
| Coordination of several transformations | The [program driver](driver/mod.rs) coordinates input admission, body compilation and the requested image or runtime product. |
| Program records exchanged by stages | The corresponding form under [representations/](representations/). |
| Target descriptions, calling conventions and registers | [targets/](targets/) |
| Independent checks of provider, installation or emitted object evidence | [verification/](verification/) |
| Implementations held for reference or migration | [Native compiler archive](../../../../_recycling/source-rust/subsystems/compiler/psi-to-native/) |

ELF, Mach-O and PE writer tests use fixed encoded inputs with known instruction bytes, relocations, register effects and stack use. The Cargo boundary check records which production packages can read each stage. [Writer fixtures](pipeline/15_objects-to-image/image/src/test_support.rs) · [Stage dependency check](../../../../tools/xtask/src/boundary/native_stages.rs)

## Stage map

Each numbered stage, the form it hands on, and what reads that form. Lines are
production Rust outside `tests/` directories, measured on `d75a30117`. Stage 06
changes nothing about the representation. The driver waits for every
body at two points: after stage 04, for the callee closure in
[`plan_callables.rs`](driver/plan_callables.rs), and between stage 09's two
phases, for handler measurement and entry planning in
[`object_emission.rs`](driver/image/object_emission.rs).

Stage 09 runs in two phases because one body's installed-handler stack bound
depends on other bodies. First it frames each body on its own. The driver then
measures every retained installed handler across the whole body roster. Last,
stage 09's [installation binding](pipeline/09_post-allocation-machine-to-framed-machine/entry-planning/src/installation_binding.rs)
checks each body's installation guards against those measurements and issues
the `BoundFunctionBody` and bound entry recipes that stage 11 encodes. Only that
module links the [binding issuer](representations/bound-framed-machine/issuer/issuer.rs),
so stage 11 cannot accept a body or entry that skipped the check.

Stages 12 and 13 share the work of laying out a function. Stage 12 fixes the
instruction order and records any selected branch relaxation. Stage 13 writes
instructions directly at their framed offsets and places calls and relocations
in the closed text section. An audit retains the original layout and unframed
rows so it can independently check the placed bytes. [Placement](pipeline/13_resolved-layout-to-placed-functions/src/lib.rs)

| Stage | Lines | Produces | Read by |
| --- | --- | --- | --- |
| 00 admitted inputs | 919 | `AdmittedProviderDemand` (checks the authorization matches the artifact) | 01 only |
| 01 target-bound program | 9,742 | `PublishedSelection`, `PreparedNativeExecution` | 02 and driver runtime, components, callables |
| 02 abstract operations | 6,418 | `VerifiedPsiOptimizationInput` | 04 and driver |
| 04 target operations | 33,812 | `NativeBodyPlan` | 05, driver closure, `TargetOperationCorrespondence` |
| 05 instruction selection | 47,046 | `StagedOptimizedSelectedInstructions` | 06 only |
| 06 selected rewrites | 26,880 | `SelectedAllocationInput` (same form plus liveness) | 07, `SelectedInstructionRewriteReplay` |
| 07 register allocation | 16,292 | `RetainedAllocation` | 08, `FixedFrameCorrespondence` |
| 08 post-allocation machine | 2,119 | `StagedOptimizedPostAllocationMachinePlan` | 09, `FixedFrameCorrespondence` |
| 09 frame and installation binding | 10,630 | `FramedNativeBody`, `PreparedFunctionBody`, then `BoundFunctionBody` | driver entry planning and handler measurement, then 11 |
| 11 encoding | 23,475 | `EncodedMachineProgram` | 12 and validator replay |
| 12 layout | 4,396 | `ResolvedFunctionLayout` | 13 |
| 13 function placement | 10,004 | `FunctionPlacement` | 14 only |
| 14 relocation-free object | 5,776 | `PublishedFunctionObject` | 15 |
| 15 objects and image | 51,561 | `FinishedNativeImage` | Build publication |

There is no stage 03 or stage 10. Stage 09 binds installations itself (above). Stage 04 receives the abstract program Stage 02 lowered,
unchanged, because a receiving native lowerer does not rerun Psi passes
([Terminal Psi product](../../../../wiki/spec/terminal-psi/product.md#optimization-ordering)). The
native abstract-operations optimizer that used to sit there never ran in the
shipped compiler, since post-Terminal selections refuse every Psi-phase
optimization. It is archived with its validators under
[`_recycling/.../pipeline/03_abstract-operations-to-abstract-operations/`](../../../../_recycling/source-rust/subsystems/compiler/psi-to-native/pipeline/03_abstract-operations-to-abstract-operations/).
Its global value numbering, control-flow cleanup and proof-check elision passes
are kept there to be ported as Psi-to-Psi passes.

## Optimization phases

The [optimization contract](../../../../wiki/spec/build/optimizations.md#exact-selection)
requires each optimization to have one phase owner. It leaves the list of
phases to this compiler. That list is `OptimizationExecutionPhase` in
[`optimization-selections`](../foundation/optimization-selections/src/lib.rs). The
table shows each phase's catalog entry count and the native stages that name the
phase, measured on `baa76c5b222e` (after archiving Stage 03), with the two
`SelectedLowering` entries added since.

| Phase | Catalog entries | Stages naming it |
| --- | --- | --- |
| `Psi` | 9 | none; runs before Terminal Psi publication |
| `TargetOperations` | 2 | 04 (rewrites applied in 05) |
| `SelectedLowering` | 25 | 06, 07, 12, 13 |
| `PreAllocation` | 10 | 06, 07 |
| `AllocationRecovery` | 2 | 06, 07, 13 |
| `PostAllocationMachine` | 9 | 08, 13 |
| `FunctionRelativeLayout` | 1 | 12, 13 |

Call inlining and vector promotion run only when a build selects them, as the
`SelectedLowering` entries `SelectedAcyclicCallInliningV1` and
`SelectedPackedVectorPromotionV1`. Stage 05 used to run both on every build,
which the contract forbids. Stage 06 now runs them before the pair-rule
literal folds, without starting those folds when no pair rule is selected
([`structural_rewrites.rs`](pipeline/06_selected-instructions-to-selected-instructions/src/selected_rewrites/structural_rewrites.rs)).
Their replay validator, `SelectedInstructionRewriteReplay`, re-executes each
step and checks each rewrite's postconditions. No part of either rewrite is
legalization. Every hosted target compiled the test fixtures and the
dependency-free samples with both turned off, and x86-64 runs every packed
operation in its memory form. Promotion needs 128-bit packed register forms,
which only x86-64 has, so a build that selects it for an AArch64 target is
refused by name.

## Plan identities

Stages 04–14 used to hash their in-memory plans with SHA-256 and hand the
hashes on. Because both sides run in the same compile, these hashes only let
the compiler check its own output, which default compilation does not do. Most
have been removed: records share the plans they were built from through an
`Arc`, and checks compare those plans (pointer first, then structure). A hash
stays only where an explicit validator, a published artifact, a catalog or a
foreign-package boundary reads it; the rows marked "kept for now" are the
in-process digests still left.

| Identity | Readers that matter | Decision |
| --- | --- | --- |
| `RelocationFreeObjectPlanIdentity`, the `OMGTRO` container and `OMGTOM` manifest (stage 14) | own receipts; container decoded only to re-check its own codec under a validator | deleted; structural object checks stay |
| `ResolvedSelectedFormLayoutIdentity`, `TargetFrameLayoutIdentity` (machine-code, 09–13) | receipts; validator compares baseline layout | deleted; carriers share the frame and layout plans and checks compare those plans and the baseline roots |
| Fragment, frame-application and text-section identities, their manifests and the realization manifest identity (machine-code, 13–14) | own receipts; object custody compared between publications | deleted; validators compare the retained text section and selections |
| `PostAllocationMachineIdentity`, `PostAllocationMachineOptimizationCustody` (08–13) | receipts; `FixedFrameCorrespondence` compares retained program | deleted; encodings, layouts, frame plans and exit contracts share the plan and checks compare it; the physical-program digest no longer hashes it |
| `FixedPrecolored*PlanIdentity`, register-homes receipts (06–07) | sibling receipts only | deleted |
| `SelectedInstructionPlanIdentity`, `RetainedPlanPrefix`, effects identities (05–06) | receipts; validators compare retained program | deleted; rewrite receipts, liveness, live-range, effects and recovery plans, the post-allocation manifest and machine plan share `Arc<SelectedInstructionPlan>` and checks compare it; records that already share the machine plan drop their own copy; runtime spill, split, rematerialization and fixed-occurrence ledger rows record only their kind |
| `LivenessIdentity`, `LiveRangeIdentity`, `AllocationLegalityIdentity` and the receipt and manifest digests built on them (06–08) | receipts; the post-allocation manifest digest | deleted, with their per-function digests and the carried-digest cache; legality, spill-choice, fixed-precolored and register-home plans name their program through the shared `selected` plan, and checks compare that plan; custody, completion and recovery receipts and the post-allocation manifest no longer record them, and the post-allocation machine plan drops its copies |
| `OptimizationIdentityBundleIdentity`, `OptimizedAbstractPlanProjectionIdentity`, `PrePhysicalOptimizationManifestIdentity` (04–15) | stage 05–07 custody, the post-allocation manifest, execution facts and the stage 15 physical scope, which only rehashed the unit, selections and budget | deleted; those records carry and compare the unit identity, selection identity and budget directly |
| Rewrite receipt identities (`CopyRemovalIdentity`, `LiteralFoldIdentity`, `FixedViewCopyIdentity`, `PressureRematerializationIdentity` and nine siblings), `SpillChoiceIdentity`, `RecoveryClassificationIdentity`, `SelectedLoweringOptimizationCompletionIdentity`, `PreAllocationOptimizationCompletionIdentity` (06–07) | sibling receipts, the transformation ledger and the post-allocation manifest; replay compares whole steps | deleted; ledger rows record each rewrite by kind; the optimization record and manifest name the program each Omega 04 phase ended with (`selected_lowering_output`, `pre_allocation_output`), and the replay validator compares that program with the run's output |
| `RegisterHomeIdentity`, `PostAllocationOptimizationManifestIdentity`, `AllocatedCalleeSavedRequirementIdentity`, the allocation publication digest (07–13) | custody receipts; machine plan, preservation, frame demands, exit contract and `published-objects` root checks | deleted; the manifest, machine plan, preservation requirements and callee-save storage share `Arc<RegisterHomePlan>` and `Arc<PostAllocationOptimizationManifest>`, and checks compare them; records that already share the machine plan drop their own copies |
| `NonAuthoritativeCalleeSaveStorageIdentity`, `TargetFrameProtocolEncodingIdentity` (09–13) | frame layout and frame-application roots; exit contract frame disposition | deleted; the frame layout shares the requirements and storage plans, the exit contract shares the protocol plan, and frame application is checked against the protocol its source carries |
| `SelectedFormEncodingIdentity`, `WholeFunctionExitContractIdentity` (11–13) | layout, relaxation record, exit requirements and encoded-program context; the contract's own recheck | deleted; those records hold a weak `SelectedFormEncodingHandle` and checks compare it so ordinary placement still releases the encoding before object construction; the footprint interning bucket key stays as a storage key |
| Physical-program digest (`PostAllocationMachineProgram::identity`, 08–13) | stage 09 call origins, installed handlers and body CPU facts; bound encoding subject | kept for now; it no longer hashes the preservation requirements or the allocation publication, which records share and compare |
| `X86BranchRelaxationIdentity`, `X86BranchRelaxationRevisionIdentity` (12–13) | relaxation replay; exit-contract layout custody | kept for now as replay checkpoints over the function rows; they no longer hash the pre-layout encoding |
| `OptimizationUnitIdentity` (04) | the unoptimized program record, custody, the post-allocation manifest, execution facts, fact keys | bind the Psi fingerprint, work schedule, entry, target, admitted scalar values, selected machines and boundaries, provider candidates, target geometry, sequence bounds, module attachment and accepted facts. Stage 02 lowers each selected body in Psi order. The seed API also accepts independently constructed functions. It hashes their complete operands once and retains the digest with the read-only graph, so attaching proof facts reuses the same function identity |
| `MachineFunctionIdentity` | sort key in validator replay; program fingerprint plus placement, not a plan hash | keep |
| `MachineEffectCatalogIdentity`, `RegisterConstraintCatalogIdentity`, `TargetRegisterEnvironmentIdentity`, `PhysicalRegisterModelIdentity`, `AllocatorAvailabilityIdentity`, `OptimizationSelectionIdentity` | catalog and target configuration roots compared across stages | keep; they name configuration, not a stage's output |
| `NativeArtifactIdentity` | `native.identity.txt` (`omega run --keep`) | keep; binds emitted bytes, no longer plan hashes |

`NativeArtifactIdentity` already hashes the object, image and text bytes. The
plan hashes it also chained in changed nothing those bytes did not, and
[native emission](../../../../wiki/spec/build/machine_state_evidence.md#native-emission-and-backend-testing)
says artifact identity must not depend on a digest recording a pass. Removing
them changes the identity's value, not the emitted image.

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

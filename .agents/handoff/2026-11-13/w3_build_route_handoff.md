# worker-3 handoff: Build programs on the lowered-form route (2026-11-12/13)

## Result

Nothing landed, no PR opened, no branch pushed. The minimal Build program is not a few
missing constructs away from the lowered-form route: it needs five capabilities the route
does not have, and the first two sit in files an active open PR (#2765) is rewriting. What
this session produced is the measured target: the exact refusal, the exact Terminal module
the old path produces for the minimal Build program, and the hand-off design that module
implies. #2796 stays blocked on all of it.

## Measured (worker-3 slot, main 587f1f7265a, `target\debug\omega.exe`, dev opt-level 1)

Probe: a temporary env-guarded call in `prepare_checked_build`
(`build-settings/src/admission/checking.rs`) that runs
`lowered_discharge::judge_compilation(&program, Compilation { entries: &[entry], subject: None,
selected: &[], bindings: &[] })` on the settled Build slice, and a dump of
`produced.view().module()` in `PreparedBuildExecution::prepare`. The probe is saved as
`w3b_build_route_probe.patch` (same directory, applies to 587f1f7265a; adds
`lowered-discharge` and `structure-elaboration` to build-settings' Cargo.toml). Run with
`OMEGA_W3_BUILD_ROUTE=1` and `OMEGA_W3_BUILD_DUMP=<file>`; use a copy of the fixture with the
application name changed, otherwise a completed pure Build is reused and `prepare` is not
reached.

Subject: `tests/omega/pass/arithmetic/anonymous_integer_arith/build.omg`
(`machine build(builder: &mut Build) { Build::application(builder, "..."); builder.freestanding = true; }`).

1. Route verdict on the Build slice:
   `FallBack(Refusal { reason: Unsupported, machine: MachineId(1), detail: "type generic data" })`.
   That is `Build<'storage>`'s lifetime parameter (`record_shape`,
   `structure-elaboration/src/read/places.rs`). Elaboration stops at the first refusal, so
   the rest of this list is read from the source and from the dump, not run.
2. A portable publication of the Build program is impossible on the old path too:
   `TerminalPublication::Portable` fails with `checked machine identity: declaration `build`
   has non-hermetic source origin `User``. The Build runs as invocation-scoped code
   (`terminal_codec::InvocationTerminalCode::new`, which requires
   `module.identities.invocation_scope()`).
3. The module the old path produces for this two-statement program (full debug dump
   `w3b_dump1.txt`, 6,256 lines; compact views `w3b_build_machines_compact.txt`,
   `w3b_build_boundaries_compact.txt`, `w3b_types.txt`):
   - 2 machines. `build`: structural parameter `Build` (`MutableBorrow`);
     `EstablishByteSequenceLiteral` into a `ByteSequenceLiteral` place;
     `StructuralLeafCopy` to an `OperationResult` place; `Jump` passing it `SharedBorrow` into
     a block parameter of the unqualified view type; `CallUnit` with two structural
     arguments; `BooleanConstant`; `StructuralScalarFieldStore`; `ReturnUnit`.
     `published_service_ceiling: [ServiceId 1]`.
     `Build::application`: `declared_service_reach: [ServiceId 1]`, the same leaf copy and
     jump, then `BoundaryCall { boundary: BoundaryMachineId 2, structural_arguments: [Build
     SharedBorrow, view SharedBorrow] }`.
   - 19 structural types: `Build` (non-copy record, `no_code_affine_cleanup: false`), five
     empty records (`Record { fields: [] }`, non-copy), sums with and without payload,
     `FixedArray { element: u8, length: 55 }`, and three byte-view types
     (`ByteSequence(BorrowedView { access: Some(SharedBorrow) })` copyable, the same with
     `access: None`, and one with `descriptor_cleanup: false`). `Build.identifier` is
     `field_type: ByteSequence(BorrowedView { access: Some(SharedBorrow) })`.
   - `services: [ServiceDeclaration 1]`, `root_service_reach.concrete: [1]`.
   - 7 `boundary_machines` (all of the prelude's boundary traits the slice keeps), each a
     full `BoundaryMachineDeclaration` with a `CheckedBoundaryCallable`, `owning_service`,
     structural parameter declarations.
   - Also filled: `calling_abi_signatures` (about 590 dump lines), `native_layouts` (about
     590), `scalar_applications` (about 2,200), `unoptimized_behavior`. Which of these the
     interpreter needs for a Build activation was not measured.

## What the Build program needs from the route, in dependency order

1. Data with lifetime parameters (`type generic data`). `read/places.rs`.
2. Data without `[copy]` and data with no fields behind a `&mut` formal (`type data with
   custody`, `type data without fields`). This is the lane of open draft PR #2765
   ("psi: carry plain affine moves and normal-edge cleanup", updated 2026-11-13, another
   session), which rewrites `read/places.rs`, `body.rs`, `lower.rs`, `lower/places.rs`,
   `lib.rs`, `lowered-discharge/src/{places.rs,verdict.rs}`. Do not start 1 or 2 until it
   lands or is closed; its body says "stored references" and "borrowed extraction" stay
   unsupported, so Build still needs work after it.
3. Byte views: a `&[u8]` formal, field and string-literal argument (`type slice`, `type
   reference`), and a fixed-array field. The route's `ValueType` is `Scalar | Record |
   Rigid` (`body.rs`); this adds a kind to both crates (about 30 match sites name
   `ValueType::Rigid` outside tests).
4. Service reach and boundary calls: `machine service reach` and `machine invokes`
   (`read/machine.rs`, `machine_shape`), the module's `services` and `root_service_reach`,
   `declared_service_reach`/`published_service_ceiling` per machine, the
   `boundary_machines` catalog and `OperationKind::BoundaryCall`. The route has no boundary
   call today (`grep BoundaryCall structure-elaboration/src` finds nothing).
5. Invocation-scoped publication. `AcceptedClosure::publish`
   (`lowered-discharge/src/verdict.rs`) builds a portable `CanonicalTerminalArtifact` and
   refuses a machine with no declaration identity (`PublicationError::Undeclared`); a Build
   machine in user source has none. It needs a second publication that yields
   `InvocationTerminalCode` with the invocation scope in the identity table
   (see how Psi 06 sets it: `06_checked-trees-to-lowered-psi/src/machine_lowering.rs`,
   `attached_unit.rs`; identity side in `terminal-psi/src/identity.rs`).
6. The Build companions. `TerminalBuildHost` reads `TerminalBuildBindings`,
   `TerminalBuildRequests` and `TerminalBuildFilesystem` from `ProducedTerminalProgram`
   (`psi-compiler/terminal-production/src/build_bindings.rs`). Each is a list of
   `(OperationId, BoundaryMachineId, action, request)` built by walking Psi 06's
   `source_call_occurrences` (typed call node -> Terminal operation) and reading
   `expression_table.build_selection_request(call.build_selection)` and the prelude trait
   signatures by symbol. The route already keeps the same correspondence
   (`Elaboration::sources`, `MachineSources::operations`), and the request tables are in the
   typed trees, so the capture can be rewritten over `Elaboration` + `TypedTrees` in
   `terminal-production` (a crate #2796 keeps). `ProducedTerminalProgram` already has both
   custody forms (`TerminalProgramProduct::{Portable, Invocation}`); it needs one public
   constructor from route output.
7. The wiring: `PreparedBuildExecution::prepare` takes route output when the route accepts
   the Build slice, else the old path. Admission
   (`prepare_checked_build`) must hand the typed slice (`SettledBuildSlice::program`) or the
   accepted closure forward; today `AdmittedBuildProgram.prepared` carries only checked
   trees (`PreparedBuildMachineProgram::from_checked`). Note that the slice is produced by
   `validation::execution_projection::project_build_program` and provider plans by
   `BuildProviderPlans::capture(&checked, ..)`, both old-checker crates: admission itself is
   a Psi 05/validation customer beyond the one `CheckRequest::check()` call.

Steps 5 to 7 cannot be tested end to end before 1 to 4, so they were not written: they
would be code nothing reaches.

## What still stops #2796 from merging

Everything above. No Build program reaches the route's acceptance, so Build preparation
still needs `TerminalProductionRequest::produce`.

## Slot

`C:\Projects\Omega\.agents\worktree\worker-3`: clean, detached at origin/main
587f1f7265a, nothing running. `target\debug\omega.exe` there was last built WITH the probe
patch applied; rebuild before trusting it for anything else.

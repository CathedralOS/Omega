# bump_allocator_canary hosted-entry wall — `ProgramEntryVisibleParameters::None`

Lane context: `leaf/bump-allocator-canary` (parked). Queued wall behind the
`UnrecognizedInductiveBody` open-obligation wall filed in
`tools/bump_allocator_canary_open_obligations.md`. This note is the mechanism
trace for: *fixture exercise machines take `whole: Extent in Granted & Vacant`
parameters that hosted `main` can never supply.*

Verdict: the visibility computation is correct; there is nothing to fix in it.
`VisibleParameters::None` for hosted targets is a load-bearing contract
invariant, and no hosted-ingress route exists that could surface the extents
the fixture needs. The gap is upstream of parameter admission: no application-
visible source of `Extent in Granted` exists on hosted targets at all.

## Where hosted parameter visibility is computed

1. **Slot declaration** —
   `omega-rust/omega/backend/target/src/target_profile.rs`,
   `TargetProfile::program_entry_slot()` (~lines 487-592). Every hosted target
   (LinuxX64, LinuxArm64, MacosArm64, MacosX64, WindowsX64, and the catch-all
   covering `CrossPlatformCli`/`LocalUnchecked` — i.e. every schema `omega run`
   can select) declares `ProgramEntrySchema::HostedApplication` with
   `ProgramEntryVisibleParameters::None`. The per-target comments are explicit:
   the physical bridge "provision[s] the two internal roots ... never hosted
   arguments."

2. **Slot/schema invariant** —
   `omega-rust/omega/backend/plans/program-entry-plan/src/selected_entry.rs:32-47`.
   `HostedApplication ⇒ VisibleParameters::None` is enforced as a checked
   invariant: "a hosted `HostedApplication` keeps them as internal bridge
   inputs and exposes no source-visible parameters. Any other shape pairing is
   drift, not a third contract." Editing the enum value in the slot
   declaration is rejected here — there is no third pairing to select.

3. **Entry admission** —
   `omega-rust/omega/compiler/build-evaluation/src/admission/selection.rs:264-288`.
   Non-`self` parameters become `visible`; under `None` any visible parameter
   produces
   `hosted ProgramEntry exposes no arrival parameters, but <machine> declares ...`.
   Only a provisioned `&mut self` receiver is permitted (and even that is
   rejected when the attached data's all-zero image is invalid —
   `selection.rs:250-262`).

4. **Root production** — hosted arrival is
   `LinuxX86_64PhysicalEntry::enter(initial_stack: addr) -> i32` (exact plan:
   `program-entry-plan/src/program_entry_physical/exact_linux_x86_64.rs`) —
   one `addr` in rsp, `exit_group` status in edi. The toolchain bridge
   adapter-maps it into the semantic continuation
   `ProgramStorageEntry::enter(image: Extent in Granted, initial_storage:
   Extent in Granted)`; both Extents stay in toolchain custody. The
   `*Application` boundary traits (`std/targets/linux_x86_64/entry.omg:310`)
   bundle `ProgramStorageEntry` so the bridge — not application code — owns
   the semantic crossing.

## The precise mechanism gap

The enum has only two variants (`None`, `ImageAndInitialStorage`) and
`ImageAndInitialStorage` is bound to `ProgramStorageApplication` (freestanding;
`source_signature/mod.rs:380-495` checks both visible params against
`ProgramStorageEntry::enter`'s exact identities). So under `omega run` — which
always executes a hosted schema — an entry machine can never declare a visible
parameter of any type, `Extent` included.

That alone would only push the canary to source its extents inside `main`.
The deeper gap: **no application-reachable route produces an
`Extent in Granted` on hosted targets.** The three `Granted` establishers
(`extent.omg:42-47`) are: `ProgramStorageEntry::enter` (bridge-internal),
`DeviceLoanProvider::complete` (consumes a `DeviceLoan` from
`lend_for_*`, which already requires `Extent in Granted`), and
`ExtentRootProvider::grant(root: Extent)` — callable, but its input `Extent`
carries base/length geometry the app must already possess, and no hosted
surface names real storage (the physical entry's `initial_stack: addr` is
bridge-internal; authored `addr` literals like `let a: addr = 4096` are
forgeable geometry with no backing claim).

## What would unblock

Either of:

- A hosted-visible handoff for the bridge-provisioned roots — e.g. a
  provider requirement the toolchain bridge realizes (surfacing
  `initial_storage` or a carved sub-region to the application), which is a
  target/bridge design decision, not an admission-rule change; or
- A freestanding `ProgramStorageApplication` run target for the fixture
  (`UefiX64` is the only declared one today — needs firmware execution
  support under `omega run`, which does not exist on hosted runners).

Neither is a fixture-side fix; the canary's exercise machines are correctly
shaped for a program-storage entry that hosted launch does not offer.

Claims: BUMP-ALLOCATOR-CANARY (zergling-z6) covers `omega-rust/psi/target`,
`07_lowered-psi-to-terminal-psi`, `source/library/core/extent.omg`; this filing
touches only `tools/` documentation.

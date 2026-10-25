# Optional reach ceilings through checked calls

A checked call publishes its complete Mark reach, so an empty ceiling cannot hide it.

Every place copies that sentence. Its runtime oracle performs the same comparison: the value
carried from that call equals 7, `done()` exits 0, and any other value takes
`bad()` and crashes `Trap`. Only the transport of the returned value changes.
The provider `MarkerMark` returns 7. The requirement `Mark::read` publishes
`reaches Mark`. The calling machine writes no `reaches` clause.

Owning task: `OPTIONAL-REACH-CEILINGS` (`TASKS.md`). This row establishes one
requirement of that item: one unannotated direct boundary call runs, and an
empty ceiling rejects that hidden direct service. It does not close the item.

| Place | Positive project | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | the call result is a local in the entry, same shape as local |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the call result is a local |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the call result is stored in a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the call result arrives as a parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call returns the boundary result |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms return the boundary result |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the comparison uses the named constant the provider returns |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the call result |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | another machine returns the boundary result to the checker |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of the four hosted target bodies performs the call |

## Execution

Every positive place and root `main.omg` binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, and
declares `Expected exit: 0`. On Linux x86_64, `omega run --both` agreed:
interpreter exit 0 and native exit 0. Windows, macOS, and QEMU execution were
not run on this host.

```sh
omega run --both samples/cli/facts/unannotated_direct_boundary_call/main.omg
omega run --both samples/cli/facts/unannotated_direct_boundary_call/local/main.omg
omega run --both samples/cli/facts/unannotated_direct_boundary_call/field/main.omg
omega run --both samples/cli/facts/unannotated_direct_boundary_call/parameter/main.omg
omega run --both samples/cli/facts/unannotated_direct_boundary_call/call_result/main.omg
omega run --both samples/cli/facts/unannotated_direct_boundary_call/match_join/main.omg
omega run --both samples/cli/facts/unannotated_direct_boundary_call/const_name/main.omg
omega run --both samples/cli/facts/unannotated_direct_boundary_call/other_state/main.omg
omega run --both samples/cli/facts/unannotated_direct_boundary_call/other_machine/main.omg
omega run --both samples/cli/facts/unannotated_direct_boundary_call/target_body/main.omg
python3 tools/sample_native_gate.py --facts --filter cli/facts/unannotated_direct_boundary_call
```

The oracle discriminates. A provider result other than 7 takes `bad()` and
crashes `Trap`. The invalid twins are not that corruption: each inserts only
a bare `reaches` on the machine that directly calls `Mark::read`.

## Reject twins

Each `invalid/sample.json` is `intended-invalid` and requires
`reaches undeclared service \`Mark\``. The twin keeps the call, the provider,
the comparison, and the entry. Checking rejects because a written `reaches`
clause, including a memberless one, is a ceiling, and `Mark` does not fit the
empty ceiling.

| Place | Twin | Pinned diagnostic fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `reaches undeclared service \`Mark\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `reaches undeclared service \`Mark\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `reaches undeclared service \`Mark\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `reaches undeclared service \`Mark\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `reaches undeclared service \`Mark\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `reaches undeclared service \`Mark\`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `reaches undeclared service \`Mark\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `reaches undeclared service \`Mark\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `reaches undeclared service \`Mark\`` |

Before the checker change, the positive local program and its empty-ceiling
twin both stopped in source checking with
`machine \`main\` publishes service reach \`<none>\` but its checked body reaches undeclared service \`Mark\``.
That baseline was recorded against the opt-level 1 compiler at
`c2e06b875ba2f6a205dd49b408d937536833b190`. Omission was treated as the empty
ceiling. A private checked body with no authored `reaches` clause now keeps
the derived row, including the direct boundary contribution. A present clause
still requires that direct contribution to fit the authored row.

## Pinned fixtures

`tests/omega/pass/facts/unannotated_direct_boundary_call_through_<place>_exit`
copies each positive place, including `build.omg`, and records native exit 0.
`tests/omega/fail/facts/unannotated_direct_boundary_call_through_<place>_rejected`
copies each reject twin's `main.omg` and the diagnostic fragment. Fail
fixtures have no `build.omg`.

## Requirements and evidence

1. **Establishment.** `Mark::read` is a boundary requirement whose authored
   ceiling is `reaches Mark`. The unique covering provider `MarkerMark`
   returns 7. The calling machine omits `reaches`, so the derived row is the
   published reach for that private checked body. The exit test compares the
   returned value with 7; it does not substitute for the reach rule. The
   invalid twin writes the empty ceiling on that same caller, so the direct
   service `Mark` is undeclared.
2. **Identity and lifetime.** The fact is the value returned by that one
   direct call. Locals, the `Hold.value` field, parameters, call results,
   match joins, the named constant `MARK_VALUE`, the successor machine, the
   forwarding machine, and the target bodies carry that value without an
   intervening write of a different value.
3. **Composition.** Each positive and its twin differ only by inserting bare
   `reaches` on the machine that directly calls `Mark::read`. Joins and calls
   substitute that same returned value. The comparison `== 7` (or
   `== MARK_VALUE`, which is 7) is the consuming operation in every place.
4. **Soundness boundaries.** Proof, ownership, and termination are unchanged.
   The reach axis is the contract under test. An omitted clause infers; an
   explicit empty clause does not admit `Mark`. Service reach stays separate
   from `invokes`, suspension, blocking, and crashes. Public omission still
   publishes an empty synchronous-invocation ceiling and is outside this row.
5. **Checked-to-Terminal Psi (`Psi 05–07`).** The direct boundary call, the
   provider body, and the comparison lower into Terminal Psi and execute in
   the interpreter. The empty-ceiling twins are rejected in checking and do
   not produce a program.
6. **Provider settlement and ABI (`Omega 00–02`).** Each positive binds
   `ProgramEntry` for all four hosted targets. `MarkerMark` is the unique
   covering candidate for `Mark`. The returned `u64` uses the ordinary
   integer return lane.
7. **Instruction selection (`Omega 03–06`).** The comparison and the exit
   path are ordinary integer compare and branch. Linux x86_64 native exit
   was 0 for root and all nine places.
8. **Emission (`Omega 07–09`).** Linux x86_64 images executed and agreed with
   Terminal Psi (`omega run --both`). Windows, macOS, and QEMU images were
   not executed here.

### Demonstrated

One unannotated direct call of `Mark::read` returns 7 and exits 0 through all
nine places and the root, on Linux x86_64, with Terminal Psi and native
agreement. Each empty-ceiling twin rejects with
`reaches undeclared service \`Mark\``.

### Still open on OPTIONAL-REACH-CEILINGS

Unannotated transitive calls. A pinned service-free empty ceiling, and
rejection of a hidden transitive service. A present clause as a bound on the
complete derived row rather than an additive union. `A + B` accepts `A` and
rejects `C`. Injected, generic, dynamic, and target-specific refinement.
Public bodies that omit `reaches`. Boundary requirements that omit their own
ceiling. Audit slack and package-review round trips.

## Complete derived reach (current repair)

**Fact sentence:** A checked call publishes its complete Mark reach, so an empty
ceiling cannot hide it. The consuming operation is the caller's authored empty
reach ceiling. The runtime oracle is the same selected read returning 7, followed
by native and Terminal exit 0. `Mark` is an exact boundary declaration; no caller
value guard establishes or changes a service row.

`OPTIONAL-REACH-CEILINGS` owns this evidence. The existing direct production stays
in place. `transitive/` carries the same call through a public wrapper with an
independent `invokes Mark;` contract and a private helper. `generic/` retains an
exact nominal machine requirement and substitutes the selected callable's
published contract. `dynamic/` retains the selector slot's pinned requirement,
without replacing it by a selected implementation's smaller body. The
`target_body` place also carries the call through the portable target family.
These are productions of the same reach summary, not new fact folders.

Each positive binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`,
`macos_arm64`, and `linux_arm64`, and declares `Expected exit: 0`. Each twin
changes only the caller's omitted `reaches` to an explicitly empty `reaches`.
The twin must reject `reaches undeclared service \`Mark\``. Its scalar comparison
is a runtime oracle, not replacement evidence for reach.

| Place | Transitive public/private | Nominal generic | Dynamic selector |
| --- | --- | --- | --- |
| local | [positive](transitive/local/main.omg) / [reject](transitive/local/invalid/main.omg) | [positive](generic/local/main.omg) / [reject](generic/local/invalid/main.omg) | [positive](dynamic/local/main.omg) / [reject](dynamic/local/invalid/main.omg) |
| field | [positive](transitive/field/main.omg) / [reject](transitive/field/invalid/main.omg) | [positive](generic/field/main.omg) / [reject](generic/field/invalid/main.omg) | [positive](dynamic/field/main.omg) / [reject](dynamic/field/invalid/main.omg) |
| parameter | [positive](transitive/parameter/main.omg) / [reject](transitive/parameter/invalid/main.omg) | [positive](generic/parameter/main.omg) / [reject](generic/parameter/invalid/main.omg) | [positive](dynamic/parameter/main.omg) / [reject](dynamic/parameter/invalid/main.omg) |
| call_result | [positive](transitive/call_result/main.omg) / [reject](transitive/call_result/invalid/main.omg) | [positive](generic/call_result/main.omg) / [reject](generic/call_result/invalid/main.omg) | [positive](dynamic/call_result/main.omg) / [reject](dynamic/call_result/invalid/main.omg) |
| match_join | [positive](transitive/match_join/main.omg) / [reject](transitive/match_join/invalid/main.omg) | [positive](generic/match_join/main.omg) / [reject](generic/match_join/invalid/main.omg) | [positive](dynamic/match_join/main.omg) / [reject](dynamic/match_join/invalid/main.omg) |
| const_name | [positive](transitive/const_name/main.omg) / [reject](transitive/const_name/invalid/main.omg) | [positive](generic/const_name/main.omg) / [reject](generic/const_name/invalid/main.omg) | [positive](dynamic/const_name/main.omg) / [reject](dynamic/const_name/invalid/main.omg) |
| other_state | [positive](transitive/other_state/main.omg) / [reject](transitive/other_state/invalid/main.omg) | [positive](generic/other_state/main.omg) / [reject](generic/other_state/invalid/main.omg) | [positive](dynamic/other_state/main.omg) / [reject](dynamic/other_state/invalid/main.omg) |
| other_machine | [positive](transitive/other_machine/main.omg) / [reject](transitive/other_machine/invalid/main.omg) | [positive](generic/other_machine/main.omg) / [reject](generic/other_machine/invalid/main.omg) | [positive](dynamic/other_machine/main.omg) / [reject](dynamic/other_machine/invalid/main.omg) |
| target_body | [positive](transitive/target_body/main.omg) / [reject](transitive/target_body/invalid/main.omg) | [positive](generic/target_body/main.omg) / [reject](generic/target_body/invalid/main.omg) | [positive](dynamic/target_body/main.omg) / [reject](dynamic/target_body/invalid/main.omg) |

The corresponding `unannotated_{transitive,generic,dynamic}_boundary_call_through_*`
fixtures under `tests/omega/{pass,fail}/facts` are byte copies of these sources.
`contracts/empty` pins service-free local mutation under an empty ceiling.
`contracts/declared_slack` pins `Mark + Unused` as a real conservative contract:
its body reaches only Mark, while its caller still needs the published Unused
allowance. Its twin removes only Unused from that caller's ceiling. Unused is
root-registered for audit, so its overdeclaration must remain visible separately
from checked-body reach. Package comparison retains exact declaration ownership.
`contracts/generic_narrow` checks nominal substitution against a wider allowed
requirement: the selected inferred Mark contract fits a Mark-only caller, while
a twin with a published Mark + Unused contract rejects. In contrast,
`contracts/target_union` keeps the Forbidden contribution of a Windows body even
when this host executes the Mark-only macOS body; its Mark-only caller twin
rejects. `contracts/disjoint_service/invalid` rejects Forbidden under a
Mark + Unused ceiling.

### Baseline and repair mechanism

At `b50066b75303e6e899f27764c28e57deb5dbb1ab`, the optimized public compiler
wrongly accepted the frozen transitive empty-ceiling control. The unchanged
positive read returned 7 and both engines exited 0. Generic and dynamic
empty-ceiling controls also wrongly accepted. The original baseline receipt is
retained in this wave's qualification record; historical Linux direct-call
observations above remain historical observations.

The checker already derived the transitive row, but validated checked bodies
using a second direct-boundary-only scan. That scan is removed. Every authored
ceiling checks the complete ordinary inference row, including helper, callback,
cleanup and target-family contributions. Inference records the body row beside
an unchanged written ceiling; it publishes the inferred row only when the
clause is absent. This preserves declared slack without using it as body reach.
Dynamic lowering names a dispatch slot, so the shared service query follows the
slot's recorded requirement identity before reading its contract. It never
reselects a provider by spelling or uses its body to narrow the requirement.

### Requirements established by these productions

- **Establishment:** the exact boundary requirement introduces Mark; checked
  calls record that requirement's row. Omission requests inference, while an
  authored clause creates an independent upper-bound obligation.
- **Identity/lifetime:** rows use exact declaration and nominal binder identities;
  no mutable value or runtime selector can rewrite a published row. Public and
  private helpers preserve that identity, and declared slack stays contractual.
- **Composition:** helpers, generic substitution, dynamic slots and target-family
  joins carry the same requirement. Every empty-ceiling twin changes only the
  consuming contract, and all applicable nine transports are retained.
- **Soundness:** boundary/provider authority, trust, `invokes`, blocking,
  suspension and crashes remain independent. Selected implementations must fit
  their requirements; a silent selected body cannot erase the pinned reach.
- **Psi 05–07:** the checked service records survive ordinary Terminal production;
  positive runtime observations require actual Terminal execution.
- **Omega 00–02:** exact boundary/provider identity selects the read implementation;
  result and argument placement preserve 7 through the nine carriers.
- **Omega 03–06:** the actual macOS ARM64 realization executes each call and its
  continuation using the ordinary register/frame route.
- **Omega 07–09 and emission:** Mach-O execution must exit 0 and agree with
  Terminal Psi. Other hosted targets are bound and checked on this host; their
  native execution is not claimed by these records.

The complete parent remains open until mandatory boundary requirement ceilings,
all affected source migration, remaining refinement cases and package-review
acceptance are demonstrated. Run records identify the actual producer and host;
source presence or a filled row alone does not close the task.

### macOS ARM64 observation

The optimized producer for this repair has CLI SHA-256
`fbaacd199c2a6660f13f68858f4495ce79fb5164a950c4b191552d8b34e25acd`
and corpus runner SHA-256
`a54e22af13250656ae8c5297f7378c39d97e02bc1db3aa28601162d50bb9c69b`.
The source closure is the repair's Rust tree on base `b50066b75303`; the
worktree-bound build receipt retains exact input blobs and profile queries.

- The affected sample gate measured 41/41 positive native oracles and 40/40
  authored rejection identities on `macos_arm64` (one job, 85 seconds).
- The scoped checked corpus recorded 85 fixtures: every positive checked and
  every invalid control rejected with its authored fragment.
- The scoped both-engine corpus recorded 41 built positives, each with exit 0
  and native/Terminal agreement. This includes all original and new transported
  positives and the executable contract probes; the package-round-trip fixture
  also executes to its authored exit 0.
- `omega audit packages --project contracts/declared_slack --details` after
  ordinary project review reports `requires-review false`. Its fresh policy
  retains published Mark + Unused, checked Mark, and one ConsumerAudit Unused
  slack finding. Accepted lock data is historical; the fresh findings supply
  the current comparison observation.

The corpus records carry `@b50066b75303+wt`, preserving their actual published
base plus the staged source delta instead of claiming an untested later main
revision. The existing broader corpus has unrelated pinned failures; these
scoped records neither overwrite nor qualify them.

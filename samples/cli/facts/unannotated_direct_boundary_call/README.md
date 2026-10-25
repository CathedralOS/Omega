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

### Remaining acceptance after the original direct-call row

The original row established direct calls only. The productions below retain
that same Mark fact through transitive, generic, dynamic, injected-provider,
imported-public and recursive calls. They also distinguish absent versus empty
ceilings, require boundary requirements to publish their contract, and exercise
package review, conservative slack and exact service identities.

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
  positives and the executable contract probes. Package acceptance round-trip
  behavior remains separately required by the parent task.
- `omega audit packages --project contracts/declared_slack --details` after
  ordinary project review reports `requires-review false`. Its fresh policy
  retains published Mark + Unused, checked Mark, and one ConsumerAudit Unused
  slack finding. Accepted lock data is historical; the fresh findings supply
  the current comparison observation.

The corpus records carry `@b50066b75303+wt`, preserving their actual published
base plus the staged source delta instead of claiming an untested later main
revision. The existing broader corpus has unrelated pinned failures; these
scoped records neither overwrite nor qualify them.

## Explicit boundary requirement ceilings

The same nine original positive places retain `Mark::read`'s explicit `Mark`
ceiling. Each place's [`invalid_missing_requirement/main.omg`](local/invalid_missing_requirement/main.omg)
removes only that requirement clause, keeping the selected checked implementation
and the consuming program unchanged. The twins are pinned as
`tests/omega/fail/facts/boundary_requirement_ceiling_missing_through_<place>_rejected`.
They require `Mark::read`'s missing explicit-ceiling diagnostic, independently of
whether the caller itself requests inference or declares a ceiling.

| Place | Positive | Missing-ceiling twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [reject](local/invalid_missing_requirement/main.omg) |
| field | [program](field/main.omg) | [reject](field/invalid_missing_requirement/main.omg) |
| parameter | [program](parameter/main.omg) | [reject](parameter/invalid_missing_requirement/main.omg) |
| call_result | [program](call_result/main.omg) | [reject](call_result/invalid_missing_requirement/main.omg) |
| match_join | [program](match_join/main.omg) | [reject](match_join/invalid_missing_requirement/main.omg) |
| const_name | [program](const_name/main.omg) | [reject](const_name/invalid_missing_requirement/main.omg) |
| other_state | [program](other_state/main.omg) | [reject](other_state/invalid_missing_requirement/main.omg) |
| other_machine | [program](other_machine/main.omg) | [reject](other_machine/invalid_missing_requirement/main.omg) |
| target_body | [program](target_body/main.omg) | [reject](target_body/invalid_missing_requirement/main.omg) |

This establishes the declaration-side obligation in
[effects, declarations](../../../../wiki/spec/language/effects.md#declarations):
a boundary requirement publishes its own ceiling, including when it supplies a
default body. An empty ceiling is retained as a real authored clause; omission
on a checked implementation remains inference. A nested static binder's ceiling
belongs to that binder and cannot annotate the enclosing requirement.
Memberless boundary identities have no operation requiring an annotation.

The source migration adds explicit empty ceilings to formerly empty boundary
requirement contracts. It preserves their parameters, results, facts, bodies,
provider selections and runtime oracles. The product Psi parser retains the same
empty-row distinction and refuses omitted requirement ceilings. Its native parser
gate also covers the declaration terminator, a following clause and a default
body as distinct delimiters. The requirement's static rejection occurs before
Terminal production; the original positive places retain their native/Terminal
execution obligations.

## Checked realizers and imported public summaries

`provider/` and `imported/` are further productions of the same Mark fact.
They retain the identical nine transport places and the authored exit 0.

The provider production gives `Mark::read` a pinned Mark ceiling. Its selected
checked realizer omits its own clause and calls `Backend::read`, whose explicit
row is also Mark. `Backend` is an interface identity; calling it does not add
Backend to the service row. Each twin changes only that realizer call to
`Forbidden::read` and requires the trait-refinement rejection. The requirement
permits both synchronous invocation identities, so the rejection isolates the
service ceiling rather than missing invocation permission. Both providers still
return the same 7; a runtime equality cannot hide the contract violation.

The imported production calls a dependency's public `fetch_mark`, which omits
its own reach clause and calls a private helper. The root supplies the selected
Mark realizer. Each twin changes only the original consuming callable's clause
to empty; its support package and provider selection remain identical. This
checks that an imported public summary propagates Mark through ordinary package
checking and accepted-input compilation. Omission is preserved separately from
the explicit empty boundary requirement/caller cases.

| Place | Provider positive / twin | Imported positive / twin |
| --- | --- | --- |
| local | [positive](provider/local/main.omg), [reject](provider/local/invalid/main.omg) | [positive](imported/local/main.omg), [reject](imported/local/invalid/main.omg) |
| field | [positive](provider/field/main.omg), [reject](provider/field/invalid/main.omg) | [positive](imported/field/main.omg), [reject](imported/field/invalid/main.omg) |
| parameter | [positive](provider/parameter/main.omg), [reject](provider/parameter/invalid/main.omg) | [positive](imported/parameter/main.omg), [reject](imported/parameter/invalid/main.omg) |
| call_result | [positive](provider/call_result/main.omg), [reject](provider/call_result/invalid/main.omg) | [positive](imported/call_result/main.omg), [reject](imported/call_result/invalid/main.omg) |
| match_join | [positive](provider/match_join/main.omg), [reject](provider/match_join/invalid/main.omg) | [positive](imported/match_join/main.omg), [reject](imported/match_join/invalid/main.omg) |
| const_name | [positive](provider/const_name/main.omg), [reject](provider/const_name/invalid/main.omg) | [positive](imported/const_name/main.omg), [reject](imported/const_name/invalid/main.omg) |
| other_state | [positive](provider/other_state/main.omg), [reject](provider/other_state/invalid/main.omg) | [positive](imported/other_state/main.omg), [reject](imported/other_state/invalid/main.omg) |
| other_machine | [positive](provider/other_machine/main.omg), [reject](provider/other_machine/invalid/main.omg) | [positive](imported/other_machine/main.omg), [reject](imported/other_machine/invalid/main.omg) |
| target_body | [positive](provider/target_body/main.omg), [reject](provider/target_body/invalid/main.omg) | [positive](imported/target_body/main.omg), [reject](imported/target_body/invalid/main.omg) |

The corpus copies each complete project under
`tests/omega/{pass,fail}/facts/unannotated_{provider,imported}_boundary_call_through_<place>_{exit,rejected}`.
The frozen f5c producer already executes the minimal provider and imported
probes with native exit 0 and Terminal exit 0 and rejects their exact controls.
These establish remaining acceptance of the existing inference/selection
mechanisms; the demonstrated repair in this change is the missing explicit
boundary requirement obligation, not a manufactured failure in these probes.

## Ranked recursive summaries and ordinary exits

`recursive/` carries the same Mark fact through a ranked mutual tail-call
component. Both machines count down from the same `u64` measure, each internal
edge strictly decreases it, and the Mark-reaching checked helper sits outside
the component. The boundary requirement supplies an explicit termination
premise for that helper. These independent progress obligations are proved;
an empty reach ceiling neither supplies nor weakens them.

The local positive is the frozen current-main-integrated defect witness:
source checking succeeds, but native realization refuses the helper call in
`second::entry` because scalar-call availability asks for that state's flow
under `first`, the component's dispatch owner. Fusion retains the exact authored
state symbol and owner. The repair derives that owner from the state before
replaying the call's ordinary arguments, claims and contract evidence. It does
not fabricate another callee plan or skip availability checking.

Every recursive place has an empty-ceiling twin pinned under
`tests/omega/fail/facts/unannotated_recursive_boundary_call_through_<place>_rejected`.
The positive copies are pinned in the corresponding pass folders.
[The foreign-owner exit](contracts/recursive_foreign_exit/main.omg) starts at the
other countdown parity so runtime execution also exits through the non-root
owner's helper call. Both exits preserve the same service row and result 7.

| Place | Recursive positive | Empty-ceiling twin |
| --- | --- | --- |
| local | [positive](recursive/local/main.omg) | [reject](recursive/local/invalid/main.omg) |
| field | [positive](recursive/field/main.omg) | [reject](recursive/field/invalid/main.omg) |
| parameter | [positive](recursive/parameter/main.omg) | [reject](recursive/parameter/invalid/main.omg) |
| call result | [positive](recursive/call_result/main.omg) | [reject](recursive/call_result/invalid/main.omg) |
| match join | [positive](recursive/match_join/main.omg) | [reject](recursive/match_join/invalid/main.omg) |
| const name | [positive](recursive/const_name/main.omg) | [reject](recursive/const_name/invalid/main.omg) |
| another state | [positive](recursive/other_state/main.omg) | [reject](recursive/other_state/invalid/main.omg) |
| another machine | [positive](recursive/other_machine/main.omg) | [reject](recursive/other_machine/invalid/main.omg) |
| target body | [positive](recursive/target_body/main.omg) | [reject](recursive/target_body/invalid/main.omg) |

## Package review custody

A public checked helper that omits `reaches` has an inferred published contract;
review must not manufacture an authored clause. The imported support package's
ordinary `main.omg` review entry loads its exported values module. Its
`fetch_mark` policy retains no declared clause and a checked/published Mark row.
The explicit wider contract keeps Mark + Unused even when its body checks as
Mark. The public helpers in [the empty control](contracts/empty/main.omg) retain
`Some(empty)` versus inference while both execute to 7 without a service.

`compiler`'s `package_review` integration gate reviews the pinned corpus copies,
encodes and recovers source-free policy, and asserts these distinctions.
Authority consumers use the explicit/governing ceiling or the checked realized
row for inference. They never substitute a narrower concrete row or infer a
contract for an unknown body. Declared slack continues to use an actual ceiling.
Opaque/bodyless supplies preserve their governing published contract.

The package workflow also keeps a fresh accepted project, changes only a private
helper from a Mark-reaching call to an Extra-reaching call, observes ordinary
compilation reject stale inputs, revalidates, and observes inferred Extra in the
public wrapper and native/Terminal exit 0. A caller constrained to Mark rejects
Extra after the same change. Invocation permissions and provider selections
permit both calls throughout, isolating reach rather than another obligation.
These workflow outputs retain exact producer, source, policy and execution
artifacts in the qualification record; acceptance is not claimed by a source
presence count or historical lock alone.

## Current macOS ARM64 evidence

The second repair's immutable optimized producer is built from compiler revision
`1759dd244876856c7833b9ad87de746e7d72eb50` on published base
`daf76df20a9df4bfcd0f9587c4e1545036ab8764`. Its build receipt retains full
compiler/library input blobs, executable digests and verified build profiles.
These are observations of that producer, not executions of later main commits.

- The sample gate measured all 69 positives in this folder, including its root,
  with native exit 0, and all 76 authored reject controls with matching reasons.
- The pinned both-engine gate measured 68 corresponding positive fixtures plus
  the existing reviewed-service-row control: all 69 native and Terminal
  executions exited 0 and agreed. The sample root separately ran with
  `omega run --both --keep`, also native/Terminal 0.
- The 1,041-fixture affected checked pass retained the migration's broader
  consumers. A focused comparison measured the 43 moved or historically red
  fixtures against the actual published base. A Windows metadata fixture that
  exceeded the busy broad pass's time limit subsequently checked with matching
  Terminal inspections in a focused run; no outcome was reclassified by raising
  an oracle bound.
- The 480-project affected sample pass retained 195 native passes and 244
  fact-sensitive rejections. Its 21 failures were reproduced on the exact
  published-base compiler/library or attributed to the unchanged, absent macOS
  foreign-retention dylib. Ten foreign-host projects and ten library projects
  supplied no host runtime oracle. Those cases are not new green observations.
- Strict package update/review/resume, fresh and cached audit, private-helper
  source invalidation, revised inference and constrained-caller rejection pass.
  Fresh audit preserves core filesystem classification, same-named user-trait
  separation, custom audit registration and explicit conservative slack.

Independent semantic review accepted the exact compiler candidate. Broad raw
observations, source/producer identities, baseline comparisons and review are
retained in the implementation qualification record. Native observations on
other development hosts remain unverified; four hosted entry bindings and
checking of every target body do not claim those executions.

### Integrated requirement migration

Published main `a9e0628deefea9471e49ff8b055e36c70577ee07` added another 77
omitted requirement ceilings in 39 files while the first measurements ran.
Those declarations now explicitly preserve their original empty contracts.
The integrated compiler revision `a5b6463056a9c3e818e10c1f6628306cc27702e3`
passes the two package policy integration tests and the strict package workflow.
Its final scoped check records 77 checked positives and 85 matching rejects;
all 78 pinned runtime positives agree at native/Terminal exit 0. Its sample
gate observes 79 native positives and 87 matching rejects, and both sample
roots separately agree at native/Terminal exit 0. These records include the
newly migrated input-authority row as regression evidence, not a new reach fact.

The records use the published base plus the staged delta (`@a9e0628deef+wt`).
Independent review found the added ceilings insertion-only and verified that
all 17 reviewed implementation files retain their exact bytes.

The Omega-written parser's new cases are retained in its existing black-box
gate. That full harness previously refused loan-chain lowering. A separate
finite probe reaches native execution but fails at initial Lexer storage
(native signal 11 and Terminal's zeroed-structural-backing refusal), before the
new parser branch. Product-parser native execution is therefore unverified;
these reach witnesses do not claim that unfinished product pipeline is green.

The final compatibility step preserves the empty contract of the newly published
Build observer requirement on base `5f1191d434382cdf52a6c397d58af723d077fa68`.
Producer `8ad1ae9751330a18ef4c670205524d579342a510` records 78 checked positives,
85 matching rejects and 79 successful pinned native/Terminal executions; its
166-project sample pass retains 79 positives and 87 matching rejects. The finite
product-parser probe's initial-storage failures were also replayed on the exact
published `daf76df20a9d` compiler and archived Psi/library sources. They remain
attributed product-pipeline limitations, before the changed parser branch.

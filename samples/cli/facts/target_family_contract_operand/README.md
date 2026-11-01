# Generic callback contract operands

Parent: **BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md).

Generic callback operands retain exact declaration identities through explicit requirements and selected implementations; a call with 42 satisfies the common <=64 contract and preserves Item callback behavior.

The ordinary `Choosing::choose` requirement publishes `number <= 64` and a
callback usable for `count <= 512`. The four original generic bodies retain
their own parameter identities and complete contracts:

| Implementation | Type / callback binders | Input bound | Callback bound |
| --- | --- | --- | --- |
| ArmChoice | Value / Selected | 64 | 64 |
| X64Choice | Element / Callback | 128 | 128 |
| DarwinChoice | Payload / Operation | 256 | 256 |
| WindowsChoice | Datum / Action | 512 | 512 |

Build selects these ordinary named implementations explicitly for their original
four targets. Source checks every map regardless of selection. The public input
bound implies each provider input bound. Callback inputs are contravariant:
callers supply capability through 512, so a provider needing only 64, 128 or
256 asks no more. A total callback is valid, but is not mandatory. The original
`forward` function keeps its authored total-callback contract. No implicit
family join, target modifier or private-body guarantee supplies the caller's
contract.

Each twin changes only Windows' input bound to 41. It must reject because the
public requirement does not imply that provider precondition, including on macOS
where WindowsChoice is unselected. This replaces the retired family's false
call-conjunct diagnostic with ordinary implementation refinement.

- [local](local/main.omg) / [reject](local/invalid/main.omg)
- [field](field/main.omg) / [reject](field/invalid/main.omg)
- [parameter](parameter/main.omg) / [reject](parameter/invalid/main.omg)
- [call result](call_result/main.omg) / [reject](call_result/invalid/main.omg)
- [match join](match_join/main.omg) / [reject](match_join/invalid/main.omg)
- [const name](const_name/main.omg) / [reject](const_name/invalid/main.omg)
- [nested machine](other_state/main.omg) / [reject](other_state/invalid/main.omg)
- [another machine](other_machine/main.omg) / [reject](other_machine/invalid/main.omg)
- [selected target body](target_body/main.omg) / [reject](target_body/invalid/main.omg)

The selected-target place uses an ordinary DirectRouting requirement and shared
DirectCode implementation, which calls the target-selected Choosing requirement.
It retains the original direct-forwarding body and complete reach ceiling.

Every positive runs the original Item code 42 case and an Item code 43 case:
number stays 42 through the same place, while both direct and forwarded calls
must return true then false. Constant results, a discarded callback, a wrong
Item argument or a changed numeric argument therefore fail the runtime oracle.
Match join executes both choices with distinct route markers 11 and 22; its
ordinary marker helper promises the same returned number, without a caller
proof guard. Every positive binds all four hosted ProgramEntry requirements.
These bodies intentionally return the same semantic results on every target;
this row does not claim different per-OS runtime result discrimination.

The [shadowing control](shadowed_scope/main.omg) retains independent same-spelled
bool locals/parameters beside the u64 contract operands. Its
[foreign-scope twin](shadowed_scope/invalid/main.omg) still rejects an undeclared
read of another machine's number. The [bounded callback control](controls/callback_capability/main.omg)
accepts a callback usable through 512; its [twin](controls/callback_capability/invalid/main.omg)
limits that callback to 64 and must reject despite the current literal call 42.
The [stronger provider control](controls/stronger_provider_requirement/main.omg)
demands callback capability through 1024 and must reject against the published
512 requirement even when that provider is unselected.

## Requirements and evidence

- Establishment: the transported 42 proves the public input bound before each
  call. Callable formation checks the complete supplied type/signature before
  contract implication. The caller's actual 42 cannot substitute for universal
  refinement of the generic maps.
- Identity and lifetime: exact positional declaration symbols identify runtime,
  result and static binders. Display names or unrelated shadowing declarations
  grant neither type meaning nor a premise. Existing borrow/lifetime formation
  remains independent of arithmetic entailment.
- Composition: all nine places preserve the same number and callback operation,
  including total-callback forwarding and target-selected generic bodies. Both
  callback outcomes and match routes are observable. Receiving runtime equality
  checks establish no precondition of a completed call.
- Soundness: all nine narrower-provider twins, wrong callback capability,
  stronger provider demand and foreign-scope read must reject. Type formation,
  nominal authority, effects, termination and crash ceilings are checked before
  or independently of fact implication.
- Psi checking uses the existing declaration ContractFacts/Conjunct judgment,
  including scoped integer and IEEE implication, when exact fact correspondence
  alone is insufficient. Required premises imply actual premises; actual
  guarantees imply required guarantees. Input variance already reverses the
  complete nested contracts and binder frames. Unknown implication refuses.
  Unsupported captured applications may still conservatively refuse when their
  substitution is unavailable; no broader generic-refinement closure is claimed.
- Terminal and native realization must preserve exact static callback selection,
  Item references, integer arguments, bool results and selected implementation
  identity through the ordinary provider/calling pipeline. Actual matching-host
  execution is required; authored target rows alone do not prove other hosts.

The governing contracts are [substitution](../../../../wiki/spec/language/machines.md#substitution),
[nested callable families](../../../../wiki/spec/language/generics.md#nested-contracts-and-proof-families),
[named implementations](../../../../wiki/spec/language/conformances.md), and
[Build-owned target selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection).

## Preserved baseline

The retired family model used a conjunction of all four member preconditions.
On historical base f7681adbc9b1190fbc19abf217b28e62ea2fb037 the copied member
operands wrongly refused; the scoped place-type repair retained their exact
symbols in the receiving telescope. Historical Linux and Windows observations
proved the older family row, not this new Source migration.

At fixed migration base 1ea8c21f8b4dc2c96e9ee4dbfd2fc100baba61f5, retained optimized
producer 4e45c0b749ed passes the original 20 Source outcomes and 10 actual
native/Terminal zero-exit pairs on macOS ARM64. The Source gate's exit 1 records
only nine generated-symbol diagnostic differences; the both gate's exit 1 records
new matching-host golden keys. Neither has an oracle failure. An ordinary
Choosing prototype on that producer then wrongly rejects the three valid
callback weakenings 64/128/256 to 512. Its Windows 41 twin separately rejects
for the intended outer precondition. These exact observations justify the
shared implication repair; the initial missing-explicit-reach setup attempt is
retained separately from that compiler defect.

The new optimized producer 98708cfd91eb has scoped validation and Clippy passes,
with immutable Rust/SDK inputs and executable profiles. Critical native/Terminal
observations accept the migrated local, selected-body and bounded-512 programs;
reverse callback/provider capability controls and the unselected Windows 41 body
reject. Fresh observations are macOS ARM64 only. The parent target-selection
migration and wider dynamic/generic contract acceptance remain open.

## Migration acceptance

The full nine-place row and shadowing programs preserve ten public native/Terminal
zero-exit agreements and ten intended Source refusals. The three new capability
controls are pinned with the row: 23 Source outcomes (11 checked, 12 intended
rejections) and 11 native/Terminal pin agreements pass on producer 98708cfd91eb.
All four target profiles pass selected-body Source preparation/checking.

Seven Terminal counterfactuals distinguish both callback outcomes, both match
routes and the selected body: the unchanged program and a changed unselected
ArmChoice body exit zero; constant true/false callbacks, either wrong route,
and a changed selected DarwinChoice body produce typed Traps. No native trap
wave is claimed. Eight existing deep nominal-variance, applied-type-bound,
proposition-tag and nested-scope Source controls preserve the baseline's four
positive/four rejection split and diagnostics. These checks do not claim
matching-host execution outside macOS ARM64 or complete the parent migration.

# Nested borrowed field production

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, recursive/reference
predicate read inventories. This is another production of the existing
[data_where_standing](../../README.md) fact. `Holder.counter` stores a shared
reference to `Counter`, whose `value` stores a separate shared reference to
`i32`. The bound and exact operation match the [borrowed field production](../README.md).
The consuming body passes that loaded scalar descriptor through ordinary read.

Before each transport, capture copies the scalar reference from an incoming
SourceHolder, saves its read and neighboring scalar fields, then replaces
holder.counter. A second read through the saved reference must equal the first.
The helper constructs the returned Counter from that saved reference and the
original neighboring values. SourceCounter supplies the predicate needed to
construct Counter; the output annotation cannot establish its own bound.
The existing nine places then transport that returned Counter through Holder
and perform the same exact addition.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [missing predicate](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [missing predicate](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [missing predicate](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [missing predicate](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [missing predicate](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [missing predicate](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [missing predicate](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [missing predicate](other_machine/invalid/main.omg) |
| Selected target implementation | [program](target_body/main.omg) | [missing predicate](target_body/invalid/main.omg) |

Each twin removes only SourceCounter's default predicate. Counter keeps its
predicate, so the returned record's construction must reject when its input no
longer supplies the bound. Public capture and consuming bodies owe their
obligations independently of main's literals. Entries replace the incoming
holder pointers in both directions, from 60 to -17 and from -17 to 60. The
returned Counters must still produce 130 and 53 through the original transports.
Entries verify unchanged source and output neighbors, backing scalars, holder
canaries and the newly installed pointer markers and values. The match entry
executes both selections; selected-target entries keep distinct provider tags.
The const-name place still materializes HIGH and LOW through Backing, then
produces SourceCounters before capture. Every positive declares exit 0 and
binds all four hosted targets.

| Descriptor control | Obligation |
| --- | --- |
| [Saved pointer](controls/snapshot/main.omg) | A copied scalar pointer keeps its original referent after the local record pointer is replaced. |
| [Replacement identity](controls/snapshot/invalid/main.omg) | The old value cannot prove equality to a read through the replacement. |
| [Known local supplier](controls/known_source/main.omg) | A locally constructed descriptor keeps its established supplier. |
| [Write-only intermediate](controls/write_only/invalid/main.omg) | A write-only reference cannot authorize reading the nested scalar descriptor. |
| [Suspended descriptor home](controls/suspended_home/invalid/main.omg) | A live exclusive borrow prevents a conflicting descriptor read. |

## Requirements and evidence

- **Establishment:** SourceCounter establishes bounded(value) == true on the
  saved scalar reference. After replacing the incoming holder's pointer, capture
  must prove that predicate when constructing Counter. Counter's standing
  predicate then justifies the exact addition through read's checked equation.
  No caller guard or receiving annotation supplies a missing premise.
- **Identity and lifetime:** the Holder slot, Counter descriptor, scalar-pointer
  slot and scalar referent remain distinct. Reading a pointer field retains
  the actual completed descriptor and its lifetime. The receiver permits the
  slot read; its address is not the child's pointee or reborrow parent.
- **Composition:** every place transports the same record/reference/predicate.
  A match preserves every incoming origin and the selected receiver identity.
  Descriptor snapshots and current referenced contents have separate epochs.
  Replacing SourceHolder.counter must install the new reference without
  retargeting the separately saved scalar reference or its surviving predicate.
- **Soundness:** complete source inventories, readable intermediate references,
  actual pointer-slot access, exact carrier/access and surviving writes remain
  mandatory. Distinct loaded-pointer namespaces prove no spatial separation.
- **Checked-to-Terminal Psi:** the bound erases; pointer loads, scalar reads,
  helper calls, selection and canary observations remain executable.
- **Layout, ABI, instructions and encoding:** existing reference/record layout,
  calling plans and image emission must preserve both pointer crossings and
  agree with Terminal Psi. Authored target bindings establish no foreign-host
  runtime observation.

The earlier discovery base was 539d46c0eb8aa68b72e9f3b326c27d8ed2959346.
Before this incoming-holder extension, the nine original places and two positive
descriptor controls produced native and Terminal exits 0/0 on macOS ARM64.
Their nine predicate-removal twins and three identity/authority controls
rejected their intended obligations. Those observations describe the previous
programs and remain historical evidence. They do not qualify the changed
SourceCounter/capture programs or their constructor refusals.

The incoming-holder replacement case is frozen separately in
[the discovery control](controls/incoming_snapshot/README.md). Its source and
twins remain unchanged. On optimized producer cd5340c6d0348e868e34d68ee85f250c431f79ba,
all nine new positives refuse Counter construction even though SourceCounter
supplies the bound. The nine twins correctly refuse that same construction
after removing the input predicate. Initial positive attempts stopped at
incompatible historical package locks; those setup outcomes remain separate.
Replays in fresh projects containing the identical authored files establish the
nine constructor refusals without changing or deleting those historical locks.

The current row still requires every positive to produce native and Terminal
exit 0, each sole-predicate twin to retain its Counter-construction refusal,
and the existing identity/authority controls to retain their intended outcomes.
The initial input hashes and both sets of baseline observations are retained
as incoming-reference-row-initial-inputs.json,
incoming-reference-row-baseline-observations.json and
incoming-reference-row-baseline-fresh-observations.json in the existing PR
evidence directory.

Prepare each positive with omega update --project <place> --target macos_arm64
--offline, then run omega run --both <place>/main.omg. Producer revisions,
binary digests, baseline refusals and observed outcomes belong in the existing
PR evidence. The corpus pins still retain the earlier source programs until
the changed row has been validated.

The exact storage and lifetime rules are
[entry and current scalar values](../../../../../../wiki/spec/terminal-psi/fact_validity.md#restored-scalar-values),
[replacement of carried borrows](../../../../../../wiki/spec/language/lifetimes.md),
[captured places](../../../../../../wiki/spec/terminal-psi/loans.md#captured-places-and-available-premises)
and [source-derived qualification](../../../../../../wiki/spec/language/domains.md#declaration-and-membership).

This production does not close the parent task's remaining recursive/case/
collection inventories, proposition handback, filesystem qualification or
source migration. GUI qualification remains pending separately.

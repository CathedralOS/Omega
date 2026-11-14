# Bounded atomic fetch

Owning outcome: **ATOMIC-MEMORY-MODEL** in TASKS.md.

Fact: **A shared atomic fetch preserves its resident bound and ordering, and a load carries that bound into Exact subtraction.**

Every place preserves bound 100, mutates 99 through a shared `fetch_and` helper with mask 63, and consumes the observation in `100 - observed` before checking runtime result 65. Each twin changes only the initial atomic store from 99 to 127. That store must establish the unchanged resident bound immediately; the later AND cannot excuse an invalid intermediate resident.

The former mask-127 controls were valid programs. Unsigned AND cannot increase
an already valid resident, so a helper allowing masks through 127 still keeps
the standing ceiling 100. The shared numeric bound calculation previously lost
that ceiling while rounding to a common bit prefix. Those programs remain as
positive [resident-bound productions](resident_bound/README.md), each with its
own store-127 establishment twin. The bound calculation keeps both known bits and the
admitted unsigned numeric ceilings; no atomic-specific proof rule is added.

The match programs run both choices and observe an independent initialized
marker. Taking the same arm twice cannot satisfy the exit oracle. The selected
target pins now match the ordinary-provider samples and their explicit Build
rows; the previous prefixed pins are retained in the baseline snapshot.

## Integrated publication validation

At exact compiler source `d322ce056e15b7519ec54018b8e6963b99e5957c`,
[the Linux row replay](https://github.com/CathedralOS/Omega/actions/runs/41818931334)
checks all eighteen positives and rejects all eighteen establishment twins for
their intended store obligation. All eighteen fact programs execute at zero in
both engines. The Boolean and single-attempt neighbors also agree at zero;
the width-sensitive program agrees at its authored exit 70.
[The same compiler’s seed](https://github.com/CathedralOS/Omega/actions/runs/41815967062)
observes all 37 ordinary projects: nineteen zero agreements and eighteen intended
refusals. Both optimized binaries are authenticated before and after the replay;
no second compiler build or changed case limit is involved.

The integration changes how calls preserve returned bounds and how branches are
planned, so this complete Linux row was rerun. The atomic instructions and their
checks retain their meaning. The other three hosts keep their actual earlier
491/8679/417a observations described below, with independent review of the
unchanged physical behavior and equivalent queries. No execution of the current
artifact format on those hosts is claimed by this integration.

Two earlier supplemental Linux legs remain actual timeouts: standalone width
checking and a non-atomic byte-field snapshot execution. Their failed summaries
and positive oracles remain unchanged. The width program has actual successful
BOTH observations, including the integrated replay; that does not relabel the
separate timeout. Two numeric proof failures reproduce on the fixed pre-repair
compiler and likewise remain open. Concurrent contention, forced retry, formal
memory-model axioms, target-refinement proofs and the broader
**ATOMIC-MEMORY-MODEL** acceptance are not closed by this row.

<details>
<summary>Earlier exact sources, controls and retained measurements</summary>

The frozen [Linux ARM64 baseline](https://github.com/CathedralOS/Omega/actions/runs/41778920756)
uses `4637c6da3b4843292581b9b04f7c39b43675a75e`, whose tree equals published
`f59057c97230bd9715b3cbc50faa9b4da291b3b7`. All nine original positives check,
but native realization refuses `FetchAnd` before either engine runs. The nine
mask-127 programs also expose the shared numeric approximation described above.
Their old source and refusal oracles remain frozen in that evidence.

The repaired compiler at `491bd96c236f147d52531c4ebb05076a0bb557ef` has actual
native/Terminal exit-zero agreements for all eighteen positive productions on
all four matching hosts. Each store-127 twin rejects its establishment
obligation. The ordinary CLI observes all 37 projects, including the root.
The target programs use separately recorded corrected inputs at
`86799c125677a194a7bc4db22bd6eb8dac547c9e`; their compiler remains the actual
archived 491 source. The empty service promise is retained, with the selected
trait recorded only as an invocation dependency.

The completed Linux row uses compiler source
`f012a95f6e18e4a85afc43784a2a46bd748bdcac` in
[run 41805379523](https://github.com/CathedralOS/Omega/actions/runs/41805379523).
All eighteen fact positives check and execute at native/Terminal exit 0; all
eighteen establishment twins reject. The Boolean and single-attempt neighbors
also agree at 0, while the width-sensitive program agrees at its authored 70.
[The retained-compiler replay](https://github.com/CathedralOS/Omega/actions/runs/41808847330)
observes all 37 ordinary projects: nineteen zero agreements and eighteen
intended refusals.

The other hosts retain their actual observations. ARM uses
[the full 491 scope](https://github.com/CathedralOS/Omega/actions/runs/41787238415)
plus [corrected target inputs](https://github.com/CathedralOS/Omega/actions/runs/41793274090).
macOS and Windows retain the unchanged 491 fact/neighbor observations and their
[macOS](https://github.com/CathedralOS/Omega/actions/runs/41793281052) and
[Windows](https://github.com/CathedralOS/Omega/actions/runs/41793284487) corrected
input replays. Their width programs now have fresh Source and actual 70/70
observations at 417a in
[macOS run 41802041877](https://github.com/CathedralOS/Omega/actions/runs/41802041877)
and [Windows run 41802045704](https://github.com/CathedralOS/Omega/actions/runs/41802045704).
The later state-grouping and case-observation queries preserve the exact rows,
full identities, duplicate rules and order; independent source review accepts
reuse at these original producer stamps.

A final storage-only correction uses dense arena-slot buckets while retaining
full handle/generation equality and the original observation order.
[Its focused Linux run](https://github.com/CathedralOS/Omega/actions/runs/41811817773)
at `69b68f2359b97883bfeb27fa6860de4a1e80441d` checks the atomic match and
byte-case cleanup positives plus their controls; both positives actually agree
at exit 0. Independent source review accepts the unchanged full-row results
above at their original f012/417a producer identities. This establishes no new
other-host execution or timing result.

The workflow repairs remove repeated searches of unrelated state/capability
rows and sorting of facts that case observations never consume. They preserve
all authority and evidence checks. No speed ratio or general performance
closure is claimed.

The earlier full and isolated Linux jobs remain failed: standalone width checking times out, and an
additional non-atomic byte-field snapshot execution times out. Neither is
recorded as passed. The width's actual current BOTH result already compiles and
runs that unchanged program. The snapshot has Source success; the changed
queries are equivalent, and the current whole-case store (70/70) and byte-case
partial cleanup (0/0) exercise their relevant consumers. Its supplemental native
result remains unverified. Earlier timeout reports and their original oracles
remain retained.

Two supplemental numeric positives still refuse identically at the fixed
pre-repair source: `runtime_immutable_unsigned_bitwise_bounds_exit` and
`scalar_state_return_keeps_byte_class`. Their positive oracles and failed
summaries remain unchanged. This row does not close those proof gaps,
concurrent contention, forced retry, formal memory-model axioms or target
refinement proofs under **ATOMIC-MEMORY-MODEL**.

The final documentation and record update changes no compiler, library, authored
program, Build or oracle bytes. Executions keep the actual sources above; no
execution at a later documentation or main commit is inferred. The source
archives on PR #1795 retain those revisions through squash and branch cleanup.

</details>

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [invalid](invalid_local/main.omg) |
| Field | [program](field/main.omg) | [invalid](invalid_field/main.omg) |
| Parameter | [program](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| Call result | [program](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| Match join | [program](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| Const name | [program](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| Nested machine | [program](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| Other machine | [program](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| Target body | [program](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

Transport uses ordinary propositions under [domains](../../../../wiki/spec/language/domains.md):
record contracts retain the resident ceiling, parameters use `requires`, and
returned snapshots use `ensures result <= 100`. Receiving locals remain bare `u32`.
The owning machine's guarantee applies at its nested machine's normal exit;
nested signatures declare only arrival requirements. No caller guard or named
replacement domain re-establishes the fact.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Current target observations cover all four matching hosts with the same
empty service contract, explicit provider selection and actual exit-zero
agreements. Historical mask-control refusal is preserved as a conservative
proof limitation; the current rejection controls test invalid establishment.

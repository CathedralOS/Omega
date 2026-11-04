# Bounded atomic fetch

Owning outcome: **ATOMIC-MEMORY-MODEL** in TASKS.md.

Fact: **A shared atomic fetch preserves its resident bound and ordering, and a load carries that bound into Exact subtraction.**

Every place preserves bound 100, mutates 99 through a shared `fetch_and` helper with mask 63, and consumes the observation in `100 - observed` before checking runtime result 65. Each twin changes only the initial atomic store from 99 to 127. That store must establish the unchanged resident bound immediately; the later AND cannot excuse an invalid intermediate resident.

The former mask-127 controls were valid programs. Unsigned AND cannot increase
an already valid resident, so a helper allowing masks through 127 still keeps
the standing ceiling 100. The shared numeric bound calculation previously lost
that ceiling while rounding to a common bit prefix. Those programs remain as
positive [resident-bound productions](resident_bound/README.md), each with its
own store-127 establishment twin. The kernel keeps both known bits and the
admitted unsigned numeric ceilings; no atomic-specific proof rule is added.

The match programs run both choices and observe an independent initialized
marker. Taking the same arm twice cannot satisfy the exit oracle. The selected
target pins now match the ordinary-provider samples and their explicit Build
rows; the previous prefixed pins are retained in the baseline snapshot.

Current repair qualification is in progress. At fixed source
`4637c6da3b4843292581b9b04f7c39b43675a75e`, whose tree equals published
`f59057c97230bd9715b3cbc50faa9b4da291b3b7`,
[the Linux ARM64 baseline](https://github.com/CathedralOS/Omega/actions/runs/41778920756)
checks nine positives, wrongly refuses the nine valid mask-127 programs, and
refuses every native attempt at missing `FetchAnd` selection before either
engine runs. Its ordinary stages did not execute. The first repair probe
passed compilation and five AArch64 encoding checks, then stopped at an old
public-encoder test's register homes; it supplies no program execution result.
Full current-source checks, actual engines, ordinary projects and independent
review remain required. This does not close the atomic memory-model task.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Transport uses ordinary propositions under [domains](../../../../wiki/spec/language/domains.md):
field invariants use `where`, parameter bounds use `requires`, and returned
snapshots use `ensures result <= 100`. Receiving locals remain bare `u32`.
The owning machine's guarantee applies at its nested machine's normal exit;
nested signatures declare only arrival requirements. No caller guard or named
replacement domain re-establishes the fact.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Earlier migration evidence was Linux x86-64 native and Terminal Psi agreement
for the positive and refusal of the then-current mask control. That refusal was
a conservative proof limit, not a semantic counterexample. It is historical;
the current complete scope still needs fresh matching-host outcomes.

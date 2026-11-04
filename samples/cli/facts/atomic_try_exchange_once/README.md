# Non-observing single-attempt exchange

Owning outcome: **ATOMIC-MEMORY-MODEL** in [TASKS.md](../../../../TASKS.md).

Fact: **A non-observing single attempt returns the proposed u32 on failure and the displaced u32 on commit.**

The nearest row, [atomic_compare_exchange_once](../atomic_compare_exchange_once/README.md),
observes a copy of the resident on failure and has no displaced payload on
success. This row must preserve the other canonical outcome family: mismatch
and uncommitted attempts return the proposal, while a committed exchange returns
the displaced resident. A missing non-observing result or a substituted observing
result cannot satisfy that requirement. This is a distinct result and ownership rule,
not a larger width or count of the existing row.

The first [local program](local/main.omg) is a frozen baseline witness, with its
[result-family twin](local/invalid/main.omg). The positive keeps resident 10,
proposal 99 and untouched neighbor 17. It tests both a matching key 10 and a
mismatching key 9. A matched single attempt may lawfully be uncommitted; that
case must return proposal 99 and retain resident 10. A mismatch must return
proposal 99, never the resident 10. Success must return displaced 10 and leave
resident 99. The runtime checks occur after the operation and introduce no
static premise. The twin changes only the selected primitive to observing
compare_exchange_once while retaining the non-observing receiving type and all
oracles; it must reject the wrong canonical result family.

The non-observing result has one owner. Its `affine` rule means that it may
be consumed at most once, even though its u32 payload is copyable.

This first production uses dedicated AtomicU32 storage and the same concrete
u32 bits for its resident and comparison key. It does not select an arbitrary custom key law or
admit an affine/linear resident. The accepted [atomic outcome rule](../../../../wiki/spec/resources/access.md#atomic-compare-exchange-outcomes)
keeps Key and its encoding law in call evidence, outside the runtime outcome.
The concrete builtin admission must retain that correspondence rather than
infer a provider contract from a receiving type. Broader resident/key support
and non-observing-only placed permissions remain separate acceptance.

The fixed source/oracle baseline at `99240c1329b` is now measured by
[41824546059](https://github.com/CathedralOS/Omega/actions/runs/41824546059).
The valid local program refuses because `try_exchange_once` is unresolved;
no engine runs. Its observing-family twin rejects the intended sealed-result
obligation. The ordinary free-name control checks and actually agrees at exit 0.
This is a compiler gap, not an invalid source program. The separate earlier
request failed during workflow setup and supplies no compiler evidence.

The complete row is authored, with matching positive and negative pins:

| Place | What the example checks |
| --- | --- |
| [Local](local/main.omg) | Bind the direct operation result. |
| [Field](field/main.omg) | Move it into a record and select its field. |
| [Parameter](parameter/main.omg) | Pass it as an owned machine argument. |
| [Call result](call_result/main.omg) | Return it from an ordinary machine. |
| [Match join](match_join/main.omg) | Join two owned paths and observe markers 17 and 23 for both key choices. |
| [Const name](const_name/main.omg) | Retain the same key, proposal and neighbor through named constants. |
| [Nested machine](other_state/main.omg) | Transfer it to a nested machine. |
| [Other machine](other_machine/main.omg) | Consume it in a separate free machine. |
| [Selected body](target_body/main.omg) | Return it from one ordinary implementation selected by four explicit Build rows. |

Every place has an `invalid/main.omg` twin changing only the primitive to the
observing family. The [wide control](controls/wide/main.omg) carries distinct
values above 32 bits through the u64 payload and native register boundary;
its twin keeps the same wrong-family obligation. It is not another fact row.
The [ordinary-name control](controls/ordinary_name/main.omg) keeps a free machine
with the same spelling ordinary. Additional invalid controls require exact
[resident type](controls/wrong_resident/main.omg),
[dedicated storage](controls/ordinary_storage/main.omg),
[legal failure ordering](controls/failure_ordering/main.omg) and
[one owner of the affine result](controls/duplicate_result/main.omg).
The [discarded-result control](controls/discarded_result/main.omg) leaves the
result unused and observes a counted proposal helper, resident 10 and neighbor
17 afterward. Its rejection twin selects the wrong result family. This checks
that automatic disposal preserves once-only operand execution; the nine places
observe the returned payloads themselves.

| Requirement | Evidence required from this row |
| --- | --- |
| Establishment | The actual dedicated cell and concrete builtin identity encoding admit the operation; the sealed family fixes its cases and payload roles. The outcome type supplies no atomic authority. |
| Identity and lifetime | Failure retains proposal 99, commitment returns displaced 10, and neighbor 17 stays untouched. The affine whole result cannot be consumed twice. |
| Composition | Whole outcomes cross records, calls, parameters, both join paths, nested machines and public selected returns. No caller equality guard establishes the operation's contract. |
| Soundness boundaries | Unknown resident identities, ordinary storage and invalid failure orderings refuse. Custom encoding laws, affine residents and placed-only access are outside this concrete admission. |
| Portable program | One event keeps its actual operands, ordering and complete outcome contract through Terminal encoding and interpretation. No extra atomic read or retry supplies a payload. |
| Build, layout and calling conventions | Four explicit entry/provider rows and canonical case layouts carry the whole affine result; the u64 control challenges a distinct native carrier width. |
| Registers and instructions | Existing single-attempt ISA forms produce the prior and actual commitment tag. Native payload publication must use that tag, preserving both inputs and the canary. |
| Native images | Actual corpus and ordinary commands must agree with Terminal at authored exit 0 on every claimed matching host. |

At compiler and input source `33dbcf476f87`,
[41832747517](https://github.com/CathedralOS/Omega/actions/runs/41832747517)
checks the local positive and its intended twin, preserves the ordinary-name
and observing-operation controls, and records three actual native/Terminal
exit-zero agreements. Ordinary local and name commands also agree at zero;
the ordinary twin refuses its intended result-family obligation.

The [full-row replay](https://github.com/CathedralOS/Omega/actions/runs/41836746210)
uses that same authenticated compiler with authored inputs `e766e6465a2c`.
Seven standard places, the u64 control and ordinary-name control agree at zero
through corpus and ordinary execution. All 14 authored invalid controls match.
The field and match-join places stop in lowering and run neither engine. The
report remains failed; those two places are unfinished. The artifact records
the compiler and program-input identities separately, with unchanged compiler,
library, tooling and configuration bytes.

Checkpoint `0253ef7cc691` records the atomic result's automatic-disposal
schedule in the shared value owner and passes it through lowering. Its new
qualification also includes the discarded-result control. This source is being
measured; no successful new execution is claimed. The match diagnostic may name
a stale phase, so no descriptor-catalog cause is established from it alone.
Earlier missing-caller and dependency compile failures remain retained and
supply no program observations.

All nine places, their controls, ordinary package review and four-host
execution remain unfinished until the actual reports meet these obligations.
Formal concurrency, forced Uncommitted outcomes, affine/linear residents,
arbitrary-key laws, contention and target-refinement proofs remain open in the
parent task.

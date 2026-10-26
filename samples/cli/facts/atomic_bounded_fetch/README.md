# Bounded atomic fetch

Owning outcome: **ATOMIC-MEMORY-MODEL** in TASKS.md.

Fact: **A shared atomic fetch preserves its resident bound and ordering, and a load carries that bound into Exact subtraction.**

Every place preserves bound 100, mutates 99 through a shared `fetch_and` helper with mask 63, and consumes the observation in `100 - observed` before checking runtime result 65. Each twin changes only the mutate helper's `requires mask <= 63` to `<= 127`.

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

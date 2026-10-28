# Private arrivals preserve a bounded data field

The recorded count is below 4, so its exact increment preserves the buffer count <= 4 default domain.

This is a remaining normal-completion obligation of
[NESTED-MACHINE-UNIFICATION](../../../../../../TASKS.md). The same private-arrival
fact must justify the receiving data predicate after a store. An unrelated
sibling arm ending in Unit must not erase observations of the helper's actual
callers. This is a different receiving obligation from the adjacent array-index
row; it does not add another numeric width or count family.

Each positive transports the same `count < 4` decision, writes an unrelated
byte, and increments the bounded count. Its twin changes only that decision's
bound to 5. The resulting `count == 4` arrival cannot justify `count + 1 <= 4`
and must refuse the data `where` obligation. All positive programs bind the
four hosted entries and declare exit 0.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [program](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [program](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [program](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [program](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [program](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [program](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [program](other_state/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg) | [program](other_machine/invalid/main.omg) |
| Build-selected implementation | [program](target_body/main.omg) | [program](target_body/invalid/main.omg) |

The runtime oracle checks incrementing 3 to 4, refusing a further append at 4,
incrementing 0 to 1, and a disabled append. Saved counts, stored bytes and an
untouched canary make the executed branches observable. Caller equality checks
are runtime oracles; the store itself must independently preserve the declared
data predicate.

Optimized baseline `05e1922` wrongly refuses all nine positives at the
increment. The repaired producer `5792c168` runs all nine with native/Terminal
exit 0; their nine twins retain the intended data-store refusal. The two
positive controls also run with exit 0. The five negative controls preserve
stale-write, all-arrival, operand-formation and completion-resolution boundaries.
All 25 source fixtures and 11 runtime agreements are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_default_domain_*`.

The focused regression scope has 287 source observations: 284 intended outcomes
and three exact baseline failures. Its 155 BOTH observations contain 108
native/Terminal exit agreements, 42 declaration-only checks, three Terminal-only
runs without native bindings for this host, and two refusals. The retained
baseline reproduces all five unmet runtime outcomes and all three source
failures; their goldens are unchanged. A corrupted saved-count oracle traps
both engines. Formatting and the affected crate's Clippy pass with existing
warnings. Records retain their actual producer identities and host; this does
not establish other-host native execution.

The customer is `source/psi`: its source/token/parser tables increment bounded
counts under caller guards. A retained product parser replay
reports this data-store refusal at many such sites. Fresh full-parser checks
on both baseline and repaired compilers timed out after 90 seconds; they supply
no product acceptance. Checking an unused source module alone passes on both
compilers and is coverage only. The broader product declaration-model migration
and its checking workflow remain open.

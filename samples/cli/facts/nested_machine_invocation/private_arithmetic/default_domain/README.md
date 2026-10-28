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

The current baseline wrongly refuses all nine positives at the increment;
their nine twins refuse the same intended obligation. The trace proves the
bound under the private arrival, then loses the entire observation because
another arm's explicit Unit completion is treated as unsupported. Removing
the indexed write still fails; an explicit helper `requires` or a scalar
result avoids that traversal failure. Source, oracle and optimized-producer
identities are retained in the session's qualification evidence. Runtime
acceptance, pinned fixtures and the full product migration remain unfinished.

The customer is `source/psi`: its source/token/parser tables increment bounded
counts under caller guards. The product parser currently reports this same
data-store refusal at many such sites. The broader product declaration-model
migration remains open; no other-host native execution is claimed here.

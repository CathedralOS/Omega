# Source effect records

This representation contains the summaries produced during source settlement:

| Export group | Meaning and allowed operations |
| --- | --- |
| `capabilities` | Named capability verbs and their exact source occurrences; arena lookup and iteration. |
| `invocations` | Published, effective and realized call targets; lookup by declaration. |
| `operational` | Suspension/blocking summaries, cleanup dependencies, selector contexts and static bindings; immutable indexes and call traversal. |
| `service_reach` | Interned service sets and per-machine/state/call summaries; lookup only. |
| `ContractDependency`, `ConditionalCarryRequirement` | Normalized callback conditions and carry obligations; record composition, shape validation and canonical commitment. |

No operation activates source, infers a program's effects, publishes acceptance,
or runs another compiler stage. Producers populate these records. The independent
checker judges the program that carries them. `graphs::reach` owns the generic
finite-set dependency solver formerly embedded here; this crate no longer exports it.

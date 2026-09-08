# External-entry stack composition

[Body WCSU](storage.md#stack-demand-and-backing) does not include architectural
arrival or adapter storage. [External-root admission](../build/external_roots.md)
joins those demands under the selected boundary plan and installed facts.

## Domains and epochs

`EntryStack` chooses the execution-stack disposition, not the complete storage
history of entry. `EntryStackRealization` retains a finite complete set of
admissible arrival contexts, each with a finite ordered epoch sequence.

| Epoch field | Meaning |
| --- | --- |
| Stage | `Enter`, exactly one `Body`, or `Exit`. |
| Active domain | Stack domain used during that interval. |
| Occupancy by domain | Storage still live on each domain, including inactive ones. |
| Nesting allowance | Nested occurrences permitted during that interval. |

An epoch is maximal while active-domain interpretation and nesting allowance
remain unchanged. A hardware-atomic switch may start directly in its new domain;
a software switch ends one epoch and starts another. A direct generated entry
with no adapter may consist of its body epoch alone.

For each domain, calculate every admissible context/epoch's live occupancy plus
body WCSU only at `Body` on its execution domain, plus permitted nested demand.
Concurrent live storage adds with alignment. Sequential epochs and alternative
contexts combine by maximum, not sum. A nested root's relative `Interrupted`
domain resolves to the parent epoch's active domain, not a global stack ID.

`Nestable(maximum_depth)` bounds simultaneously live occurrences on one root
lineage, including the current occurrence; zero rejects. `Masked` permits no
nested occurrence covered by that policy. `ProviderDefined` must close to
equivalent finite evidence. Without a proven phase-specific restriction, apply
the declared nesting allowance conservatively to every epoch. Unresolved domains,
unknown contexts, missing endpoints, arithmetic overflow, and unbounded nesting
cannot produce a bounded composition.

## Arrival evidence

The complete context set derives from validated installation facts. Sealed
target rules derive architectural arrival; exact emitted stub instructions
derive generated adapter epochs. Direct generated entries rejoin their installed
Terminal stack closure and need no opaque-provider receipt.

An opaque adapter supplies an admitted complete context set bound to target,
exact installed entry, boundary plan, root, provider, and receipt. Its roster
must equal the realization's roster. A structurally valid subset or a bare
receipt is not completeness evidence. Generated, architectural, and opaque
origins retain separate provenance even when they compose into the same epochs.

Target installation reconstructs hardware stack choice from validated hardware
configuration and its complete context set; callers cannot substitute a claimed
frame size or independently selected stack. A fixed public disposition must
agree in every context. `ProviderSelected` must resolve in each context to the
interrupted domain or one exact provisioned domain. Conditional switching may
choose different domains only when the target's arrival rule establishes it.

The realization binds the boundary plan's strong commitment, exact installed
code, context-to-body-domain closure, body demand, preemption ceiling, and each
required provider receipt. Equal byte totals or compact report values cannot
hide changed premises. Publication and installation replay the same composition.

## Foreign callback entry

A target callback-entry plan may continue on the provider's stack under its
containment contract, preflight it against exact Omega WCSU plus the target's
entry/unwind margin with a protocol-valid unavailable outcome, or enter a
target-supported owned stack with sufficient provision. Preflight establishes
fit for the predicted Omega segment; it is not a new source crash cause.
Opaque foreign frames remain in the provider domain. A separated-stack profile
returns to that domain before another foreign call.

Backing, containment, and detection of a compiler-underestimated bound remain
separate from the source machine's crash contract. The stack realization alone
is neither a provisioned lease nor permission to execute.

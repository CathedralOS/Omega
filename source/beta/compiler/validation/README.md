# Beta compiler validation inventory

Everything here targets the one canonical edge:

```text
source/beta/compiler/beta_compiler.alpha
  -> source/beta/compiler/beta_compiler_bytecode.tape
```

Construction and diagnostics do not admit that edge. Exact checked
Alpha-source/tape correspondence remains open in `TASKS_BOOTSTRAP.md`.

| Retained owner | Bounded failure-detection role | Deletion condition |
| --- | --- | --- |
| `admission/bc-artifact-structure.alpha` and `.sh` | Independently decode the reachable canonical Alpha tape and reject malformed instruction framing, invalid direct targets, root `ret`, cross-procedure branches, overlapping procedure regions, and tape-hole overflow. The wrapper includes positive and negative fixtures and accepts an explicit candidate tape before persistence. It proves neither all-path termination nor language correctness. | Merge or delete when the exact checked source/tape certificate reconstructs and proves the same decoder, target, procedure-region, and capacity facts. Delete earlier if a future canonical layout invalidates its ordered-region model rather than weakening that model silently. |
| `admission/encoding/` | Run an Alpha-written, subject-bound two-pass reconstructor over the exact framed source and tape. It enforces the authoritative assembly grammar, compares every reconstructed opcode, operand, `db` byte, label fixup, and extent, permits no trailing input, and has source/tape/fixup/grammar mutation controls. Its 6,993-byte Alpha tape runs the 12-control gate in under one second on the development host. This is executable correspondence evidence, not a rooted derivation certificate. | Adapt it into the artifact-aware goal/ledger producer for the checked ground theorem, then merge or delete this status-only form when the rooted certificate absorbs its failure detection. Delete it instead if that adaptation would preserve a second assembly source of truth. |
| `differential/` | Compile a small set of symbolic Beta shapes with the canonical compiler, compare independently generated source/tape scalar terms, differentially pin each term on eight small-input trials, and have the rooted checker validate term equality plus mutation teeth. Its deliberately narrow observation and finite grounding are diagnostic only. | Delete when exact operational refinement covers the retained shapes, or when maintaining two symbolic recognizers is no longer economical. Individual-case deletion rules live in its README. |

The former ordinary-FOL seam was deleted: it proved hard-coded toy machines,
reconstructed no canonical source or tape byte, and could not be imported by a
later certificate. The source-only symbolic-loop gate and duplicated hand-built
Alpha/checker cases were also deleted because they never traversed this edge.
Git history is the archive for all three.

Run the retained checks directly:

```sh
sh source/beta/compiler/validation/admission/bc-artifact-structure.sh
sh source/beta/compiler/validation/admission/encoding/test.sh
sh source/beta/compiler/validation/differential/test.sh
```

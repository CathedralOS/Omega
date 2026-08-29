# Beta compiler validation inventory

Everything here targets the one canonical edge:

```text
source/beta/compiler/beta_compiler.alpha
  -> source/beta/compiler/beta_compiler_bytecode.tape
```

Construction and diagnostics do not admit that edge. Exact checked
Alpha-source/tape correspondence remains open in `TASKS_BOOTSTRAP.md`.
Owned validation that cannot be adapted into that correspondence is negative
value and must be deleted; prior implementation cost is not a retention reason.

| Retained owner | Bounded failure-detection role | Deletion condition |
| --- | --- | --- |
| `admission/` | Independently check reachable Alpha-tape structure while the exact checked source/tape certificate remains open. | Delete the structural check when the rooted certificate reconstructs and proves the same facts. |
The former ordinary-FOL seam was deleted: it proved hard-coded toy machines,
reconstructed no canonical source or tape byte, and could not be imported by a
later certificate. The source-only symbolic-loop gate and duplicated hand-built
Alpha/checker cases were also deleted because they never traversed this edge.
The final two-recognizer symbolic differential was deleted after drifting to
13/18 while returning success; a false-green parallel semantics is worse than
no diagnostic. Git history is the archive for all of them.

Run the retained checks directly:

```sh
sh source/beta/compiler/validation/admission/bc-artifact-structure.sh
```

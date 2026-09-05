# Omega pipeline crate disposition

[Ownership cleanup](ownership_cleanup.md) | [Repository layout](../repository_layout.md)

The visible selected program route is:

```text
Terminal Psi
  -> terminal-psi-to-abstract-operations
  -> abstract-operations-to-abstract-operations
  -> abstract-operations-to-target-operations
  -> target-operations-to-selected-instructions
  -> selected-instructions-to-register-homes
  -> register-homes-to-post-allocation-machine
  -> post-allocation-machine-to-post-allocation-machine
  -> post-allocation-machine-to-resolved-layout
  -> backend publication
```

The two X-to-X phases preserve their program representation. Empty selections
are identity execution; evidence records remain explicit. Native orchestration
belongs to `compiler/native-realization`, outside this directory.

| Owner | Disposition and invariant |
| --- | --- |
| `terminal-psi-to-abstract-operations` | Keep: admits immutable Terminal Psi and constructs the verified abstract program. |
| `abstract-operations-to-abstract-operations` | Keep: applies exact abstract rewrites and publishes independently checked current data. Replay is an internal consumer. |
| `abstract-operations-to-target-operations` | Keep: resolves abstract operations into target operations with explicit provider authority. |
| `target-operations-to-selected-instructions` | Keep: legalizes target operations and selects instruction forms. |
| `selected-instructions-to-register-homes` | Keep: owns selected analyses, allocation algorithms, and current allocated program admission. Callee-saved requirements are an internal calculation. |
| `register-homes-to-post-allocation-machine` | Keep: joins selected instructions, homes, effects, and register facts into a current physical machine plan. This is the representation-changing entrance, not an optimization phase. |
| `post-allocation-machine-to-post-allocation-machine` | Keep: exact machine rewrites operate on that physical program. Folding away the preceding entrance would hide the homes-to-machine transition. |
| `post-allocation-machine-to-resolved-layout` | Keep: owns layout-independent selected-form encoding, baseline layout, and explicit relaxation. Each internal producer retains its independent checker. |
| `post-allocation-machine-to-frame-layout` | Move calculation and admission into `backend/machine-emission/src/frame_layout`; raw geometry, save storage and spill records belong to representations. |
| `post-allocation-machine-to-selected-form-encoding` | Merge into the layout owner's `selected_form_encoding` module. Encoding and layout retain distinct admission checks without separate packages. |
| `selected-form-encoding-to-resolved-layout` | Rename to the complete post-allocation-to-layout owner. |
| `optimization-validation` | Dissolve: `optimization-unit` owns manifest and cycle data; `semantics/optimization-unit-semantics` owns reusable validity and replay; the abstract rewrite stage owns sealed context, cycle admission, candidate and publication checks. |
| `target-operations-to-assigned-target-operations` | Retain the existing alternate route until selected lowering covers its ranked, callback, and structural Unit behavior. Removing it during organization work would remove supported programs. |

Frame application, frame protocol, instruction-byte assembly, and final artifact
publication belong to backend owners. Target register-environment setup also has
independent allocation and emission consumers and stays in the backend.

Public current data belongs to representations. A sealed admission object is
different: it may retain the inputs required for independent replay, but ordinary
data access must not depend on traversing that history. Raw data, decoded records,
or recomputed identities alone must never grant publication authority.

The closed architecture roster prevents accidental sibling crates. It does not
prove physical-route convergence or completion of selected pre-Terminal Psi
optimization; those have separate behavioral acceptance conditions in the
ownership cleanup document.

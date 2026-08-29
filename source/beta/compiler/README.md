# Beta compiler

This directory owns the compiler artifact required by the Beta rung:

- `beta_compiler.alpha` is the canonical immediate-predecessor source;
- `beta_compiler_bytecode.tape` is the current platform-independent artifact;
- `validation/` contains only machinery that targets the canonical source or
  its emitted tape;
- `rebuild-artifact.sh` performs exact direct construction;
- `test.sh` owns the focused accepted/rejected language discriminators;
- `artifact_env.sh` installs the admitted tape into the selected Alpha seed.

Construction, testing, and evidence generation do not grant authority by
themselves. One Alpha source directly produces one exact Beta compiler tape.
The validation directory belongs here because the artifact
being admitted owns its validation. Bounded diagnostics can expose regressions,
but acceptance must ultimately terminate in the independently
rooted checker under `source/alpha/checker/`.

## Persisted artifact

`beta_compiler_bytecode.tape` is emitted directly from
`beta_compiler.alpha`. Its complete construction lineage is:

```text
audited Alpha seed + Alpha-written assembler
  -> beta_compiler.alpha
  -> beta_compiler_bytecode.tape
```

`rebuild-artifact.sh --check` reconstructs the tape and compares it
byte-for-byte without changing the repository. `artifact_env.sh` stamps it into
the selected audited Alpha seed. No Beta self-host, textual Alpha output, or
second assembler invocation participates.

`validation/admission/encoding/test.sh` independently replays the authoritative
two-pass assembly relation in Alpha against the exact framed source and tape.
It is a bounded reconstructor with mutation controls; it does not admit the
edge until its subject-bound judgment is expressed as a derivation checked by
`source/alpha/checker/`.

The committed artifact is 20,977 bytes with SHA-256
`1911fc4f9667081ca96559ee970f07c3359f225c1177b5ed889d55c05a059f0f`.
The byte comparison, not the convenient digest, governs repository identity.

## Retention inventory

| Retained child | Bounded role | Deletion condition |
| --- | --- | --- |
| `validation/` | Exact artifact structure and exact Alpha encoding reconstruction for this compiler edge. | Delete a diagnostic when a stronger artifact-bound proof subsumes it; delete the subtree when direct checked refinement subsumes every retained check. |

Root files are the one compiler source, one Alpha-tape artifact, one artifact
loader, one exact reconstruction entry point, and one focused language gate.
No separate cold-start, self-host, generated-artifact, or publication owner is
retained.

# Beta compiler

This directory owns the compiler artifact required by the Beta rung:

- `beta_compiler.alpha` is the canonical immediate-predecessor source;
- `beta_compiler_bytecode.tape` is the current platform-independent artifact;
- `cold-start/` owns direct construction and focused compiler tests;
- `validation/` contains only machinery that targets the canonical source or
  its emitted tape;
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

`cold-start/rebuild-artifact.sh --check` reconstructs the tape and compares it
byte-for-byte without changing the repository. `artifact_env.sh` stamps it into
the selected audited Alpha seed. No Beta self-host, textual Alpha output, or
second assembler invocation participates.

The committed artifact is 20,977 bytes with SHA-256
`1911fc4f9667081ca96559ee970f07c3359f225c1177b5ed889d55c05a059f0f`.
The byte comparison, not the convenient digest, governs repository identity.

# Beta compiler validation

This directory owns validation of the exact `bc.beta` source and persisted
`artifacts/bc.tape`. It contains three independently runnable layers:

| command | role |
| --- | --- |
| `sh bc-artifact-structure.sh` | Alpha-rooted instruction framing, reachable direct targets, procedure regions, call/return shape, and seed payload bounds |
| `sh bc-block-control.sh` | canonical whole-source/artifact maximal-observation reconstruction for `B_bc1` |
| `sh refinement.sh` | proof-carrying instruction refinement for the supported symbolic program families |

`refinement-cert-diamond.sh`, `symbolic-loops.sh`, and `ownership-test.sh`
provide bounded independent cross-checks. They are not additional compiler
stages.

## Canonical whole-compiler obligation

`bc-block-control.sh` has one mode and one input format:

```text
u32_le source_length
exact bc.beta bytes
u32_le tape_length
exact bc.tape bytes
canonical control witness
canonical call-bounds witness
```

The shell owns and packages the exact repository source and tape. The Python
mapper and call-bounds analyzer are untrusted witness producers; they cannot
substitute either subject. Four format-level negative controls retain the same
canonical checker programs while changing source bytes, tape bytes, framing,
or witness extent. All must reject with empty output.

The canonical conjunction is split across fourteen Alpha executables because
the audited seed accepts at most 262,140 tape bytes. Responsibility-specific
`.alpha` modules are concatenated only into those bounded programs. The
programs cover source/artifact control and effect custody, frame and stack
shape, memory-site classification, expression and statement composition,
bounded emitters, parsing/resource outcomes, and the final greatest-fixed-point
maximal observation. The final ROOT tape is 79,003 bytes for the current exact
subjects, SHA-256
`33a15b13586df64bcbe714adf517f35cf3e312c1f70c9971a7e5fd3c971ca40a`.

Historical focus modes, per-mutation checker-source permutations, local green
receipt caches, and mutation-only mapper outputs were removed. Git history is
their archive. The retained command reconstructs every canonical prerequisite
on every run and completes in tens of seconds rather than depending on cached
host state.

Run it directly from any working directory:

```sh
sh source/beta/compiler/validation/bc-block-control.sh
```

## Authority and limits

The ROOT conjunction is unique lower-rung executable evidence for exact
maximal-observation equality over finite `B_bc1` inputs, including typed
resource outcomes and coinductive divergence. It is not yet a certificate
accepted by the universal `source/alpha/checker/artifacts/check.tape`:
the current proof language has no encoded Alpha/Beta simulation relation or
settled coinduction rule. Accordingly the short chain manifest continues to
disclose complete Beta source/artifact admission as open rather than promoting
this executable reconstruction by pedigree.

`refinement.sh` separately derives Beta and Alpha symbolic meanings for curated
and generated program families and asks the rooted checker to validate their
equivalence. See [`REFINEMENT.md`](REFINEMENT.md) for that narrower claim and
its exact unsupported cases.

The shared parser and concrete reference interpreter live under
`source/beta/reference/`. Validation may consume them as untrusted
reconstruction or differential machinery; neither defines Beta. Beta's
canonical runtime meaning remains [`../../SEMANTICS.md`](../../SEMANTICS.md).

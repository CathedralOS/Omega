# PR1189 execution evidence

Reviewed source: `c23b89964270e19ce1093099957557ff4f32ab37`.
Fixed baseline: `bfcffe6bcfcedb6fd60713e223408566c6c4e10d`.
Optimized repaired producer: `a17219c1def10c8448ee4d0aca8cab6c0dc9eac1`.
Archive SHA-256: `0882fa22e528d8381bc80ff47861eb25d2774d62d2ae0f8a76190aa768451c4f` (1278648 bytes).

The archive retains original refusal receipt bf8235d4abc64a2e8676b940ee8f3bd9,
canonical implementation contexts, frozen probe source and oracles, raw historical
and fresh observations, optimized producer digests/profiles, source/oracle hashes,
scoped corpus records, exact-candidate independent review, and both exported
corpus source bundles. Compiler executables are retained in the local workbench;
the archive records their immutable identities and build logs.

Extract `evidence.tar.gz` into a fresh directory. `manifest.json` binds each file.
The bundle for7766281105f8 contains the earlier06b2fbeddef source as ancestry;
both original exported bundles are retained unchanged. In an Omega clone that
contains the fixed baseline, `git bundle verify PATH` verifies each bundle and
`git fetch PATH refs/corpus-observations/FULL_SHA` restores its source commit.

Actual Linux x86-64 observations: ten borrowed-input positives and the existing
owned-input prerequisite agree at native/Terminal exit0; thirteen new controls
and five retained safety controls reject as intended. Corrupting the counter
oracle traps in both engines. All three affected crate checks pass. Clippy is
non-green on standing source-attributed style lints; full logs remain included.
Other hosts and wider NESTED-MACHINE-UNIFICATION acceptance remain open.

The evidence tag is an artifact record, not compiler source or a release tag.

# Indexed-handback qualification

Qualification of the `saved_observation_snapshot/indexed_handback` fact row (PR #1074,
"psi: preserve indexed observations through normal call completion") on Linux x86_64,
target `linux_x86_64`, optimization level 1.

- Base `a91a8de86514`; implementation source `755b31d334ff`. After GitHub reported a
  `normal_returns.rs` conflict, the result was re-measured on main `02d6d92b37d4` with source
  `da19ac492c58` (7,853 compiler and library inputs unchanged).
- Sample gate: 11 positive places ran native and Terminal with exit 0 / 0; 15 intended rejections.
- Corpus, first run: 31 runtime fixtures at 0 / 0, 43 invalid fixtures rejected (41 with the expected
  reason). It recorded 26 new golden rows (11 runtime, 11 checked, 15 invalid); every original golden
  byte was retained.
- Corpus, after integration: runtime leg 33 fixtures (31 built, 2 rejected), 0 lost successes, 0 wrong
  reasons; reject leg 43 rejected, 2 with a different reason, 0 failures not rejected.
- Open: other-host execution, the scalar-range migration and product/compiler parity.

Raw data (manifest, evidence archives, two source bundles) was removed from the tree. It is at
[this folder at commit `6e5131c510b9`](https://github.com/CathedralOS/Omega/tree/6e5131c510b9e6227b4592652c9a5a39e350a970/wiki/drafts/measurements/indexed_handback_2026_10_31),
starting with its [manifest.json](https://github.com/CathedralOS/Omega/blob/6e5131c510b9e6227b4592652c9a5a39e350a970/wiki/drafts/measurements/indexed_handback_2026_10_31/manifest.json).

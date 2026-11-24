# Atlas packet unit_020: native stage 14 object plans and the dynamic ELF writer

Receipt in progress. Claims are added as they are settled.

Evidence baseline: `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. Survey revision
`4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Gap packets `unit_020_gap_007` and `unit_020_gap_008` own the
ranges listed for unit 20 in `second_pass_ranges.tsv`; this packet covers only the ranges `records/unit_020.md`
read.

Paths: `S14` = `source-rust/subsystems/compiler/psi-to-native/pipeline/14_placed-functions-to-objects/src`,
`ELF` = `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-elf/src`,
`R` = `records/unit_020.md`, `D11` = `digests/11-native-stages-11-15.md`.

## Claims

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `R` `S14/target_matrix.rs` §3 and §9: `(Aarch64, Coff)` returns `None`, and callers map it to `NonCanonicalTarget`. | `S14/target_matrix.rs:36-58` | [#5246](https://github.com/CathedralOS/Omega/pull/5246) added the `windows_arm64` target. `(Aarch64, Elf \| Coff)` now returns `main`, `.text`, alignment 4. The fixture `windows_arm64_objects_use_the_coff_names_and_aarch64_alignment` pins the row. Every `(Architecture, ObjectFormat)` pair now has a row. | **Fixed** in #5246. The header comment (`:1-7`) still says five pairs and that `(Aarch64, Coff)` returns `None`. |

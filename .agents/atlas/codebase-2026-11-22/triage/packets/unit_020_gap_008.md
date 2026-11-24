# Atlas packet unit_020_gap_008: ELF load placement and procedure linkage in native stage 15

In progress. Early receipt: the first three claims below are settled. The rest of the record is still under review.

Evidence baseline: `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. Survey revision
`4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Assigned slice: area 11 only, the four ranges in
`units/gap_008.tsv`.

Paths: `ELF` = `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-elf/src`,
`LP` = `ELF/load_placement.rs`, `PL` = `ELF/procedure_linkage.rs`, `R` = `records/unit_020_gap_008.md`,
`D11` = `digests/11-native-stages-11-15.md`.

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `R` LP 471-585 §6 and five-things 5; `D11:98`. `checked_align` exists twice with different rules for non-power-of-two alignment. | `LP:1051-1066`; `ELF/section_headers.rs:289-297` | Same claim as `triage/packets/unit_021.md` claim 2 (PR #5412): no upstream descriptor supplies a non-power-of-two or zero alignment to a non-null row. | **Duplicate** of unit_021 claim 2; **intended**. Duplication lead only. |
| 2 | `R` LP 471-585 §5. A zero-size target never satisfies the ADRP page-delta check. | `LP:451-461`, `LP:492-509` | The caller already refuses any constraint whose target has `byte_size == 0` ("procedure fixup target section has no bytes") before matching the kind. The targets are `.plt` and `.got.plt`, which always hold the PLT header and three GOT header words. | **Intended.** The `false` return refuses; it never accepts. |
| 3 | `R` LP 471-585 §9. `align_for_region` defaults to alignment 1 when no row names the region. | `LP:557-571`; regions from `ELF/section_headers.rs:150-194`, `:275-283` | Each of the 13 non-null roster rows is classified into one region from its fixed flags. The fixture at `section_headers.rs:375-420` pins rows 1-7, 10, 11 to read-only, 8 to read-execute, 9 and 12 to read-write and 13 to file-only, and requires a nonzero span in each of the four regions. So every region has rows and the default is unreachable. | **Intended.** |

# Atlas packet unit_020_gap_008: ELF load placement and procedure linkage in native stage 15

Packet complete. Every correctness claim in `records/unit_020_gap_008.md` has a disposition below, and none
stays open. The review found no wrong result, accepted invalid input or lost data in the four gap ranges. The
error messages the record flagged (832 bytes, twelve file offsets, eleven virtual addresses) were already
corrected by [#5376](https://github.com/CathedralOS/Omega/pull/5376). Each place the record called "open"
either cannot be reached or ends in a refusal.

Running the owning `image-elf` fixtures found an unrelated build break on `main`. The break is outside this slice
and is listed under "Build break found while running the fixtures". [#5427](https://github.com/CathedralOS/Omega/pull/5427)
repaired it while this review was in progress. This PR changes only the ledger.

Evidence baseline: `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`, read at
`a0152d26` (two later commits touch neither file). Survey revision `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.
Since the survey, [#5376](https://github.com/CathedralOS/Omega/pull/5376) changed the count messages in
`load_placement.rs` and `section_headers.rs`. [#5357](https://github.com/CathedralOS/Omega/pull/5357) made the
fingerprint's `target_tag` call `TargetProfile::identity_tag()`. Current line numbers in `load_placement.rs`
after line 1040 are about 9 lower than the record's. Assigned slice: area 11 only, the four ranges in
`units/gap_008.tsv` (`load_placement.rs` 471-585 and 986-3379, `procedure_linkage.rs` 581-1954 and
2435-3466).

Paths: `ELF` = `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-elf/src`,
`LP` = `ELF/load_placement.rs`, `PL` = `ELF/procedure_linkage.rs`, `SH` = `ELF/section_headers.rs`,
`DT` = `ELF/dynamic_table.rs`, `R` = `records/unit_020_gap_008.md`, `D11` = `digests/11-native-stages-11-15.md`.

## Open claims

None.

## Resolved claims

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `R` LP 471-585 §6 and five-things item 5; `D11:98`. `checked_align` exists twice with different rules for non-power-of-two alignment. | `LP:1051-1066`; `SH:289-297` | Same claim as claim 2 in `triage/packets/unit_021.md` ([#5412](https://github.com/CathedralOS/Omega/pull/5412)). No upstream descriptor gives a non-null row an alignment of 0 or one that is not a power of two. Fixtures: `LP` `checked_arithmetic_rejects_overflow_and_invalid_alignment` (`checked_align(7, 3)` is refused) and `SH:503` (alignment 0 returns the value). | **Duplicate** of unit_021 claim 2; **intended**. Duplication lead only. |
| 2 | `R` LP 471-585 §5. A zero-size target never satisfies the ADRP page-delta check. | `LP:451-461`, `LP:492-509` | Before it matches the constraint kind, the caller refuses any target with `byte_size == 0` ("procedure fixup target section has no bytes"). The targets are `.plt` and `.got.plt`, which always hold the PLT header and three GOT header words (`PL:3081-3091`). | **Intended.** The `false` result refuses the layout; it never accepts one. |
| 3 | `R` LP 471-585 §9. `align_for_region` defaults to alignment 1 when no row names the region. | `LP:557-571`; regions from `SH:150-194`, `:275-283` | Each of the 13 non-null roster rows gets one region from its fixed flags. The fixture `both_targets_pack_exact_aligned_rows_without_absolute_authority` (`SH:373-420`) pins rows 1-7, 10 and 11 to read-only, row 8 to read-execute, rows 9 and 12 to read-write and row 13 to file-only. It also requires a nonzero span in all four regions. Every region therefore has rows, and the default is never used. | **Intended.** |
| 4 | `R` LP 986-1294 §4. `target_tag` hashes nine `TargetProfile` values, including ones ELF never uses. | `LP:1047-1049` | The function now returns `target.identity_tag()` ([#5357](https://github.com/CathedralOS/Omega/pull/5357)). It only feeds `non_authoritative_layout_compatibility_fingerprint`, which tests compare for equality after replay (`placement_is_deterministic_and_target_bound`). Earlier stages admit only the two Linux targets (claim 12). | **Intended.** A fingerprint input, not a target selector. |
| 5 | `R` LP 986-1294 §6, "unsure" item 1, five-things item 4; `D11:100`. `ElfPlacedDynamicSectionKind` and `ElfDynamicRosterSectionKind` are one type, and `public_section_kind` is an identity function. | `LP:1102-1106`; `SH:2547` | `SH:2547` declares `pub(crate) type ElfDynamicRosterSectionKind = crate::ElfPlacedDynamicSectionKind`, so they are one type. `public_target` (`LP:2995`) does the same for `ElfDynamicAddressTarget`. | **Intended** (confirmed). The identity functions are a cleanup lead with no effect on behavior. |
| 6 | `R` LP 986-1294 §9, LP 1663-1858. The fingerprint is `non_authoritative`; it detects drift, not tampering. | `LP:987-1045` | Acceptance comes from the replay in `validate_candidate`, not from the fingerprint. The corruption fixtures (`geometry_resolution_policy_and_identity_drift_reject_with_custody`) are refused by the replay, and each refusal returns the upstream plan. | **Intended.** |
| 7 | `R` LP 1861-2237 §5, five-things item 3; `D11:269`. The messages say "exact 832-byte", "exactly twelve file offsets" and "exactly eleven virtual addresses", but the constants give 896, 13 and 12. | `LP:1947-1964`, `:2099-2120` | [#5376](https://github.com/CathedralOS/Omega/pull/5376) replaced these with `require_count`, which reports the actual and the required count. The remaining text "exactly twenty-five fixups and resolutions" matches `PLACEMENT_COUNT` = 25. | **Fixed** in #5376. Duplicate of the "11, ELF count diagnostics" row in `triage/07-13-psi-and-native.md`. |
| 8 | `R` LP 1861-2237 §9. `address_by_row[..].unwrap_or(0)` and `offset_by_row[..].unwrap_or(0)` require a row with no ledger entry to hold 0. | `LP:2122-2153` | The replay requires exactly 13 file offsets and 12 addresses, refuses a duplicated row field, and requires that row 0 has neither field and row 13 has an offset but no address. Across 14 rows, that leaves no other row without an entry. Fixture: `ledger_has_exact_field_split_and_file_only_section_name_table`. | **Intended.** Agrees with the record's own trace. |
| 9 | `R` LP 2509-3094 §9; `D11:186`. `expected_targets` takes the version-aware count from the semantic plan, and `validate_contents` checks the same plan's `address_obligations`. A plan that is wrong on both sides would pass. | `LP:2538-2547`, `:2805-2815`; `DT:1053-1055`, `:1205-1210`, `:1287-1297`, `:1403`, `:1576-1640` | `has_version_requirements()` reads `verneed` from the sealed import-section plan. The dynamic tag stage derives two extra obligations only when `verneed` is non-empty. Its own replay, `validate_address_obligations(contents, !structural.verneed.is_empty())`, refuses any other list. The import-section stage replays `verneed` from its bytes (`ELF/import_sections.rs:641-643`). Fixture: `independent_replay_rejects_missing_duplicate_or_misdirected_address_obligations` (`DT:2021`). | **Intended.** The sealed upstream plan cannot give a wrong count. |
| 10 | `R` LP 2509-3094 §9. `.expect("filtered allocated ELF dynamic target")` cannot fail. | `LP:2763-2786` | The filter just before it requires `section.virtual_address().is_some()`. | **Intended.** |
| 11 | `R` LP 2509-3094 §5, "unsure" item 3; `D11:249`. `ElfDynamicAddressApplicationKind` is `#[cfg(test)]` only. | `LP:2549-2553`, `:2779`, `:2876-2883`; `DT:559-563` | Production records `kind_tag = fixup.kind as u8` from the single-variant `ElfDynamicPayloadFixupKind`, and the replay requires `kind_tag == ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress as u8`. The test-only enum gives tests a typed view of that tag (`LP:2604-2606`). Fixture: `missing_duplicate_reordered_and_ledger_field_drift_reject_with_custody` flips `kind_tag`. | **Intended.** |
| 12 | `R` PL 1129-1953 §4-5, five-things item 2; `D11:141`. The descriptor and template stages accept only `LinuxX64` and `LinuxArm64` and return an error for any other `TargetProfile`. | `PL:216-234`, `:166`, `:448`, `:1328-1337`, `:1985-1997`, `:2720-2724` | The dynamic ELF lane serves the two Linux hosted targets. macOS uses Mach-O and Windows uses PE. The relocation plan's derive and replay both call `target_relocation_spec`, which refuses every other profile. So no later stage can receive one, and each later stage refuses it again. | **Intended** (stated scope). |
| 13 | `R` PL 2694-3061 §9. `validate_encoded_resources` reaches `unreachable!` for a non-Linux target, and `validate_contents` calls it before its own target match. | `PL:2715-2767`, `:2851` | By claim 12, a `ValidatedElfProcedureLinkageRelocationPlan` exists only for the two Linux targets. The template stage's `derive_contents` also refuses other targets before the replay runs (`PL:1985-1997`). Neither `unreachable!` can be reached. | **Intended.** The order of the checks could be swapped for clarity, but behavior is the same. |
| 14 | `R` PL 2694-3061 §5, §8-9; `D11:187`. `validate_semantic_target` accepts `FutureDynamicSection` and `PltHeader` without a check. | `PL:3001-3033`, `:2728-2733` | The replay first requires `contents.fixups == derive_fixups(target, linkage.contents())`, so a fixup cannot name a different target than the derivation gives. Fixture: `independent_replay_rejects_every_template_fixup_constraint_and_identity_corruption`. | **Intended.** Agrees with the record's trace. |
| 15 | `R` PL 2435-2463, "unsure" item 2. The `Page21` and `Load64Low12` arms of `derive_constraints` were not read. | `PL:2420-2450` | `Aarch64Page21` gives `Aarch64PageDelta21` and `Aarch64Load64Low12` gives `Aarch64Load64Low12Aligned`. The match lists every kind and has no `_` arm. The replay requires `constraints == derive_constraints(&fixups)` (`PL:2738-2741`), and `LP:466-486` checks each of the four kinds. | **Intended.** The inference in the record is now confirmed. |
| 16 | `R` PL 1129-1953 §9. `unreachable!("descriptor count checked above")` follows the count check. | `PL:1461-1474` | The count check returns before the match. | **Intended.** |

## Build break found while running the fixtures

| Finding | Source | Reproducer and trace | Outcome |
| --- | --- | --- | --- |
| The native compiler does not compile on `main`. [#5393](https://github.com/CathedralOS/Omega/pull/5393) added `Proposition::DeclarationIdentity`, but `encode_proposition` in the native optimization-unit identity still matches the old variant list. | `source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/identity.rs`, `proposition_encoding::encode_proposition` | `CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx test -p image-elf --lib < /dev/null` on `a0152d26` fails with `error[E0004]: non-exhaustive patterns: &semantic_vocabulary::Proposition::DeclarationIdentity(_) not covered` at `identity.rs:267`, exit 101. #5393 is in the baseline `24ad2700`. When this was found, no open PR touched the crate. | **Fixed** in [#5427](https://github.com/CathedralOS/Omega/pull/5427), which merged during this review. That change writes tag 23, the portable encoder's tag, then each clause value's carrier and operand and the left and right positions. Published Psi still refuses the form (`psi/artifact/wire/src/foundation_validation.rs:6080`). Fixture: `declaration_identity_commits_its_cited_declaration`. An equivalent local repair was written and tested, then dropped as a duplicate. |

## Inventory and duplication leads

These record entries describe shape or repeated code. None claims a wrong result. Each is a lead for a
refactor, not a defect.

| Original entry | Current source | Disposition |
| --- | --- | --- |
| `R` LP 471-585 §7, LP 1663-1858 §7, PL 2694-3061 §7: grep counts for `retained_target`, `retained_image`, `aarch64_page_delta_covers_extent`, `into_relative`, `validate_fixup_coverage` | `LP:492`, `:512`, `:530`; `PL:3035` | Inventory. The record notes that the counts include definitions and tests. Each named function has at least one production caller (`LP:478`, `:1810`, `PL:2737`). |
| `R` LP 986-1294 §6, PL 2465-2533: the FNV fingerprint recipe repeats in each stage | `LP:987`, `:2218`, `:3072`; `PL:1560`, `:2465` | Duplication lead. Each stage hashes its own contents. |
| `R` LP 1663-1858 §6, five-things item 1; `D11:97`: the derive / `Candidate` / `seal_stage!` pattern, with rules written twice (`PL:1339-1386` vs `:1481-1530`) | `ELF/dynamic_executable.rs:355-400` | Intended design: each dynamic stage replays its own rules independently. Changing a rule takes two edits, and the corruption fixtures catch any edit made to only one side. |
| `R` LP 1861-2237 §6, PL 2694-3061 §6; `D11:99`: two quadratic overlap checks | `LP:2189`; `PL:3035` | Duplication lead. |
| `R` LP 2509-3094 §6; `D11:100`: section index per kind appears three times | `LP:1086-1101`, `:2999-3043` | Checked: `target_section_index` and `target_section_kind` agree with the enum discriminants for all nine targets, and `application_for_fixup` requires both to match the placed section (`LP:2763-2770`). Duplication lead. |
| `R` LP 1663-1858 §5: `final_image_layout()` exposes only text, data and bss | `LP:1705-1711` | Intended. `image::FinalImageLayout` has exactly these three fields. The Mach-O and PE writers expose the same shape. |
| `R` LP 1296-1661, LP 2239-2506, LP 3096-3378, PL 581-1127, PL 1590-1952, PL 3132-3465: test groups use only `LinuxX64` and `LinuxArm64`, with `_ => unreachable!()` in fixtures | test modules | Intended. These are the only targets the lane admits (claim 12). |
| `R` PL 3063-3130: PLT stubs are lazy-binding shapes, and the GOT template must be all zero | `PL:2852-2855`, `:3081-3115` | Description only. The record makes no correctness claim, and this review did not trace loader binding. |

## Validation

- Before the repair, on `a0152d26`: `CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx test -p image-elf --lib < /dev/null` failed to
  compile `optimization-unit` with E0004 (`Proposition::DeclarationIdentity` not covered), exit 101.
- On `main` at `08a667b1`, which includes #5427, plus this ledger:
  `CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx test -p image-elf --lib < /dev/null` gives 138 passed, 0 failed. These are
  the owning fixtures cited above.
- No code is changed. No global compiler build, sample gate or end-to-end native run was performed.

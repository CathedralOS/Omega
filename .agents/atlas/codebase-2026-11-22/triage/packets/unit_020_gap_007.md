# Atlas packet unit_020_gap_007: dynamic ELF image stages (area 11)

Packet complete. Every claim in `records/unit_020_gap_007.md` has a disposition below. One claim was a real
validator gap and is fixed in this PR: the resolved procedure-linkage replay never compared the `.rela.dyn`
bytes with their upstream row. A changed `r_info` or `r_addend`, or an appended byte, passed replay and would
have reached the assembled file unchanged. No claim stays open.

While testing, `image-elf` did not compile on `main`: #5393 added `Proposition::DeclarationIdentity` and the
native `optimization-unit` identity encoder had no arm for it. This branch first carried its own arm, but
#5427 and #5443 landed the same repair on `main`. After merging `main`, the branch keeps `main`'s encoder and no
longer touches `optimization-unit`.

Evidence baseline: `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`; the branch started at `a0152d26`.
`units/gap_007.tsv` assigns ten ranges in `image-elf`; the record read all of them. Paths: `E` =
`source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-elf/src`,
`R` = `records/unit_020_gap_007.md`.

## Fixed

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `R` §6 item 9 and five-things item 4: the unchanged-bytes loop skips `GeneralRelocation`, so `.rela.dyn` bytes outside an `r_offset` field are never compared with upstream. `R` "Unsure about" item 1 asks whether another stage covers them. | `E/file_assembly.rs` `resolved_procedure_linkage::validate_contents` (loop after the per-application checks) | The length check before the loop also omitted `.rela.dyn`. No later stage covers it: the assembled-file stage copies section 11 from `general_relocation_bytes()` (`E/file_assembly.rs:593-606`) and replays against that same copy. Fixture `general_relocation_bytes_outside_r_offset_or_length_drift_reject_with_custody` builds a data-slot import on both Linux targets, flips byte 8 (`r_info`) or byte 16 (`r_addend`), or appends a byte, and refreshes the fingerprint. Before the fix, replay accepted the flipped `r_info` (`unwrap_err()` on an `Ok` value). | **Fixed.** `GeneralRelocation` joins the unchanged-bytes loop, whose `validate_unchanged_bytes` also requires equal length. |

## Intended or not reachable

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 2 | `R` §10 item 9: `placed.virtual_address.unwrap_or(0)` at `E/load_placement.rs:362` might let a file-only `.interp` or `.dynamic` alias a program header at address 0. | `E/load_placement.rs:354-409`, `:910-925` | `validate_candidate` requires `image_base == IMAGE_BASE` (`0x400000`, `E/lib.rs:85`) and the sections to equal an independent re-derivation (`:920-923`). PT_INTERP must lie inside the read-only load's memory and PT_DYNAMIC inside the read-write load's memory (`:368-409`). Both loads start at or above `0x400000`, so a header at address 0 with nonzero size fails containment. | **Intended.** No fail-open path. |
| 3 | `R` "Unsure about" item 4 and §7c item 5: `PltLazyTail` adds the x86 lazy-tail offset 6 even on AArch64. | `E/file_assembly.rs` `semantic_target_address`; producer `E/procedure_linkage.rs` `append_data_fixups` (about `:2303-2340`) | Only the `LinuxX64` arm emits `PltLazyTail`. The `LinuxArm64` arm points every GOT slot at `PltHeader`. | **Intended.** AArch64 cannot reach the x86 constant. |
| 4 | `R` "Unsure about" item 3: the deferred-constraint match arms after `E/load_placement.rs:470` were not read. | `E/load_placement.rs:468-491` | Four arms, one per `ElfProcedureLinkagePlacementConstraintKind`: x86 signed 32-bit span; AArch64 branch-26 needs a 4-byte-aligned source and a span below 2^27 bytes; ADRP needs both ends of the target within ±2^20 pages (`aarch64_page_delta_covers_extent`, which refuses a zero-size target); low-12 load needs an 8-byte-aligned target. Each matches its architecture's encoding range. | **Intended.** |
| 5 | `R` five-things item 5 and §2 item 6: the assembled-file fingerprint (`E/file_assembly.rs:717`) starts from `Fnv1a::new()` rather than `checked::stage_fingerprint`. | `E/file_assembly.rs:717-736`; `E/dynamic_executable.rs:137-142` | The fingerprint is non-authoritative. `validate_contents` makes the real comparison, and the zero and drift fixtures (`E/file_assembly.rs:1152-1168`) cover the fingerprint check. The only difference is the missing stage tag. | **Intended** (style). It is a consistency lead, not a wrong result. |
| 6 | `R` §1a item 5, §3a item 5, §4 item 5, §10 item 5, five-things items 2 and 3: the ELF shape is hard-coded (5 program headers, 14 sections, 113-byte name seed, section indices 8-12, two Linux targets). | Cited lines in `R` | Every stage replays these constants against an independent derivation and refuses any other shape. `validate_relocation_closure` and `procedure_linkage_header_size` refuse targets other than `LinuxX64` and `LinuxArm64`. | **Intended.** The fixed shape is a design choice; adding a section changes literals and tests together. |
| 7 | `R` §1a item 9, §1b item 9, §2 item 9, §3a item 9, §4 item 9, §7c item 9: no fail-open seen. | Cited lines in `R` | The record found none, and this review found none. | **Intended.** |

## Duplicate code and unused API

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 8 | `R` §1b item 6, §3a item 6, §6 item 6: three overlap checks (pairwise loop `E/dynamic_table.rs:710-729`, `validate_nonoverlapping_applications`, occupied bitmap `E/file_assembly.rs:479-510`) and two "unowned byte" checks. | Cited lines | Each copy is a validator that compares against the producer's output, so a disagreement refuses. | **Duplicate** code; a code-quality lead, not a correctness defect. |
| 9 | `R` §4 item 6 and §10 item 6: header and PT_LOAD field checks repeat between `E/file_assembly.rs:1484-1531` and `E/load_placement.rs:40-111`. | Cited lines | Both refuse on mismatch. | **Duplicate** code. |
| 10 | `R` §1b item 7, §7b, §7c items 6-7: single-variant `ElfDynamicPayloadFixupKind`; identity functions `public_kind` and `public_section_kind`; test-only accessors. | Cited lines | No behavior depends on them beyond returning their argument. | **Duplicate** code / unused API; no wrong result. |
| 11 | `R` §1a item 7, §2 item 7, §3a item 7, §10 item 7: getters such as `descriptor_count` and helpers such as `copy_fragment` have one caller or test-only callers. | Cited lines | Usage observation only. | **Intended**; no wrong result. |
| 12 | `R` "Unsure about" item 2: `plan_stage!`, `seal_stage!` and `Candidate` were read only by use. | `E/dynamic_executable.rs:292-349` | `seal_stage!` builds `validate_candidate` from the stage's `validate_contents` and the fingerprint comparison (nonzero where declared). It does not re-derive contents, so each stage's `validate_contents` must cover every field. That is why claim 1 was a real gap. | **Intended**; the macro does what the record inferred. |

## Main build break found during testing

| Cause | Source | Trace | Outcome |
| --- | --- | --- | --- |
| #5393 (`4045f5ea`) added `Proposition::DeclarationIdentity(ClauseIdentity)`. The native optimization-unit identity encoder's `match` had no arm for it. | `source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/identity.rs` `proposition_encoding::encode_proposition` | `mbx test -p image-elf` on `a0152d26` failed with `error[E0004]: non-exhaustive patterns: Proposition::DeclarationIdentity(_) not covered` at `identity.rs:267`. | **Duplicate**: fixed on `main` by #5427 and #5443 while this branch was open. The branch dropped its own arm when it merged `main`. |

## Validation

- `mbx test -p image-elf --lib general_relocation_bytes_outside` before the fix: 1 failed (replay accepted the
  flipped `r_info`). After: `mbx test -p image-elf --lib`: 139 passed, 0 failed.
- After merging `main` at `21283d61`: `mbx test -p image-elf`: 139 passed; `mbx clippy -p image-elf
  --all-targets -- -D warnings`: clean.
- I searched open PRs (`gh api search/issues`, `image-elf`, `rela.dyn`, `DeclarationIdentity`,
  `optimization-unit`). The only related one, #5412 (unit 021), adds a different packet file and changes no code.

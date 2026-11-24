# Atlas packet unit_020_gap_007: dynamic ELF image stages (area 11)

Early receipt; work in progress. The record's first open fail-open note is confirmed by reading: the
resolved procedure-linkage replay never compares `.rela.dyn` bytes with their upstream row, so a changed
`r_info` or `r_addend`, or an extra byte, would pass replay and reach the assembled file. A repair is in
progress on this branch. The second open note (`unwrap_or(0)` for PT_INTERP/PT_DYNAMIC) is intended.

Evidence baseline: `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. Paths: `E` =
`source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-elf/src`,
`R` = `records/unit_020_gap_007.md`.

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `R` §6 item 9 and five-things 4: `GeneralRelocation` is absent from the unchanged-bytes loop. | `E/file_assembly.rs:2219-2337` | In progress. | **Reproduced by reading**; repair in progress. |
| 2 | `R` §10 item 9: `placed.virtual_address.unwrap_or(0)` at `E/load_placement.rs:362` would let a file-only `.interp`/`.dynamic` alias a program header at address 0. | `E/load_placement.rs:354-409`, `:910-925` | `validate_candidate` requires `image_base == IMAGE_BASE` (`0x400000`, `E/lib.rs:85`) and the candidate's sections to equal an independent re-derivation (`:920-923`). PT_INTERP must lie in the read-only load's memory and PT_DYNAMIC in the read-write load's memory (`:368-409`). Both loads start at or above `0x400000`, so address 0 with a nonzero size fails the containment checks. | **Intended.** No fail-open path. |

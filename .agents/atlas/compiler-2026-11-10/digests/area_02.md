# Area 2 digest: Back end: native artifacts, calling conventions, executable images

Source records: `unit_002.md`, `unit_003.md`, `unit_003_gap_001.md`, `unit_004.md`, `unit_005.md`, `unit_005_gap_002.md`, `unit_006.md` (7 files, all read in full). Checkout `worker-23` at `457adf4a11f`.

Path short forms (all under `omega-rust/omega/compilation/backend/`):
`NA/` = `artifacts/native-artifact/src/`, `CC/` = `calling-conventions/src/`, `ELF/` = `images/image-elf/src/`, `EM/` = `images/image-emission/src/`, `MO/` = `images/image-macho/src/`, `PE/` = `images/image-pe/src/`, `IM/` = `images/image/src/`, `ESP/` = `instruction-set-architectures/encoded-scalar-plan/src/`, `A64/` = `instruction-set-architectures/isa-aarch64/src/`, `ff` = `EM/function_fragments.rs`.

Words kept from the records: "suspected", "inferred", "not traced", "open", "stated", "traced" mean what the reader brief says. Where I opened the code myself the line says "checked".

---

## What this area does

This area takes machine code that earlier stages have already selected and placed, and turns it into a file an operating system can load, together with a sealed record of what that file contains. It decides how values cross a call boundary on each target (which registers, which stack slots), packs function bytes, data, symbols and relocations into one format-neutral image, writes that image out as ELF, Mach-O or PE, and then re-reads or re-derives most of what it wrote to confirm the bytes match the plan. Around that it keeps "custody" evidence: digests and byte spans that tie each boundary call and operator in the program to the exact bytes that realize it.

**native-artifact** (15,716 lines; this area's records cover 22 of 27 files, 13,949 lines). In: the canonical Terminal Psi module, an `ObjectArtifact`, an emitted image, selected provider plans and provider executions. Out: a sealed native artifact with one SHA-256 identity (NA/native_artifact/identity.rs:93-240), "physical evidence" (one child record per surviving boundary or operator occurrence, NA/physical/derivation/evidence.rs:67-462) or a recorded gap, publication receipts (NA/publication.rs:136, :422), and a macOS `.app` bundle on disk (NA/package_publication.rs:280).

**calling-conventions** (8,442 lines, all covered). In: a calling policy plus a call signature, or Terminal Psi structural type declarations. Out: a `CallPlan` saying where each parameter and the result live (CC/plans/call_plan_evaluation.rs:25), a validated boundary entry plan with a commitment digest (CC/boundary_application.rs:155-333), structural storage shapes (CC/terminal_layout.rs:33, CC/structural_shapes.rs:35), and string-keyed host operation and import-library tables (CC/host_operations.rs:57-472).

**image** (5,283 lines, all covered). In: an `ObjectPlan`, a `RelocationPlan`, text and data bytes. Out: a `FinalImage` (IM/builder.rs:159), relocations patched in place (IM/relocations.rs:17), inventories that account for every byte of final text and data as a classified region or a recorded gap (IM/final_image.rs:77, :1062), and checks that only declared relocation fields changed (IM/relocation_envelope.rs:19, :86).

**image-elf** (22,134 lines, all covered). In: a `FinalImage` and, for dynamic output, an interpreter plan. Out: a static ET_EXEC file (ELF/static_executable.rs:169), a dynamically linked executable built by 22 value-consuming stages (ELF/dynamic_executable.rs:614-663), or an ET_DYN shared component (ELF/shared_component.rs:710). Linux x86-64 and AArch64 only.

**image-emission** (39,628 lines, all covered).
- `ff` (24,923 lines): in, an admitted fragment container; out, the `ObjectArtifact` (ff:3592) and a replay that re-derives every claim in it from the retained abstract, target and selected plans (`source::admit`, ff:18534; top-level `validate`, ff:24426).
- `entries.rs` (5,868 lines): hand-assembled entry shims for hosted receivers, the "free Unit" entry and the UEFI physical entry, each with a byte-for-byte replay validator (EM/entries.rs:1558/:1706, :2467/:2054, :2814/:2310, :3422/:3576, :4730/:5300).
- `object_artifact.rs` (5,506 lines): the `ObjectArtifact` carrier (EM/object_artifact.rs:22-51), foreign-call custody derivation (:1916), settlement replay, a structural layout replay (:4469-4950) and stack-demand derivation (:5135).
- `image.rs`, `object.rs`: in, a sealed `ObjectArtifact`; out, an executable or shared-component image through the per-format writers (EM/image.rs:1745, EM/object.rs:238), with a final-image replay (EM/image.rs:803).

**image-macho** (4,710 lines, all covered). In: a `FinalImage`, a signing identifier, an optional dylib identity and export roster. Out: a signed MH_EXECUTE or MH_DYLIB (MO/lib.rs:671-714) that is re-read through two validators before return (MO/lib.rs:876, :893).

**image-pe** (1,904 lines, all covered). In: a `FinalImage`, a subsystem number, executable or DLL kind. Out: a PE32+ file (PE/lib.rs:171-178). The output is not re-read after writing (PE/lib.rs, unit 6).

**encoded-scalar-plan** (324 lines, all covered). In: load or store, six register numbers. Out: a 16-variant step program both ISAs lower (ESP/lib.rs:34, :53).

**isa-aarch64** (25,757 lines; this area's records cover 11 of 12 files, 7,515 lines; `selected_form_encoding.rs`, 18,242 lines, belongs to unit 007). In: targets, register views, frame and plan descriptions. Out: the physical register model and constraint catalog (A64/register_model.rs:160, :2014), the machine-effect catalog (A64/machine_effects.rs:271), frame, FPCR, syscall and post-handoff byte sequences, and a decoder for generated entry streams (A64/generated_entry.rs:136).

---

## How it decides

**By identity or exact equality of retained values (most of the area).**
- Artifact validation is a chain of equality tests on retained values and recomputed digests (NA/native_artifact/dynamic_elf.rs:109-173).
- Physical evidence looks up settlements, foreign calls and internal calls by id tuples: `(machine, invocation_site, boundary, operation_ordinal)` (NA/physical/derivation/evidence.rs:112-123), `(machine, invocation_site)` (:153-182), owner (:219). The "exactly one match, else error" idiom is used throughout (`let [x] = vec.as_slice() else`, evidence.rs, unit 2).
- `ff` replay matches abstract operations to target operations by `psi_operation` and then compares fields; uniqueness is keyed on `psi_operation`, not on the whole row (ff:16611-16639, inferred from the test asserts, gap_002).
- Mach-O and ELF pick which imports get a stub by symbol-handle identity (MO/dyld_linking.rs:183; ELF/shared_component.rs:296).

**By place (register, stack slot, byte offset).**
- Call plans are keyed on value class, size, alignment and register-bank counters, never a name (CC/plans/call_plan_evaluation.rs, unit 2 item 4).
- `validate_incoming_location` is keyed on location variant, register identity, stack offset and shape size (ff:22850-22950).
- Region inventories and relocation envelopes are keyed on offset and extent arithmetic and byte digests (IM/final_image.rs:95-166, IM/relocation_envelope.rs:37-153).

**By value (bytes and bit patterns).**
- Call instructions are recognised by fixed bytes: x86 `0xe8` + four zero bytes, AArch64 `0x9400_0000` (NA/physical/derivation/foreign_calls.rs:1237, :1251; ELF/procedure_linkage.rs:342-361).
- The AArch64 generated-entry decoder is a closed whitelist of opcode masks (A64/generated_entry.rs:56-118).
- Mach-O thunk form by opcode mask (MO/dyld_linking.rs:358-360).

**By position (fixed order, fixed index).**
- ELF section index equals the enum discriminant (ELF/section_headers.rs:2723-2725); dynamic tag rows are in one fixed order (ELF/dynamic_table.rs:1199-1315).
- `imports::validate` computes an expected symbol position by adding pool counts, so symbol order is an implicit contract (ff:2659-2672).
- Windows receiver validator offsets are tied to literal byte positions (`failed = bytes.len()-13`, EM/entries.rs:2104).

**By exact target equality** (not by format shape): hosted rows and preservation catalogs (A64/machine_effects.rs:481-501, A64/preservation_storage.rs:42, A64/hosted_sequences.rs:14-23, PE/shared_component.rs:65), and about 53 `NativeTarget::` mentions in EM/entries.rs (grep -c, unit 4).

### Spelling-keyed judgments (string compare, name lookup, name prefix)

Counted as distinct cited sites, not mentions. Total 72 across the area.

**native-artifact, 28 sites.** Requirement matching is done on identity text.
- NA/native_artifact/provider_execution.rs:134, :240, :298, :338 (`selected_requirement_identity` string compare); :158 (binary search over sorted requirement strings); :264-268 (canonical string order); :37-40, :99-102 (identity text, empty string when missing).
- NA/physical/derivation/evidence.rs:195-207 (requirement text from `module.identities.text`), :261-271 (plan found by `binary_search` over requirement strings).
- NA/physical/derivation/provider_custody.rs:29-35 (family row matched by identity text).
- NA/physical/derivation/provider_settlements.rs:81.
- NA/physical/derivation/internal_calls.rs:328-330, :361, :134 (member tag equals target-name string), :145-146 (`from_canonical_target_name(tag)`), :474-497.
- NA/physical/derivation/foreign_calls.rs:159-163, :486-497.
- NA/physical/derivation/fragment_custody.rs:137-141 (canonical symbol name).
- NA/runtime_provisions.rs:172-178, :186-190, :263-265.
- NA/package_publication.rs:163-167 (`<name>.app`), :217-223 (plist parsed by literal string split), :329-338 and :413-420 (code-signature identifier string equality).
- NA/native_artifact/identity.rs:322-325 (identity text hashed, empty when missing).

**calling-conventions, 10 sites.** CC/host_operations.rs:46-49 (`row_for_spelling`), :89-92, :317-320 (`from_name`), :380-395 (`darwin_import_library` by symbol prefix), :467-472 (`windows_import_library` by symbol); CC/boundary_application/callback_catalog.rs:533, :559, :541-542, :570 (joins by identity text); CC/boundary_application.rs:157.

**image-macho, 11 sites.** MO/dyld_linking.rs:117 (library from symbol name), :124 (`library == DARWIN_LIBOBJC_PATH`), :49-53 (dylib roster by install-name bytes), :480-538 (thunk/slot pairing by `symbol` string); segment and section names by 16-byte literal at :1729, :1734, :2031-2037, :2156-2157, :2197-2198, :2213-2214; MO/shared_component.rs:258-265 (export name).

**image-emission, 9 sites.** EM/entries.rs:584 (architecture chosen by `view_named("x0").is_some()`), :584-636 (registers by view-name strings), :2079-2091 (Windows import by `region.symbol == name`), :3936-3948 (`unique_region` by symbol name); ff:3403-3416 (private symbol collision by name), ff:24538-24542 (function symbols compared to `omega_terminal_machine_{n}`); EM/image.rs:944-960, :991-1015 (regions matched by symbol name + offset + size), :1578-1583 (identifier strings).

**image-elf, 6 sites** (the records describe these as byte-string compares; none is called "source spelling" by a reader). ELF/import_sections.rs:2454-2467 (import order by symbol/object/version strings), :2523-2558 (version index from sorted strings); ELF/dynamic_table.rs:1449-1454; ELF/section_headers.rs:1335-1341; ELF/shared_component.rs:56-61; ELF/file_assembly.rs:331-346.

**image-pe, 4 sites.** PE/imports.rs:82-86 (library from symbol, `KERNEL32.dll` default), :207, :353; PE/shared_component.rs:104-123.

**isa-aarch64, 4 sites.** A64/frame_protocol.rs:178-192 (register decoded from the view's name: first char `x`/`d`/`q`, rest parsed as a number); A64/register_model.rs:166-285 (builder looks rows up by `view("x9")`, `"nzcv"`, `"fpcr"`), :1997-2002 (`format!` of `x{n}`/`d{n}`); A64/preservation_storage.rs:47-58.

**image, encoded-scalar-plan: 0 sites.** Symbol strings are hashed into inventory digests (IM/final_image.rs:382-383, :1364-1365) but no judgment is keyed on them.

---

## The same job in more than one place

Each row: job, sites, how the copies differ when a record says.

### native-artifact
| Job | Sites | Difference |
|---|---|---|
| Rejoin a provider execution by a six-field match | NA/physical/derivation/provider_custody.rs:87-127; NA/physical/derivation/foreign_calls.rs:519-552; NA/native_artifact/provider_execution.rs:227-248 | First two checked: same filter, same zero-identity error, same `from_execution_record` call; only the error strings and who computes `selected_requirement` differ. The third builds a tuple key instead. Helper has one caller (provider_settlements.rs:151). |
| Hash the foreign-call mechanism | NA/native_artifact/identity.rs:253-313; NA/physical/derivation/settlement_identity.rs:104-148 | Same tags (Import 1, SystemCall 2, Table 3, Component 4); identity.rs also hashes same-stack report identity, bytes and alignment, the other only its commitment. |
| Replay bytes machine = object = final image, no relocation overlap | hosted_builtins.rs:305-320; provider_settlements.rs:168-173; internal_calls.rs:438-443; foreign_calls.rs:1267-1318; operator_applications.rs:618-679 (all NA/physical/) | foreign_calls allows change inside the relocation set; the others demand equality. |
| Call-instruction extent (x86 opcode at field-1, AArch64 at field, +4) | NA/physical/operator_applications.rs:76-83; NA/physical/derivation/internal_calls.rs:397-404 | none recorded |
| Text-relocation overlap test | NA/physical/derivation/evidence.rs:803; hand-written loops at NA/physical/operator_applications.rs:108-116, :235-243 | The file re-exports the shared helper (:15) and uses it at :603. |
| Build the occurrence roster | evidence.rs:542-641 (unoptimized); NA/physical/projection.rs:73-186 (optimized, unwired) | Separate identity encodings and domains. |
| Number the three hosted builtins | NA/physical/derivation/hosted_builtins.rs:86, :94, :104 (exit 1, write 2, read 3); NA/physical/derivation/hashing.rs:236-238 (exit 1, read 2, write 3) | Checked: read and write are swapped between the two tables. They feed different digests. |
| Direct vs dynamic artifact validation chain | NA/native_artifact/dynamic_elf.rs:109-173; `NA/native_artifact.rs` near :539-556 | Suspected; native_artifact.rs was not read by any record in this area. Error strings also doubled (provider_execution.rs:183 vs :203). |
| Access / multiplicity byte tables | hashing.rs:547-552 and :568-573; :343-347 and :563-567 | same file |
| Collect callback relocations | foreign_calls.rs:720-775 and :868-915 | fragment path vs object-relocation path, one function |
| Digest receipts with a "consistent evidence" method | NA/package_publication.rs:150; NA/publication.rs:244 | same pattern |

### calling-conventions
| Job | Sites | Difference |
|---|---|---|
| Classify an aggregate for an ABI (HFA, eightbytes) | CC/boundary_values.rs:111-262; CC/structural_shapes.rs:79-303 | u16 offsets vs u32; header says "independently of the calling-policy producer" (structural_shapes.rs:1-4). |
| Compute structural size and alignment | CC/terminal_layout.rs:146-486; CC/structural_shapes.rs:305-539; EM/object_artifact.rs:4469-4950 | Checked: all three give an integer alignment cap of 8 as a standalone scalar and 16 as a record field (terminal_layout.rs:458 vs :398/:407; structural_shapes.rs:534 vs :478/:487; object_artifact.rs:4523 vs :4909/:4917). So a 128-bit integer is aligned 8 or 16 depending on which function is asked. More copies suspected outside the area (pipeline 02 and 03; unit 3 and unit 5 greps, not read). |
| Register to number | CC/plans/vocabulary.rs:42 (`codec_index`), :719 (`register_code`); NA/physical/derivation/hashing.rs:413-448 | three tables |
| Encode a `StatePlan` | CC/plans/plan_identity.rs:126 (zero-based tags); CC/plans/vocabulary.rs:677 (one-based) | acknowledged at vocabulary.rs:8-12 |
| Stack-range overlap check | call_plan_evaluation.rs:225-228, :260-262, :277-279 | inline three times |
| Encode callback placements | callback_catalog.rs:307 (bytes); :240-246 (digest via `layout_placement_sort_key`) | two encodings of one list |
| Integer shape acceptance | CC/plans/vocabulary.rs:151-162 accepts 8/16/32/64 bits only; structural_shapes.rs:475, :533 accept any width | different rule for the same question |

### image-elf
| Job | Sites | Difference |
|---|---|---|
| Section to tag byte | ELF/dynamic_executable.rs:674; ELF/procedure_linkage.rs:571-578 | Checked: identical mapping (Text 1, Data 2, Bss 3, None 4). |
| Hash a fixup target | procedure_linkage.rs:2385-2414 (tags 1..7); ELF/section_headers.rs:2120-2149 (tags 0..6); ELF/file_assembly.rs:2430-2459 (tags 1..7) | different numbering and enum types |
| Match the three import-locator variants | ELF/imports.rs:43-200; ELF/import_sections.rs:2417-2469; procedure_linkage.rs:236-251; ELF/static_executable.rs:185-211 | fourth site only picks an error message |
| Derive a table, then rebuild it to validate | ELF/dynamic_table.rs:1218-1315 vs :1472-1539; import_sections.rs:1469-1547 vs :1678-1742; procedure_linkage.rs:1339-1386 vs :1481-1530; section_headers.rs:2567-2657 vs :2812-2925 | section_headers.rs:2874-2877 uses literal `3` where derive uses a lookup. `dynamic_sections::validate_contents` reuses the derive builders themselves (import_sections.rs:2688-2762, inferred). |
| Constants | ELF/lib.rs:80-96 vs file_assembly.rs:1268-1274; ELF/load_placement.rs:1875-1881 vs section_headers.rs:527-532; index 13 at load_placement.rs:1881 and section_headers.rs:2457; PLT/GOT sizes at procedure_linkage.rs:2907-2917 (usize) and file_assembly.rs:3531-3541 (u64); RELA size at lib.rs:92 and procedure_linkage.rs:1138; 25 at load_placement.rs:1791 and :1880 | same values |
| Hard-coded section indexes 8-12 | section_headers.rs:1881-1917; load_placement.rs:3008-3019; file_assembly.rs:598-605, :2128-2149, :2231-2249, :3335-3346 | load_placement.rs:3008-3019 is a hand table of what :3022-3052 would compute |
| Align up | section_headers.rs:289 (modulo, no power-of-two check); load_placement.rs:1061-1076 (mask, power of two required) | `checked_align(7, 0)` is 7 in the first; `checked_align(7, 3)` errors in the second (gap_001 ranges 8, 13) |
| Identity conversions that return their argument | load_placement.rs:1112-1116, :3004-3006; file_assembly.rs:3460-3464, :3498-3502 | grep: 16 + 9 occurrences |
| Accessor chains to the retained image/target | load_placement.rs:512-545; dynamic_table.rs:1354; procedure_linkage.rs:1419, :2009; file_assembly.rs:274-286 | |
| Test helpers | 27 per-file definitions of `candidate` / `refresh_fingerprint` / `custody_snapshot` / `assert_rejected_with_*` (gap_001 grep); interpreter-path literals at dynamic_executable.rs:666-672, lib.rs:690-696, section_headers.rs:2221-2225; seven-stage chain by hand at file_assembly.rs:845-851, :1805-1811, :3640-3646 | each helper body differs |

### image-emission
| Job | Sites | Difference |
|---|---|---|
| Emit a shim and separately re-derive its bytes | EM/entries.rs:1558 vs :1706; :2467 vs :2054; :2814 vs :2310; :3422 vs :3576; :4730 vs :5300 | by design two implementations per platform |
| "ELF entry points to" | entries.rs:3308 (checks only `e_entry`); entries.rs:4095 (header, machine, entry, PT_LOAD flags, shim bytes) | same name, different strength |
| Entry storage partitions and provisioning | entries.rs:1319 / :1364 (saved 16 bytes); entries.rs:5096 / :5468 (saved 24 bytes) | |
| "Producer comes before consumer on every path" | ff:5185 (dominator sets); ff:12411 + :12442 (walk from entry avoiding the producer block); ff:12568 (indexed, calls `precedes_blocks`) | Checked: same question. Two records each suspected the other's site. Differences: ff:5185 returns false when either block is unreachable (:5192-5194); ff:12442 returns true when the walk never reaches the consumer block (:12471), which includes an unreachable consumer. |
| Site to unique ordinal | ff:103 (linear scan); ff:45-101 (index) | |
| Interval exactness | ff:564; ff:675 | fast path vs "original full scan" (comment ff:489-490) |
| Address-field relocation split | ff:1750-1765; inline again at ff:3102-3116 | decode of the same two encodings three times in EM/image.rs:551-563, :711-723, :1125-1137 |
| Build a record, then re-derive it to check | ff:23607 vs ff:22850 (incoming location); ff:23684 vs ff:23135 (copy extent); ff:20279 vs :20371, ff:20726 vs :20824, ff:22138 vs :22234 (settlements) | two parallel case lists |
| Find the one target row for a `psi_operation` | 32 `fn *retained` definitions in ff (unit 5 grep) | |
| Definition occurrence of a register | ff:12635-12648 (scan per call); ff:12547-12550 (table) | |
| Resolve a structural home | ff:18264 (free fn); ff:18246 (indexed) | both end in ff:18321 |
| Stack numbers | ff:19409-19499 (encoded effects); EM/object_artifact.rs:5295-5303 (call-stack rows); ff:24111 (frame + call) | three sources |
| Append entry bridge + call relocation | EM/object.rs:93-115; EM/object.rs:714-748 | |
| Decode bridge bytes and compare | EM/object.rs:411-426; EM/image.rs:370-430 | near-identical |
| Emit sequence (prepare entry, build image, attach effects, emit, validate) | EM/image.rs:156-188, :238-253, :1830-1858, :1911-1925 | written four times |
| Scalar-result producer check | EM/object_artifact.rs:3452-3457; ff:22416-22425 | |
| `unique` helper; `operation_rows` | ff:8881 vs ff:4810; ff:12769 vs ff:9782 | second pair grepped, not read |

### image-macho, image-pe, image, isa-aarch64
| Job | Sites | Difference |
|---|---|---|
| Validate a component export roster | ELF/shared_component.rs:56-76; MO/shared_component.rs:251-290; PE/shared_component.rs:104-132 | Checked: same job three times. ELF and Mach-O are line-for-line the same test; PE detects duplicate commitments by count == 1 instead of a prefix scan and sorts by name first. Unit 6 only suspected the ELF copy. |
| Install, patch and validate import thunks | PE/imports.rs:37, :295, :319; MO/dyld_linking.rs:75, :271, :337 | PE thunks every import row (:49-178); Mach-O only referenced ones (:183). PE checks 2 opcode bytes (:340); Mach-O decodes the thunk and re-reads the container. |
| Read ULEB128 | MO/dyld_linking.rs:1659; MO/shared_component.rs:139 | different overflow rules (:1664 vs :146) |
| Decode thunk opcodes | MO/dyld_linking.rs:355-361; :555-566 | same file |
| Patch ADRP / page-offset | IM/relocations.rs:224-261; MO/dyld_linking.rs:420-468 | Mach-O has its own copy of the page-range check (:430) |
| Load-command sizes | MO/file_layout.rs:34-51 (constants); MO/dyld_linking.rs:2081-2091 (literals) | two tables |
| Data vs executable region inventory | IM/final_image.rs:89 vs :1073; :199 vs :1182; :331 vs :1313; :354 vs :1336; :399 vs :1441 | executable digest domain `.v2` vs data `.v1`; register index written as 4 bytes at :1405 and 1 byte at :1422 |
| Little-endian byte writers | IM/lib.rs:81-92, :101-142; PE/lib.rs:52-62; MO/dyld_linking.rs:470; EM/lib.rs:41-53 | at least four families; `mod codec_bytes` defined in 9 places and `DomainDigestEncoder` in 2 across omega-rust (unit 2 grep) |
| Align helpers | `fn align(` has 9 definitions in omega-rust (unit 4 grep) incl. ELF/shared_component.rs:455, EM/entries.rs:1296, :5440; `fn checked_align_up` has 5 (unit 5 grep) incl. EM/object_artifact.rs:4473, CC/terminal_layout.rs:444 | |
| Large-offset page stepping | A64/floating_control.rs:46-55; A64/frame_protocol.rs:359-375; A64/immediate_offset.rs:60 | |
| Load/store word encoders | A64/post_handoff_writer.rs:396; A64/frame_protocol.rs:377-391 | different offset rules |

Suspected duplicates that point outside this area (not opened): `ranges_overlap` at `realization/provider-planning/.../semantic_ranges.rs:144` (unit 2); `hash_bytes` at `artifacts/boundary-applications/src/coverage.rs:171` (unit 2); `scalar_shape` in pipeline 02/03 files (unit 3); `establishment` at `03_.../selection_inputs.rs:4044` (gap_002); `InstallationError` struct at `runtime/executable-installation/src/installation.rs:85` (unit 5; same name, different type).

---

## Fixed lists of shapes

### native-artifact
- Six gap subjects, the complete list of "cannot realize this" outcomes (NA/physical/mod.rs:733-760).
- Three hosted builtins only (NA/physical/derivation/hosted_builtins.rs:71-106); read-byte result layout fixed (:145-157).
- Four in-place provider realizations: `MetadataOnlyPort`, `DirectPortReadU8`, `LinuxWriteLine`, `ClaimCompletionOnly`; others return `Ok(None)` (NA/physical/derivation/provider_settlements.rs:52-61, :97-150).
- Port custody is x86-64 only (NA/physical/derivation/provider_custody.rs:143, :238).
- Foreign structural parameters: header says "only for the exact source-rooted borrowed flat-record lane that lowering owns" (NA/physical/derivation/foreign_calls.rs:7-9); returns `Ok(None)` unless every access is a borrow, multiplicity Unrestricted, path all `Field` segments, no callbacks (:1080-1125).
- Scalar normalization: floats 32/64, bool, ints 8/16/32/64; other widths (128) rejected (NA/physical/derivation/fragment_custody.rs:631-645).
- Operator spans: five call kinds (NA/physical/operator_applications.rs:45-50), `IntegerEqual` or less-or-equal compare (:411-415), `IeeeFloatCompare` on the fragment path only (:407).
- Mixed ABI only with a scalar result (NA/native_artifact/mixed_structural_scalar.rs:59).
- macOS bundle: exactly `Contents/Info.plist` and `Contents/MacOS/<name>` (NA/package_publication.rs:151-156, :341-344).

### calling-conventions
- Five calling policies (CC/plans/vocabulary.rs:433-439); `native_for_target` panics for (aarch64, COFF) (:490-494, :505-509).
- Microsoft x64: 4 integer registers, 32 shadow bytes; HFA and SysV aggregates rejected (CC/plans/call_plan_evaluation.rs:462-465).
- Syscalls: integer class only, at most 6 argument words, integer result up to 8 bytes (:907-943).
- One `Aapcs64` policy for Linux arm64 and macOS arm64; no per-OS stack-packing variant (vocabulary.rs:488-489; unit 2).
- `ValueShape::for_integer`: 8/16/32/64 bits and the 64-bit address carrier only (vocabulary.rs:152-159).
- Structural shapes classified: record, fixed array, primitive scalar, reference, borrowed byte sequence, element view; everything else `Err` (CC/structural_shapes.rs:51-70). Nesting depth 32 (:88; CC/boundary_values.rs:215).
- Direct callback parameters: "currently require an admitted 8/8 target function-pointer shape" (CC/boundary_application/native_telescope.rs:22-26).
- Interrupt gate: all 64 bits supplied exactly once (CC/host_operations.rs:516, :544).

### image-elf
- Dynamic executable: 22 stages (ELF/dynamic_executable.rs:614-663), 14 sections in fixed order (ELF/section_headers.rs:2459-2474), 5 program headers and 3 PT_LOAD with permissions exactly R / RX / RW (ELF/load_placement.rs:40-79), 25 placement fixups = 13 + 12 (section_headers.rs:530-532), 9 or 7 dynamic address targets (load_placement.rs:2535-2555), one GNU-hash bucket (ELF/import_sections.rs:2160-2163), `DF_BIND_NOW` always (ELF/dynamic_table.rs:1310), at least one DT_NEEDED (:1387-1390), every relative region non-empty (load_placement.rs:654-664).
- Imports are functions only (import_sections.rs:2367); ELF-versioned or component locators only (ELF/imports.rs:95-104).
- Relocation shapes: direct call (text, 4 bytes, addend 0) or data `Absolute64` width 8; anything else "ELF dynamic import requires a non-procedure or malformed source relocation" (ELF/procedure_linkage.rs:286-335).
- Shared component: 13 sections, 5 program headers, any `Absolute64` refused (ELF/shared_component.rs:21-23, :82-91). Static: exactly two PT_LOAD (ELF/static_executable.rs:58, :74).

### image-emission
- `source::admit`: one `match` over every abstract operation, last arm `_ => false` (ff:18699-19024).
- `requires_graph_storage_replay`: about 45 operation kinds listed (ff:18405-18478).
- Address reconstruction: five instruction kinds (`FrameAddress`, `AddressOffset`, `CopyI64`, `Load64`, `ByteViewAddress`), else `None` (ff:12976-13017).
- Case tag: 4 bytes at offset 0 only (ff:13350-13353). Indexed store: index `u64`, value `u8` (ff:16930-16933). Primitive local sizes 8/16/32/64 bits, Boolean, Binary32/64 (ff:21388-21401).
- Installed interrupts: exactly `[StructuralArgument; 6]` (ff:7286), fixed control-block offsets 48-128 (ff:6853-7023, :7856), closed instruction-kind list with each count exactly 1 (ff:7388-7451).
- Legacy carriers: 14 `ObjectFunction` fields must be empty (ff:24678).
- Hosted shapes: process exit 12 or 16 bytes for four targets, no Windows (EM/object_artifact.rs:4198-4207); byte output 40 / 42 bytes (:4372-4389); read-byte sum "exactly two cases" (:4031-4039).
- Installed arrival: all 32 exception vectors required (EM/object_artifact.rs:954); only vectors 2 and 18 nested (:1055); memory-kind and privileged whitelists (:1311-1341).
- Structural projection replay: Case, Field, FixedIndex only (EM/object_artifact.rs:4679-4818).
- Receivers: Windows ordinals 0-4 (EM/entries.rs:3148-3158), AArch64 fewer than 8 (:1578), Linux x86 six registers (:2896-2901). Physical entry: UEFI x86-64 PE only (:5133-5143).
- Emit targets: (Elf, Aarch64), (Elf, X86_64), (MachO, Aarch64), (MachO, X86_64), (Coff, X86_64), 8-byte pointers (EM/image.rs:1806-1817).
- Private objects: Text and Data sections only (EM/object.rs:520, :797-805).

### image-macho, image-pe, image
- Mach-O container: one `__TEXT` with one section, optional `__DATA` with at most `__data` + `__bss`, `__LINKEDIT` with none (MO/dyld_linking.rs:2141, :2188, :2253); at most 15 dylibs (:66, :1718); fixed bind/rebase opcode grammar (:1809-1848); build version fixed to macOS 13 (:1006-1015); 2 special slots, SHA-256 only (MO/code_signature.rs:16, :102-103).
- PE: machine fixed `0x8664`, PE32+ only (PE/headers.rs:41, :49); section order text, rdata, data, reloc, bss (PE/sections.rs:50-61); DIR64 base relocations only (:160); export by name, ordinal base 1 (PE/shared_component.rs:166).
- Five relocation kinds (IM/relocations.rs:56-116); page-offset patch is the ADD-immediate form only (:257-261). Three data and three executable region origins (IM/final_image.rs:24, :1012).

### isa-aarch64, encoded-scalar-plan
- Generated-entry whitelist: RET, BL, NOP, 64-bit ADD/SUB immediate, 14 load/store masks, MOVN/MOVZ/MOVK, logical immediate, register move; anything else "unsupported generated AArch64 instruction" (A64/generated_entry.rs:56-118).
- Frame slots 8 bytes only (A64/frame_protocol.rs:146); access sizes by register class (:377-386).
- 120 hand-listed required constraint keys (A64/register_model.rs:1822); system calls 14 keys, arguments x0..x5 (:2269-2290); scalar call arity 0..=8 (:541-547).
- Nine saturating realizations (A64/saturating_forms.rs:14). Two step programs, six registers each (ESP/lib.rs:55, :75).

---

## Accepts when it does not know

### Classed "open" by the readers (no reason at the site, not traced)

native-artifact
1. NA/native_artifact/provider_execution.rs:140-169: a foreign binding with neither a same-stack contribution nor a system call gets no stack or kernel check and returns `Ok`. Reached by Import/Table/Component mechanisms. `same_stack_contribution` not traced.
2. NA/native_artifact/provider_execution.rs:37-40, :99-102: a boundary whose identity text is missing becomes requirement `""`.
3. NA/native_artifact/provider_execution.rs (via tests.rs:287): `selected_requirement_identity` returns an unknown requirement unchanged. Read from the test; the definition (native_artifact.rs) was not read.
4. NA/native_artifact/identity.rs:322-325: a missing identity index hashes as the empty string, so two different missing indexes collide. Open whether a caller can reach a missing index.
5. NA/physical/derivation/evidence.rs:154: foreign-call custody is compared to the object only when a fragment publication exists; otherwise the retained list is `&[]` and nothing is compared. Not traced whether custody can be non-empty without a fragment publication.
6. NA/physical/derivation/evidence.rs:201-204: empty requirement text flows to the plan lookup (:264-266); open for how plans are built.
7. NA/physical/derivation/evidence.rs:60-65 (and dynamic_elf.rs:63-65): an unsupported shape returns `Ok(Blocked(gap))` and the artifact still seals with the gap in its identity. The reader did not establish whether publication refuses a gap.
8. NA/physical/derivation/provider_custody.rs:29-37: no family-realization row for the identity returns `Ok(())` with no check. The reader thought a family requirement whose row is missing from the plan would pass here; open whether a missing row can occur.
9. NA/physical/derivation/internal_calls.rs:474-481, :488-497: missing provider/candidate identity text becomes `""` in the parent digest and role.
10. NA/physical/derivation/fragment_custody.rs:401-404: a scalar argument whose native position is not a register position gets index `register_arguments` (one past the end); what `scalar_immediate_span` does with it was not read.
11. NA/runtime_provisions.rs:172-179, :233-236: empty strings for missing identity and requirement text passed to `selected_row_matches_requirement`; its behaviour on `("", "", id)` not read.
12. NA/runtime_provisions.rs:215: no calling application, hence no admitted-row check, when the binding has no import (system call, table, component). Recorded as "read", no class given.

calling-conventions
13. CC/terminal_layout.rs:191-203: records with `Authored`/`AccessView`/`Accessor` storage are accepted at their declared size with no field cross-check.
14. CC/plans/call_plan_evaluation.rs:167-310: plan validation is structural (coverage, disjointness, alignment, architecture). It never compares registers with what the policy would assign. The reader's wrong program: a policy-authored plan using any registers of the right architecture passes. Callers in provider-planning not traced.
15. CC/plans/call_plan_evaluation.rs:290-292: result-placement registers are not added to the occupied set, so an indirect-result pointer register equal to a parameter register is not detected here.
16. CC/host_operations.rs:380-394: every Darwin symbol that matches no prefix is bound to libSystem; a misspelled symbol is bound, not rejected. Three call sites; whether symbols are pre-validated not traced.

image-elf
17. ELF/imports.rs:178-185: a relocation whose symbol handle is invalid is skipped silently. Later stages not traced by that reader.
18. ELF/file_assembly.rs:2217-2337: stage 20 does not length-check `.rela.dyn` bytes, leaves `GeneralRelocation` out of the unchanged-bytes loop (:2323-2335) and out of the stage fingerprint (:2409-2413). A changed `.rela.dyn` byte outside the fixup-owned words would not be caught. The reader adds: not reachable from input in the normal flow because derive and validate run back to back (traced). No test mutates these bytes (gap_001, range 18).

image-emission
19. ff:3988-4112: `replay::validate` returns `Ok` for an artifact with no fragment replay unless it hits one of six listed shapes; such an artifact gets no replay of abstract/target/selected evidence. What creates replay-less artifacts not traced.
20. ff:18745, :18864-18865, :18995-19007: `admit` arms that return `true` with no check: atomic events (load, store, read-modify-write, swap, compare-exchange, fence), integer and boolean constants, and the comparison/arithmetic/shift/cast family. The reader's wrong program: one where the selected or machine layer lowered such an operation wrongly (wrong instruction, dropped store). Whether attribution replay (ff:8-1470) covers them was not established.
21. ff:18988, :18992: `Return` / `ReturnUnit` with empty cleanup actions admitted without looking at the selected return.
22. ff:18729: `PortWrite` admitted when any matching selected instruction exists (`.any`), not exactly one.
23. ff:16982-16984: `runtime_footprint_retained` returns `true` when the operation has no runtime indices; a non-indexed access with wrong or absent memory-access rows would pass here if nothing else checks them (other checkers not traced).
24. ff:4717-4731 (`dedicated_descriptor_owner`): a hardware descriptor instruction owned by an installation operation is skipped; safety depends on `installed_interrupts::validate` (ff:8898) running on the same function.
25. ff:20219 (`cleanup_actions_match`): tests accept any subset or reordering of the read results' discards (ff:20644-20649); the empty list is not asserted. Marked open by gap_002 because the body was not read.
26. EM/object_artifact.rs:3628: `_ => true` in the execution/realization pairing. An `AdmittedProvider` execution with any realization other than the three hosted ones is accepted by this check; a later custody equality (:3633-3641) still applies.
27. EM/object_artifact.rs:1170: `ram::validate` examines three operation kinds and `continue`s on all others.
28. EM/object_artifact.rs:1673-1675: a foreign call whose record has no descriptor input passes; a call that should have a guard but whose record omits it would pass.
29. EM/object_artifact.rs:2594-2609: argument sources `Parameter`, `BlockParameter`, `BooleanImmediate`, `IeeeFloatImmediate` record the whole call span as the argument span with no transport check.
30. EM/entries.rs:328: `_ => {}` for instruction families not in the privileged list; they contribute only their decoded footprint. Which families exist not traced.
31. EM/entries.rs:3891-3896: `prepare` returns `Ok(None)` (no shim) when the artifact is not a free-Unit entry or the target has no form.
32. EM/entries.rs:5575-5594: when two origin rows share an identity, a `None` field never conflicts with a `Some`.

image-macho, image-pe, image, isa-aarch64
33. MO/dyld_linking.rs:193-210: a relocation with an invalid symbol handle is skipped at thunk installation.
34. PE/imports.rs:95: only the Foreign-locator branch rejects UEFI subsystems 10..=12; string-backed and component imports do not, so an import-bearing EFI image is not rejected here.
35. PE/headers.rs:42, :52 and PE/sections.rs:51, :57, :60, :61, :86, :183: unchecked `as u32` / `as u16` casts.
36. PE/lib.rs: the written file is not re-read; header and section-table correctness rests on the planner.
37. IM/lib.rs:221: `final_image_symbol_address` adds the offset without checking it against the section size.
38. A64/machine_effects.rs:993-998, :1187: default arms give any semantic not in the implicit-effect table no implicit uses or defs, "never traps", fall-through, 4 bytes. The reader's wrong declaration: a semantic added to the reads/writes table (:729-879) but not to the effect table would be declared pure.
39. A64/post_handoff_writer.rs:104, :126: the byte loop is only correct up to 8 bytes; whether larger container widths can arrive depends on `layout_plans::validate_fragment`, not read.

### Classed "stated" (a comment, test name or structure at the site says why)
- NA/native_artifact/mixed_structural_scalar.rs:19: functions without a mixed ABI are skipped by the filter.
- NA/runtime_provisions.rs:460-471: a root object with no component imports needs no provisions (match arm).
- NA/physical/projection.rs:114-115: a D29 reference whose operation did not survive is dropped; test name "requires children only for final survivors" (:362).
- CC/host_operations.rs:89-92, :317-320: unknown capability/operation names become `Custom(index)`; header :9-12 "intern to a stable index for the life of the process".
- CC/boundary_application/callback_catalog.rs:471-478 and CC/boundary_application/terminal_telescope.rs:135: `Ok(None)` / `continue` for shapes that carry no layout or pointer geometry (by design).
- CC/terminal_layout.rs:121-125: unresolved shapes accepted while collecting extents (purpose of the function).
- ELF/section_headers.rs:507-510 (test): flag bits other than ALLOC/WRITE/EXEC do not change region classification.
- ff:8083-8103: installation reads of declared inputs, control words 64-96 and the table need no guard (stated by structure).
- ff:2128-2130: `None` implementation-table rows keep a zero slot; test name `trap_rows_keep_slot_offsets_and_callable_relocations` (ff:2385).
- IM/final_image.rs:131-166, :1114-1149: uncovered byte stretches become recorded gaps, not errors (header: "explicit gap digests").
- IM/relocation_envelope.rs:71-82, :98: any value inside a declared field is admitted, and final text beyond the encoded length is not examined (test comments at `image/tests/initialized_data_envelope.rs:164-170`, `text_relocation_envelope.rs:405-422`).
- MO/dyld_linking.rs:1520-1527: an import with no relocation expects no pointer (stated by symmetry with :183, no comment).
- A64/register_model.rs:1084-1120: the catalog validator compares against a catalog from the same builder, so a builder defect is accepted by construction (stated by structure, not by a comment).

### Classed "traced" (the reader followed callers)
- ELF/file_assembly.rs:184-211: `admit_elf_dynamic_executable` only derives an output; the function that compares it with the assembled file and checks the ELF magic is `#[cfg(test)]` (:352-391). The bytes were validated at stage 21 (:464-521). Confirmed by gap_001 range 16 and by my grep.
- EM/image.rs:1019: `ProvisionedUnit` shim bytes are not checked inside terminal-image validation; the emit paths (:1851-1858, :208-215) do not run `validate_unit_shim`, the replay validators (:1941-1952, :264-275) do. A bad shim would be returned from `emit_*` and caught only by a later validate.
- IM/final_image.rs:199, :1182: inventory validators check an inventory against the final bytes and its own rows, not against the `FinalImage` region lists; a consistently recomputed forged inventory validates.
- IM/relocations.rs:57-65: `byte_width` is not read when patching; a disagreeing record is caught only by the envelope validators.
- IM/builder.rs:110-114 and PE/sections.rs:181: invalid handles and Bss/None absolute relocations are carried or dropped here, then rejected by `apply_relocations` (IM/relocations.rs:25-37, :59-63).
- PE/imports.rs:83-84: empty library + unknown symbol binds to `KERNEL32.dll`; traced to the builder (IM/builder.rs:90-92), not further. Result is a load-time failure, not a compile-time one.
- ELF/dynamic_executable.rs:353 (`|_| true` replay arm): the stage's own validator replays a fingerprint stored in the contents (ELF/import_sections.rs:2763-2767).
- ELF/procedure_linkage.rs:2827-2859, ELF/section_headers.rs:2927-2953, ELF/load_placement.rs:152, :362, ELF/imports.rs:133-150: each traced to a closing check.
- EM/entries.rs:3241 (weak ELF entry check followed by shim replay), :5188-5190 (no physical entry, nothing to validate).
- ff:12334-12343, ff:679-685, ff:14003, ff:18365-18401, ff:22328-22333, ff:23552, ff:24096, ff:24517-24519, ff:19461-19463: each traced to a later rejection or to "nothing to prove".
- EM/object.rs:889-891, :457-460; EM/image.rs:528-530, :633-635, :1570-1589; EM/object_artifact.rs:2335-2343, :5279-5282, :204-219.
- NA/physical/derivation/internal_calls.rs:292-298 (`Ok(None)` becomes a gap); NA/physical/derivation/foreign_calls.rs:567 compares a call with itself when there is no publication (inferred).
- MO/dyld_linking.rs:1771-1777, :1764; PE/imports.rs:302, :305-306.
- A64/frame_protocol.rs:183, :199; A64/generated_entry.rs:23-40; A64/register_model.rs silent returns (validator rejects a non-canonical model first, :1077).

---

## Silent drops

Places where something is skipped with no row and no diagnostic. Counted as distinct sites: 41.

- native-artifact (6): NA/physical/projection.rs:114-115 (optimized-away D29 reference); NA/runtime_provisions.rs:160-164 (provisions that are not normalized foreign calls), :52-54 (parameter origins other than `SemanticFormal`), :215 (no calling application); NA/native_artifact/provider_execution.rs:32-36 (settlements that are not admitted-provider); NA/native_artifact/mixed_structural_scalar.rs:19.
- calling-conventions (2): CC/boundary_application/terminal_telescope.rs:132-135; CC/boundary_application/callback_catalog.rs:471-478.
- image-elf (3): ELF/imports.rs:169-175 (imports with no relocation are validated then dropped), :178-185 (relocation with invalid handle); ELF/file_assembly.rs:2480-2486 (relocations that target an import are dropped before local patching).
- image-emission (10): ff:2128-2130; ff:14003; ff:19461-19463 (encoding row with no footprint); ff:4456-4458; ff:10609-10611 (`DiscardRoot`/`RetireHome`/`DiscardResidual` actions); ff:4128-4130 (`summarize` returns `Ok(None)` without replay); EM/entries.rs:328; EM/entries.rs:3891-3896; EM/object_artifact.rs:1170; EM/object_artifact.rs:2335-2343 (empty list on overflow).
- image-macho (3): MO/dyld_linking.rs:183 (unreferenced imports never thunked), :193-210, :1764 (unknown load commands ignored by the fixup validator; the mapping validator rejects them at :2090).
- image-pe (3): PE/sections.rs:181; PE/imports.rs:302 (`zip` truncates if lengths differ); PE/headers.rs:117-118 (section name truncated to 8 bytes).
- image (2): IM/builder.rs:44-48 (import symbols get no region), :110-114 (missing symbol becomes an invalid handle).
- isa-aarch64 (12): A64/register_model.rs:1156, :1166, :1169, :1190, :1361, :1372, :1427, :1434, :1452, :1466, :1476 (builder step returns early when a view or base row is missing); A64/generated_entry.rs:23-40 (register 31 treated as the zero register, no effect recorded).
- encoded-scalar-plan: none recorded.

---

## Unused or unwired

With the readers' grep counts; "checked" marks counts I re-ran.

- **Optimized physical-evidence path is unwired.** `#[expect(dead_code, reason = "unwired, not superseded: ... PIPELINE-OWNER-CONSOLIDATION")]` on `mod projection` (NA/physical/mod.rs:76-80), on two scope constructors (:1030-1037, :1054-1058), on `derive_scope` and `identity` (NA/physical/fragment_publication.rs:35-39, :61-64). Grep `unwired, not superseded`: 7 occurrences in 3 files (checked: native_artifact.rs 1, fragment_publication.rs 3, physical/mod.rs 3). Inferred by unit 2: in non-test builds the optimized scope cannot be constructed, so the optimized branches of evidence.rs (:81-85, :99-110, :141-156, :415) and the fragment rejoin code run only if a test or outside caller builds it.
- `DynamicElfNativeArtifact::from_replayed_parts` (NA/native_artifact/dynamic_elf.rs:86): 0 call sites (unit 2). Checked: `from_replayed_parts` appears on 6 lines of `.rs` in omega-rust, five definitions and one call (NA/physical/mod.rs:888, the `NativeByteSpan` one). So the public constructors at NA/physical/mod.rs:645 and :678 also have no caller in `.rs` files. `validate` for the dynamic artifact runs only from this constructor (dynamic_elf.rs:92).
- `InstallationError` (EM/installation_record.rs:36, about 190 variants, re-exported EM/lib.rs:129): no constructor and no importer (unit 5; multi-line `use` not excluded). Checked: the only mentions in omega-rust are the definition, its Display/Error impls and two re-exports; the other hits are a different struct in `runtime/executable-installation`.
- `can_emit_executable_image` (EM/image.rs:1806): 1 caller (image.rs:1824), 0 outside (checked: 3 matches, image.rs 2, lib.rs 1).
- `validate_executable_output` (ELF/file_assembly.rs:353): test-only; 1 definition, 1 import, 5 calls, all in the file's tests (checked).
- `temporary_stack_bytes_for_function` (ff:19421): `#[cfg(test)]`, 7 matches all in ff (checked).
- `observe_native_function_cpu_state`, `observe_native_constant_operand` (EM/entries.rs:860, :872): callers only in `compiler/tests/support/native_entry_controls.rs` (checked).
- `CompilerEntryRegionBindingEvidence` / `CompilerEntryFootprintBindingEvidence` (IM/output.rs:77, :95): documented as "remain load-bearing across crate boundaries" (IM/lib.rs:34-42); both producers set the fields to `None` (EM/image.rs:1434-1435; IM/output.rs:240-241); consumers hash them (NA/native_artifact/identity.rs:163-164). No non-`None` producer outside a test (checked; the test is NA/native_artifact/tests.rs:403-404).
- Fourteen `ObjectFunction` carrier fields that `empty_unsupported_records` requires empty (ff:24678) are still public fields (EM/object_artifact.rs:341-373). Suspected unused; not grepped per field.
- Single-variant or empty items: `ImageOutputKind` (IM/output.rs:212; 7 matches in 3 files); `ElfDynamicAddressApplicationKind`, test-only (ELF/load_placement.rs:2557-2562); `NativeCompilerBuiltinCatalogIdentity::HostedV1` (NA/physical/mod.rs:121); empty `impl ObjectArtifact {}` at EM/entries.rs:2668-2669 and ff:3953-3954; empty `mod tests {}` at ELF/static_executable.rs:157-158.
- Ignored parameters: `_identities` (NA/native_artifact/provider_execution.rs:125); `_signature` (CC/plans.rs:427).
- `DynamicElfPublicationReceipt.installation_evidence_digest` (NA/publication.rs:428): written at :416, no accessor, takes part only in derived equality.
- `AARCH64_INLINE_ASSEMBLY_DEFAULT` (A64/register_model.rs:1709): unit 6 could not separate its count; checked: 5 matches, all in register_model.rs.
- `MachoDylib::LIBOBJC` (MO/dyld_linking.rs:1041): used only at :1060 and in a test (:1111).
- Test-only visibility: `aarch64_page_delta_covers_extent` (ELF/load_placement.rs:492) and `validate_deferred_constraint_envelope` (:425) each have 1 production caller in the same file.
- `encoded_scalar_plan`: documented as the single shared definition; 4 matches in 2 files, the two ISAs (checked). The claim holds.
- "Sole builder of `ObjectArtifact`" (EM/lib.rs:11): the only struct literal is ff:3821 (unit 5 grep); EM/object.rs:467 makes modified clones, a second path.
- One-caller public or crate-public functions are the norm in NA/physical/derivation (each `derive_*_child` and each settlement identity function has 1 caller; unit 2), and for `bind_hosted_receiver`, `bind_physical_entry` (1 caller each in `fragment_object_publication.rs`; unit 4).

---

## Stated limits

- "non-installable" (NA/native_artifact/dynamic_elf.rs:1, :46).
- "dynamic bytes can never satisfy direct-image publication custody" (NA/publication.rs:8-10).
- "only for the exact source-rooted borrowed flat-record lane that lowering owns" (NA/physical/derivation/foreign_calls.rs:7-9).
- "unwired, not superseded" (NA/physical/mod.rs:76-80; NA/physical/fragment_publication.rs:37, :63, :142).
- "Microsoft x64 aggregate classification is not normalized yet" (CC/plans/call_plan_evaluation.rs:463).
- "homogeneous-float normalization requires two to four f32/f64 members totaling at most two eightbytes" (CC/plans/call_plan_evaluation.rs:126).
- "one-eightbyte non-homogeneous SSE aggregates are not representable in the closed ABI vocabulary" (CC/boundary_values.rs:165-168).
- "direct callback parameters currently require an admitted 8/8 target function-pointer shape" (CC/boundary_application/native_telescope.rs:22-26).
- "ELF loader binding is not implemented" (ELF/static_executable.rs:188).
- "runnable dynamic ELF emission remains fail-closed before image mutation because no target-owned ELF loader plan carries the exact PT_INTERP bytes" (ELF/static_executable.rs:202; over the 20-word limit, quoted in full from unit 4).
- "string-backed ELF bootstrap imports cannot enter normalized dynamic-link planning" (ELF/import_sections.rs:111).
- "required-version count exceeds the unhidden version-index domain" (ELF/import_sections.rs:2535-2536).
- "component absolute pointers require loader-relative relocation custody" (ELF/shared_component.rs:88-90).
- "explicit physical arrival currently requires x86-64 PE" (EM/entries.rs:5133-5139).
- "interrupt provider callback closure has no retained state footprint" (EM/entries.rs:221-223).
- "Other input shapes retain the original full scan and its error order." (ff:489-490)
- "borrowed extent requires a successful View lifecycle" (ff:12021-12023).
- "cleanup arrival has no reconstructed backing observation" (ff:11540); "installation helper arrival has no reconstructed backing observation" (ff:11552).
- "entry or dynamic input has no closed physical backing supply" (ff:12208-12210).
- "legacy call mirror requires a direct machine target" (ff:23979); "shared fragments cannot carry legacy singular cleanup" (ff:22510).
- "shared image publication requires a nonempty 64-bit function roster" (ff:18542).
- "external placed backing needs its own mapped-residence proof" (EM/object_artifact.rs:1146-1149).
- "nested installation is not a closed Context source" (EM/object_artifact.rs:1161); "dynamic call has no closed fault analysis" (:1301); "arrival stack domain is unresolved" (:931).
- "shared component requires an admitted ELF, Mach-O, or Windows PE container" (EM/object.rs:337).
- "missing output configuration: dynamically linked ELF output requires an interpreter pathname" (EM/image.rs:1681).
- "the current canonical bind encoding supports at most 15" (MO/dyld_linking.rs:68).
- "AArch64 MVP encoder cannot compare value `{value}` yet" (A64/hosted_sequences.rs:222-224).
- "memory and Unit call forms are not admitted on this target" (A64/machine_effects.rs:868, :1157; a `panic!`).

Comments the readers found not to match the code:
- ELF error strings spell counts that disagree with the constants: "exact thirteen-row roster" vs 14 (ELF/section_headers.rs:706), "exactly twenty-three" vs 25 (:746), "exactly eleven allocated rows" vs 12 (:754); "exactly twelve file offsets" vs 13 (ELF/load_placement.rs:2117), "exactly eleven virtual addresses" vs 12 (:2128). The checks use the constants.
- ELF/file_assembly.rs:5-7 header says admission is "re-verifying final-byte admission"; the verifier is test-only (see above).
- NA/package_publication.rs header says "atomically replace"; the code removes then renames (:375-382).
- MO/lib.rs:5-7 says imports bind "through `__DATA,__got` pointer slots"; the section is named `__data` (MO/dyld_linking.rs:1438) and no `__got` is emitted.
- PE/imports.rs:5-6 "re-verify that every thunk's `ff 25` opcode survived"; the check covers those 2 bytes only (:340).
- A64/preservation_storage.rs:3-5 says `x19`..`x28`; the code lists x19..=x29 (:47-50).
- A64/lib.rs:11-20 claims every byte producer has an in-crate decoder; not true of floating_control.rs, frame_protocol.rs, hosted_sequences.rs in the files read (decoders for selected forms are in the file unit 6 did not have).
- A64/floating_control.rs header says "encode and decode"; the file has encoders only. A64/saturating_forms.rs:1-7 says "instruction-word realizations"; it holds classification and size tables only.
- A64/hosted_sequences.rs:1-6 says Linux and Darwin cover console and line writes; the write-byte leaf hard-codes the Linux syscall (:39-40).
- One ELF test labels section index 12 as the name table, which is index 13 (ELF/load_placement.rs:3319-3323 vs :1108-1109).

---

## Disagreements

1. **Size of the ELF section-name seed at the descriptor stage.** unit_003 item 6 says the descriptor stage uses a "hard-coded 113-byte..." seed (sentence cut off); gap_001 says 79 bytes there and 113 after the linkage stage adds four names. The code: `SECTION_NAME_TABLE_SEED` at ELF/import_sections.rs:1327-1328 is the eight names from `.interp` to `.gnu.hash`, 79 bytes by count, and the last name offset is 69 (:1325). `UPSTREAM_NAME_SEED_SIZE = 113` is in the later stage (ELF/dynamic_table.rs:21). The second pass is right.
2. **Who calls `apply_relocations`.** unit_006 says it "is called at macho/lib.rs:783 and pe/lib.rs:205". unit_003 and unit_004 record ELF callers. The code has five production call sites: ELF/file_assembly.rs:2504, ELF/shared_component.rs:307, ELF/static_executable.rs:216, MO/lib.rs:783, PE/lib.rs:205. The unit 6 list is incomplete; the ELF records are right.
3. **`private_store` / `private_address` in `spill_values` (ff:13833, :13846).** unit_005 saw no caller in its half and did not grep. The code has callers in the other half: ff:6476, :6482, :7356, :8657. They are used.
4. **Where PE thunk/slot pairing is checked.** unit_006 suspected EM/image.rs holds a PE pairing check. unit_005 records that the container-specific extra checks in `validate_terminal_image_with_import_count` are Mach-O only (EM/image.rs:842-864). The code: image.rs calls three `image_macho::validate_*` functions (:843, :851, :858) and its only `image_pe::` call is the emitter (:1848). unit 5 is right; there is no PE-specific pairing check there.
5. **`scalar_shape` at EM/object_artifact.rs:3216.** unit_003 lists it among suspected duplicates of the layout rule. The code: it is a one-line delegate to `ValueShape::for_scalar`, not a layout computation. The layout copy in that file is `condition_layout` (:4469-4950), which unit_005 recorded.
6. **`precedes` helpers.** unit_004 and unit_005 each suspected the other's site. Opened both: same question, different handling of unreachable blocks (see the duplicate table).

No other contradictions found between first-pass and second-pass records. gap_001 confirms that every range unit 3 left unread is test code, and corroborates unit 3's two ELF findings (test-only final-byte verifier; no negative test for `.rela.dyn` bytes). gap_002 found no contradiction with unit 5.

---

## What I checked myself

23 claims opened in the checkout.

| # | Claim (record) | Site opened | Verdict |
|---|---|---|---|
| 1 | Final-byte verifier is test-only; admission only derives an output (unit 3, gap_001) | ELF/file_assembly.rs:184-211, :352-353; grep `validate_executable_output` | matches |
| 2 | `DynamicElfNativeArtifact::from_replayed_parts` has 0 callers (unit 2) | grep `from_replayed_parts` over omega-rust and the whole worktree | matches; also true of NA/physical/mod.rs:645 and :678 |
| 3 | `InstallationError` has no constructor or importer (unit 5) | grep `InstallationError` over omega-rust | matches |
| 4 | `admit` arms return `true` with no check (unit 5) | ff:18735-18746, :18864-18865, :18995-19007 | matches |
| 5 | `Return` admitted on empty cleanup; `PortWrite` uses `.any` (unit 5) | ff:18728-18733, :18984-18994 | matches |
| 6 | `_ => true` in execution/realization pairing (unit 5) | EM/object_artifact.rs:3598-3632 | matches |
| 7 | No family row returns `Ok(())` (unit 2) | NA/physical/derivation/provider_custody.rs:20-37 | matches; `validate_external_requirement_applications` runs first (:28) |
| 8 | `replay::validate` returns `Ok` without replay (unit 4) | ff:3988-3996, :4100-4112 | matches |
| 9 | `image_section_tag` and `section_tag` are the same mapping (unit 3) | ELF/dynamic_executable.rs:674-681; ELF/procedure_linkage.rs:571-578 | matches |
| 10 | Provider rejoin is written twice (unit 2) | NA/physical/derivation/provider_custody.rs:87-127; foreign_calls.rs:519-552 | matches |
| 11 | Hosted builtin tags swap read and write between two tables (unit 2) | hosted_builtins.rs:71-106; hashing.rs:228-238 | matches |
| 12 | `.rela.dyn` bytes not length-checked, not in unchanged loop, not hashed (unit 3) | ELF/file_assembly.rs:2227-2252, :2323-2335, :2401-2413; grep of the field | matches |
| 13 | `can_emit_executable_image` has no outside caller (unit 5) | grep count | matches |
| 14 | Entry-binding evidence has no non-`None` producer (unit 6) | grep of field and type names | matches (only a test builds them) |
| 15 | `darwin_import_library` defaults to libSystem (unit 2, unit 6) | CC/host_operations.rs:380-395 | matches |
| 16 | PE empty library falls back to `KERNEL32.dll` (unit 6) | PE/imports.rs:78-88 | matches |
| 17 | Machine-effect default arms (unit 6) | A64/machine_effects.rs:993-998, :1187 | matches |
| 18 | Integer alignment cap 8 vs 16 in three layout copies (units 2, 3, 5) | CC/terminal_layout.rs:392-410, :453-462; CC/structural_shapes.rs:470-490, :529-539; EM/object_artifact.rs:4517-4526, :4903-4920 | matches |
| 19 | `scalar_shape` at object_artifact.rs:3216 is a suspected layout duplicate (unit 3) | EM/object_artifact.rs:3216-3218 | does not match: it delegates to `ValueShape::for_scalar` |
| 20 | Export validation exists in ELF, Mach-O and PE (unit 4, unit 6 "suspected") | ELF/shared_component.rs:52-78; MO/shared_component.rs:251-282; PE/shared_component.rs:104-134 | matches; same job |
| 21 | Two `precedes` implementations are the same job (unit 4, unit 5 "suspected") | ff:5185-5200; ff:12411-12472 | partly: same question, different result for unreachable blocks |
| 22 | Descriptor-stage seed is 113 bytes (unit 3) vs 79 (gap_001) | ELF/import_sections.rs:1318-1328; ELF/dynamic_table.rs:18-24 | unit 3 does not match; gap_001 matches |
| 23 | `apply_relocations` called at Mach-O and PE (unit 6) | grep of call sites | partly: three ELF call sites missing from the list |

Also re-run as counts: `unwired, not superseded` 7 in 3 files (matches unit 2); `encoded_scalar_plan` 4 in 2 files (matches unit 6); `temporary_stack_bytes_for_function` 7 in 1 file (matches gap_002); `AARCH64_INLINE_ASSEMBLY_DEFAULT` 5, all in one file (resolves unit 6's doubt); `private_store`/`private_address` have callers (corrects unit 5's doubt); EM/image.rs has no `image_pe::` validator call (resolves unit 6's doubt).

What I did not check: whether a physical-evidence gap blocks publication. A grep for `physical_evidence_gap` outside `NA/physical/` shows it is stored, compared and hashed in `native_artifact.rs`, `dynamic_elf.rs` and `identity.rs`; I did not read the publication callers.

---

## Not covered

Records: none unread. All seven files for units 002-006 were read in full (`unit_002.md`, `unit_003.md`, `unit_003_gap_001.md`, `unit_004.md`, `unit_005.md`, `unit_005_gap_002.md`, `unit_006.md`).

Source ranges inside this area's crates that no record of this area covers (they belong to other units):
- native-artifact: `component_product.rs` (218 lines), `entry_receiver.rs` (121), `fragment_object_publication.rs` (250), `lib.rs` (88), `native_artifact.rs` (1,090). These are in unit 001. Several findings above stop at that boundary: the direct-artifact validation chain, `selected_requirement_identity`, and what a gap does at publication.
- isa-aarch64: `selected_form_encoding.rs` (18,242 lines), in unit 007 with gap files 003-005. The decoders that the A64 headers refer to are there.

Inside the units, after the second pass every line is covered: unit 3 left 5,728 test lines that gap_001 read; unit 5 left 2,195 test lines that gap_002 read. Lower-attention reads the records admit: unit 4 read the large test modules in ff and entries.rs "at lower attention" (ff:720-1469, :5883-6169, :10680-10976, :2348-2478; EM/entries.rs:639-833, :1925-1986, :3653-3728, :4253-4403, :4873-5024); unit 5 read test code "to understand what shapes it exercises, not line by line" (ff:14473-15861, :19719-19778, :22427-22489, :24196-24392). gap_002 did not read the production bodies of six functions whose tests it describes (ff:16872, :19119, :19038, :20219 past :20231, :21404 past :21416, :21618 past :21630).

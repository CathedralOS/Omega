# Record: unit 079, gap 036 (second pass), area 14-interpreter

Unit file: `units/gap_036.tsv`. Two ranges, both test code. Nothing in either range is production code.
All line numbers are in the checkout `.../worker-2/` at `4f3c255d0559`.

## 1. `source-rust/subsystems/interpreter/memory/extents/src/mapping.rs` lines 1001-1488

**Purpose.** This is the tail of the `mod tests` block (opens at `mapping.rs:913`, grepped). The range holds helper
functions and 11 tests that exercise the map/unmap lifecycle of a mapped extent: owned and borrowed-shared mappings,
the translation-install and translation-release receipts, and peer-write revocation for shared mappings.

**Shape.**
- Helpers: `source()` (1009), `destination()` (1013), `release(&PendingUnmap)` (1017), `activate(&PendingMap)` (1025),
  `revocation_obligations()` (1292), `activated_shared_mapping_with(source, id)` (1299),
  `activated_shared_mapping()` (1315), `activated_shared_mapping_with_grant(&grant)` (1319). Other helpers they use
  (`extent`, `mapping_grant`, `mapping_grant_with_revocation`, `local_extent`, `id`) sit before 1001, e.g.
  `extent` at 927, `mapping_grant` at 985 (grepped).
- Tests (all `#[test]`):
  - `owned_mapping_round_trips_both_authorities_after_translation_release` (1034)
  - `owned_mapping_round_trips_program_local_origins` (1129)
  - `provider_reads_obligations_from_grant_and_mapping_carriers` (1169)
  - `borrowed_mapping_retains_source_loan_and_shared_polarity` (1203)
  - `mapped_range_receipt_context_binds_complete_mapping_evidence_and_exact_range` (1228)
  - `mapped_range_receipt_context_rejects_invalid_ranges` (1261)
  - `failed_mapping_returns_source_and_destination_authority` (1277)
  - `peer_write_revocation_demands_the_admitted_grants_fact_set` (1330)
  - `shared_mapping_stable_loan_requires_completed_peer_write_revocation` (1374)
  - `peer_write_revocation_receipt_binds_the_exact_mapping` (1434)
  - `peer_write_revocation_refuses_non_shared_custody` (1470)

**Inputs and outputs.** Tests call the module's public surface: `map_owned`, `map_borrowed`, `PendingMap::complete`,
`MappedExtent::begin_unmap`, `PendingUnmap::complete(...).into_parts()`, `begin_peer_write_revocation`,
`stable_loan`, `loan`, `loan_mut`, `range_receipt_context`, and the receipt constructors
`TranslationActivationReceipt::from_admitted_provider`, `TranslationReleaseReceipt::from_admitted_provider`,
`PeerWriteRevocationReceipt::from_admitted_provider`. They produce only assertions. No state outside the test.

**How the tests pin the code's decisions (what the production code is keyed on).**
- A receipt is accepted only if it names the exact pending mapping: a forged mapping identity (1056-1060) or a
  substituted source origin / provider issuance (1067-1072) is refused with "exact pending mapping".
- A receipt with `installed = false` is refused ("does not establish", 1074-1082). A receipt missing a required
  fact is refused ("lacks", 1084-1092; 1103-1108).
- Peer-write revocation demands the fact set carried by the admitted grant, not the caller's set
  (1330-1371, comments at 1346 and 1359). An unrevoked receipt that lists the facts is refused
  ("revoked write permission", 1402-1414). A receipt context taken from a different mapping is refused
  ("exact active mapping", 1443-1454); the two mappings differ only in source extent id 9 vs 1 with the same
  mapping id 60 (1440-1441).
- `stable_loan` on a shared mapping fails until revocation completes (1377-1380), then succeeds (1425).
- `begin_peer_write_revocation` on an owned mapping is refused ("shared source custody", 1481-1484).
- Range receipt context equality is keyed on full mapping evidence (authority lineages) plus the exact range:
  same compact ids with different extent ids compare unequal (1235-1251); different range compares unequal
  (1252-1257). Zero length, `u64::MAX + 2` overflow and a range past the mapping are errors (1272-1274).
- A failed map returns both extents with their authority (1277-1289). The message check is "equal length" (1287).
- Identity of origins: owned round trip returns `ProviderIssued(provider_issuance(n))` and `ProgramLocal(...)`
  origins unchanged (1043-1049, 1097-1125, 1139-1165).

**Cases by shape.** Fixed fixtures only: one 0x1000-byte source at 0x1000 and one 0x1000-byte destination at
`0xffff_8000_0000_0000` (1009-1015); fact id sets are hard-coded `[100]`, `[200]`, `[300]`, 61/62/63 (e.g.
1003-1005, 1293-1296, 1332-1336). Not a code limit; test-fixture shape.

**Same job elsewhere.** The `PendingMap`/`PendingUnmap`/revocation receipt paths each repeat the same
"wrong mapping / not established / lacks facts" three-way check pattern (1051-1092, 1103-1108, 1349-1414). I read
no other implementation of the receipt check outside this file (the production side is lines before 913 and not
in this unit).

**Unused or one-caller code.** All helpers here are private test functions. `Box::leak` appears 3 times in the file
(Grep count mode on `mapping.rs`), at 1316, 1320 and 1440; each leaks an `Extent` to get a `'static` borrow so
`MappedExtent<'static>` can be returned from a helper.

**Stated limits.** None in this range.

**Fail-open risks.** Test code; none seen. Tests assert refusals. One exception worth noting: `drop(other.begin_unmap())`
(1456) discards a pending unmap without completing it; this is a test-only cleanup of the foreign mapping.

## 2. `source-rust/subsystems/interpreter/memory/materialization/src/contract_tests.rs` lines 1-3090 (whole file)

**Purpose.** Declared in the file header (1-5) as tests for "symbolic and conventional layout materialization,
nested record and sum interior path resolution, bit-fragment writer validation, and replay fingerprint stability".
The file is a `#[cfg(test)]`-style module pulled in at `lib.rs:24` (`mod contract_tests;`, grepped; I did not read
whether a `cfg(test)` attribute precedes line 24). It holds 41 `#[test]` functions (Grep count on the file),
spread over four inline modules plus shared helpers.

**Shape.**
- Imports from `layout_plans::test_support` (7-12): `data`, `deeply_nested_layout`, `entry`, `layout_field_entry`,
  `layout_report`, `numbered_layout_field_entry`, `post_handoff_context(_with)`, `record_layout`,
  `recursive_record_array_report`, `repeated_layout`, `split_layout`, `stored_integer_layout`, `sum_layout`,
  `symbolic_path`, `writer_field_offsets`. These helpers live in the `layout_plans` crate, not here.
- Module `conventional_materializations` (14-1307): fingerprint and replay-match tests, plus
  materialize/decode tests for scalar and aggregate layouts. Includes local macro `bits!` (34-44).
- Module `symbolic_materializations` (1308-1852): placement range, indexed fields, inner (record interior)
  carriers.
- Module `symbolic_sum_materializations` (1853-2707): sum and sum-array interiors, recursive report folding,
  depth bound.
- Module `writer_fragments` (2708-2953): runtime-writer plans, loader-time constant folding, placement constraints.
  Its helpers `post_handoff_split_writer_plan` (2709) and `constrained_post_handoff_plan` (2720) are defined
  before its `use` lines (2748-2754); legal in Rust.
- File-level helpers (2961-3090): `nested_layout` (2961), `record_interior` (2989), `sum_field_layout` (2996),
  `recursive_sum_report` (3013), `sum_array_layout` (3059), `pub(super) reject_inner_layout_materialization`
  (3075), which wraps `derive_symbolic_materialization_with_inner_layouts` with `post_handoff_context()` and a
  resolver returning `None`, then returns the error string.

**Inputs and outputs.** Tests call the crate's entry points: `decode_scalar_layout`,
`materialize_scalar_layout_into`, `materialize_aggregate_layout_into`, `derive_symbolic_materialization`,
`derive_symbolic_materialization_with_inner_layouts`, `materialize_resolved_into` (all re-exported at
`lib.rs`, per crate map `14-interpreter.md:27`), and `layout_plans` functions
`normalized_layout_plan_report_fingerprint`, `layout_plan_reports_match_for_replay`,
`normalized_conventional_sum_layout_report_fingerprint`, `conventional_sum_layout_reports_match_for_replay`,
`SymbolicFieldInnerLayout::from_recursive_sum_paths`, `PlacementConstraints::validate_site`. Outputs are
assertions on returned bytes, plans, and error strings (`error.0.contains("...")`).

**How the tests pin the code's decisions (what production is keyed on).**
- Identity versus spelling. Reports with a `member_identity` rejoin by identity and ignore the field spelling
  (183-242, 436-510, 978-1064, 2036-2073, 2445-2548). Reports without identity are keyed on spelling: renaming
  an unnumbered field changes the fingerprint and fails replay (108-113, 233-241, 166-173). Two names under one
  identity are refused before any write or resolver call ("identity names both", 245-317). Repeated identity
  in supplied values or schemas is refused ("repeats stable member identity #7", 1042-1063). Drift of the
  identity between layout and schema is refused ("same stable identity", 461-473, 1020-1040).
- Fingerprint: order independent over entries, bound to geometry and schema fingerprint (47-66, 193-231);
  `size: None` differs from `Some(u64::MAX)` (320-337).
- Atomicity: every rejection test asserts the destination buffer is unchanged (e.g. 263, 316, 390, 432, 689,
  711, 868-975, 1031, 1153, 1163, 1186-1197, 2866-2868).
- Static preflight before the resolver: geometry/fit/index/path errors are asserted to fire with zero resolver
  calls (274-284, 1209-1240, 1374-1398). A resolved value that does not fit the stored width is rejected after
  one resolution of the exact target, and one resolved value serves every write for that target (1243-1306).
- Aggregate layouts: whole-extent `At` placement requires the value length to equal the compiler-derived extent
  ("compiler-derived extent is 6", 381-389); mixing `At` with scalar placements per field is refused (847-868);
  fragment overlap is checked on physical bytes in both byte orders and across different container widths
  (714-823); an outer fixed array cannot carry scalar placements (870-890); stored-integer placement cannot
  overlap another placement (913-941); a field wider than the 64-bit scalar carrier cannot fragment (892-911).
- Repeated aggregates need one constant non-overlapping, aligned stride and the exact element count (513-559).
- Loader versus post-handoff: a split pointer or stored-integer field with an unresolved target cannot be
  consumed before Omega entry (1838-... see 2838-2860); the same plan under a post-handoff context becomes a
  `RuntimeWriter` (2825-2836). A resolved plan can be tampered with and `materialize_resolved_into` re-checks
  fit and refuses before writing (2796-2810); an unresolved action cannot partially materialize (2863-2868).
- Placement: plan alignment is the join of layout and policy alignment (2874); `validate_site` is keyed on
  phase, alignment, machine regime, installation scope, and the address range (2870-2952).
- Inner carriers: each of these is refused with a distinct message: missing carrier (1417-1436), carrier no path
  traverses (1438-1449), carrier for an absent field (1451-1464), duplicate carrier (1466-1476), unsized
  carrier (1478-1496), unindexed hop over several element placements (1502-1533), non-`At` enclosing
  placement (1535-1589), missing member (1591-1604), duplicate path (1606-1624), deeper path faults
  (1628-1710), interior larger than its enclosing extent (1805-1831), leaf outside its record (1833-1850).
- Sum interiors: path must end at a payload field below a case (2079-2092); unknown case or payload
  spelled (2094-2124); path continuing below a payload (2126-2141); element index on case or payload hops
  (2143-2179); payload wider than its slot (2181-2194); sum interior may not nest carriers (2234-2251);
  payload outside sum extent (2252-2268); zero-element, stride-overlap and drift of repeated sum arrays
  (2290-2321, 2357-2441); empty-interior carriers still validate geometry, and a malformed descendant under an empty
  ancestor is refused (1871-1957).
- Depth bound: `CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT` = 64 per the message text at 1996 and 2030; a path of
  exactly the bound fails for another reason (no entry, 1960-1980), one past it fails with the bound message
  (1982-1999); a carrier tree deeper than the bound fails (2001-2033); report folding at the bound succeeds
  and past it fails (2551-2600).

**Cases by shape.** Tests assume fixed fixtures in the `layout_plans::test_support` crate (not read). Production
cases the tests reveal as the supported shapes: `LayoutPlacementReport::{At, IntegerAt, Bits}` (569, 1072,
36); `IntegerInterpretation::{Signed, Unsigned}`; inner layouts as record, sum, record-array, sum-array
(1875, 1947, 2071, 2290); `ConventionalRecordSumChildHop::{Field, Index}` (2576, 2667). A repeated record/sum
element requires an explicit index (2327-2340, 2630-2646); a case hop or payload hop cannot be indexed
(2143-2179).

**Same job elsewhere.** The inner-path validation messages for records (`1417-1710`) and sums (`2076-2321`) are
two parallel families of checks (suspected two implementations in `symbolic_materialization.rs`; I did not read
it). The fingerprint and replay-match functions for layout plans and sum reports (47-243) are a second parallel
pair (suspected in `layout_plans`; not read).

**Unused or one-caller code.** `reject_inner_layout_materialization` is `pub(super)` (3075) and is used by the
`symbolic_materializations` and `symbolic_sum_materializations` modules; Grep count for the name in this file is
not run here beyond reading (uses visible at 1439, 1452, 1466, 1485, 1522, 1553, 1579, 1594, 1609, 1640, 1655,
1680, 1694, 1758, 1793, 1819, 1840, 1970, 1989, 2023, 2082 and later). All other helpers are private test
fixtures. In `symbolic_sum_materializations`, the `use` list names `nested_layout`, `numbered_layout_field_entry`
and `deeply_nested_layout` etc. (1854-1858); I did not check whether each is used.

**Stated limits.** The file mentions "past the 64-bit scalar placement carrier" (908) and "the compiler's 64-segment
record path bound" (1996, 2030, 2596) as asserted refusals. No comment in the file calls anything unsupported or
migration debt.

**Fail-open risks.** Test code; none seen. One place where tests accept a shape that could look wrong:
`symbolic_inner_materialization_bounds_record_path_depth` first case (1960-1980) asserts the path at depth
`LIMIT - 1` is rejected for a missing entry, not accepted (comment at 1974); that fixture does not prove a path
exactly at the bound is accepted end to end.

## Coverage

- Read in full: `mapping.rs` 1001-1488 (488 lines); `contract_tests.rs` 1-3090 (3090 lines). Total 3578 lines.
- Skimmed: none.
- Not read: none of the unit's ranges. Outside the unit, I did not read `mapping.rs` 1-1000, `lib.rs`,
  `symbolic_materialization.rs`, `materialization.rs`, or the `layout_plans` crate's `test_support`.

## Five things a maintainer most needs to know about this unit

1. `mapping.rs:1001-1488` tests pin that every provider receipt (install, release, peer-write revocation) must
   name the exact pending or active mapping, and that revocation demands the fact set carried on the admitted grant,
   not one the caller supplies (1330-1371).
2. `contract_tests.rs` asserts atomicity everywhere: each rejected materialization leaves the destination bytes
   exactly as they were, and the static geometry and fit checks run before any resolver call (274-284, 1209-1306).
3. Numbered fields (with `member_identity`) rejoin by identity and ignore spelling; unnumbered fields are keyed on
   spelling, so renaming them changes the fingerprint and fails replay (183-242).
4. A symbolic path must be traversed by exactly one supplied inner layout, and each path hop has its own refusal
   (records 1417-1710, sums 2076-2321, depth bound of 64 at 1960-2033).
5. A loader (before Omega entry) cannot take an unresolved split or stored-integer write; the same input under
   a post-handoff context becomes a `RuntimeWriter` (2825-2860).

## What I am unsure about

- Whether `mod contract_tests;` at `lib.rs:24` is gated by `#[cfg(test)]`; I only grepped the line.
- The behavior of the `layout_plans::test_support` helpers (`split_layout`, `sum_layout`, `deeply_nested_layout`,
  `recursive_record_array_report`); the exact geometry the tests rely on comes from there and was not read.
- The first-half definitions in `mapping.rs` (`extent`, `mapping_grant`, `local_extent`, `id`) were not read, so
  the meaning of the numeric arguments (e.g. `extent(1, 0x1000, 0x1000, 10, 20, &[100])`) is inferred from use.
- In `symbolic_sum_materializations`, whether every imported helper is used.

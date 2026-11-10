# Area 13 digest: Validation, first half

Arithmetic domains, borrows, call cycles, write frames, contract entailment.

Source: records `unit_088.md` to `unit_096.md` (nine records, no second-pass gap records exist for these units), checkout `worker-23` at `457adf4a11f`.

File abbreviations, all under `omega-rust/psi/compilation/validation/`:
`ab` = `src/access_views_and_borrows.rs` (3,931 lines), `ad` = `src/arithmetic_domains.rs` (26,886), `br` = `src/borrow_and_reflection.rs` (6,376), `cc` = `src/call_cycles.rs` (4,256), `calls` = `src/calls.rs` (44,215), `cs` = `src/completed_storage_and_conservation.rs` (6,993), `ce` = `src/contract_entailment.rs` (91,057; this area covers 1-72848), `rs` = `representation-planning/src/representation_selection.rs` (492), `rp` = `representation-planning/src/lib.rs` (81), `sel` = `representation-planning/selections/src/lib.rs` (260).
A tag such as [088] names the record a statement comes from. "checked" means I opened the lines myself (see "What I checked myself").

## What this area does

This area is the part of the compiler that decides whether a typed program keeps its promises. It takes the typed program (`&dyn CheckingInput`) and answers questions such as: does this integer arithmetic fit its type, may this call write that storage, does this recursion end, is this `requires` clause true at this call. Almost every function either pushes an error `Diagnostic` or returns `None`/`false`/`Unknown` to mean "not proven". Nothing here rewrites the program. Seven very large files hold nearly all of it; each file is a tree of inline `mod` blocks, not separate files ([088] ad:1-5; [090] calls:1-5; [093] ce:1-4).

**`psi/compilation/validation/representation-planning/selections` (260 lines).** In: a validated `Name satisfies OpaqueRepresentation<Opaque, Carrier>` choice. Out: the stored record `OpaqueRepresentationSelection` with a 32-byte commitment that can be recomputed from its fields (sel:35-47, 72-101, 139-146) [088].

**`psi/compilation/validation/representation-planning` (573 lines).** In: build-declared `Build::select_representation` requests and executed build events. Out: a list of validated selections or diagnostics; it also re-derives retained selections against the final typed graph (rs:251-384, 386-465, 467-492) and recognises the toolchain trait `OpaqueRepresentation` (rp:38-69) [088].

**`psi/compilation/validation` (331,160 lines in 29 files; this area's records cover 165,505 lines in 7 files).**
- `ab`: checks retained access-view plans against the typed program and the `&write` (write-only reference) discipline (ab:5-1579, 1581-3929). Out: diagnostics only [088].
- `ad`: an abstract interpreter over integer and float values. In: a state's statements and contracts. Out: an `Analysis {recipe, domain, interval, primitive}` per expression (ad:8383), deferred obligations (`ArithmeticDecisions`, ad:8420-8429) and diagnostics. Its fact store is `ValueEnvironment` (ad:25856) [088, 089].
- `br`: `borrow_contracts` decides which inputs a returned borrow comes from and whether one callable contract refines another (br:7-3619); `reflection` builds and replays schema graphs, selection snapshots and visitation plans (br:3621-6376) [089].
- `cc`: builds the machine call graph, finds cycles and admits them under three regimes: proof-only ranking, runtime tail calls with a ranking, selector self-recursion with a witness (cc:15-567, 1190-3369, 3783-3796) [090].
- `calls`: validates every call site (calls:1-6400), then "write frames": for a call or statement, the complete list of caller places it may write, as string paths, or "opaque" (calls:6550-39721), then argument correspondence and call-site numbering (calls:41622-44205) [090, 091, 092].
- `cs`: `completed_storage` walks the flow checker's storage-snapshot graph to find which stores supply a place (cs:5-4674); `content_conservation` and `content_projections` check `Content<Owner, A>::project` equations (cs:4676-6993) [092].
- `ce` (to line 72848): the integer proof `Engine` over polynomials (ce:8-7563), call-requirement and citation proofs, bounds proofs, contract refinement and law conformance, the termination ranking prover (ce:24061-34063), specification validation, and the structural judge that walks proof bodies (ce:43720-72848) [093, 094, 095, 096].

## How it decides

**By identity (symbol or handle).** Opaque selection lookup and duplicates (sel:163; rs:443-454) [088]. Cycle membership by machine position (cc:63, 3780) [090]. Most call-target resolution (`call.target_symbol`, calls:1-6400) [090]. Argument correspondence by symbol, position and normalized type identity (calls:42528-42541) [092]. Strict-binding mode of the Engine by `SymbolHandle` (ce:7040-7050) [093]. Trait operation slots by symbol plus normalized type identity (ce:18602-18627) [094]. Constructor identity by carrier and member (ce:48852) [095].

**By place.** Storage-snapshot queries compare root and canonical segments (cs:49-67) [092]. Stored reference leaves keep `PlaceSegment` vectors and decide coverage on them (calls:35265, 35282, 35972) [092]. Ranking coordinates are (parameter symbol, place path) (ce:24153) [094]. Borrow leaves are matched by `(source, reference_hops, owner_path)` (br:414-421) [089].

**By value.** Intervals of `Option<i64>` ends (ad:16226); arithmetic obligations keyed on `ArithmeticDomain` and the numerics policy bridge (ad:5008, 5696-5697, 6436-6441, 6671) [088, 089]. Polynomial lower-bound proofs over stored `(Polynomial, BigInt)` pairs (ce:3378-3493) [093]. Three-valued judgments `Proven / Refuted / Unknown` throughout `ce`.

**By position.** Call-site ordinals from one traversal (calls:43407); invocation outputs are identified by `(state, statement_index, call_ordinal, parameter)` (calls:33487-33492) [092]. Bounds demands by `(state, statement_index, expression position)` (ce:10152) [093]. Slot-to-layout correspondence by sorted position (ab:887-913) [088]. Statement identity by memory address of the `StatementNode` (calls:14797, 15119; calls:11065-11067) [090, 091]; shared proof walks by machine address (ce:37969-37974, 38012) [095]; cached structural answers by pointer identity of 20 `Arc` tables (ce:66561-66649) [096].

**By source spelling or generated text.** I counted distinct cited locations (one line or line range where a judgment reads a name, label, path string, rendered text, or `format!` text): **263 in total**, counted mechanically from the list below. 203 read program names or path text; 60 read text the compiler built itself (`format!` of handles, positions or debug values). Per file:

| File | Program names / path text | Compiler-built text |
|---|---|---|
| `rp` | 2 | 0 |
| `ab` | 8 | 0 |
| `ad` | 21 | 2 |
| `br` | 8 | 4 |
| `cc` | 6 | 0 |
| `calls` | 62 | 2 |
| `cs` | 18 | 0 |
| `ce` | 78 | 52 |

The sites, by what they key on:
- *Toolchain declarations recognised by name plus file origin:* `"OpaqueRepresentation"` and path suffix `core/representation.omg` (rp:46, 59-68; checked); `"AccessCustody"` and `core/layout.omg` (ab:42, 638-652); trait leaf `"Content"` with one requirement `"project"` (cs:6845-6854); algebra names `"IntervalSet"`/`"CountedQuantity"` (cs:6537-6542); `"Nat"` with `Zero`/`Succ`/`prev` (cs:6656, 6668, 6715-6730; ce:23332, 23346-23353, 23358-23361, 23392, 23532-23538, 23749-23751); `"saturating_sub"` (ce:23343, 23469); embeddable scalars `"u8"|"u16"|"u32"|"u64"|"addr"` (cs:6867) [088, 092, 094].
- *Place facts keyed by path string:* `place_path` joins names with `.` (ad:22110-22135); fact keys `format!("{path}{suffix}")` (ad:4531, 4547-4561); prefix scans accepting a tail that is empty or starts with `.` or `[` (ad:4413-4421, 4662-4676, 25840-25853); write overlap by string prefix (ad:2918-2977; calls:16026-16043, checked; calls:11012-11025, 13433, 37281, 37294); float field paths from symbol names (ad:395-399); `"<path>.len"` as the slice-length key (ad:23116, 23174, 23224); `param.field` seeds (ad:13219); survival of facts across unknown writes by declared name (ad:26164-26185); difference-constraint atoms by name (ad:1440-1461); `Operand::Place` equality includes its path string (ad:20801-20834); `u64::MAX` written as a decimal string (ad:21730) [088, 089, 091, 092].
- *Aliases, parameters, locals and receivers looked up by name:* calls:1882-1917, 1939, 7611-7642, 8259-8289, 9057-9065, 10162, 10662-10676, 12125, 12128, 17677, 17700, 17703, 17709, 17820, 17923, 19438, 19484, 20719, 22657, 22667, 22890, 26142-26186, 34396-34413, 34517-34584, 34608-34623, 34938, 39311-39356, 41298-41305, 41484-41486, 41505; ce:6171-6179, 6599-6600, 7056-7064, 9876-9879, 16057, 18506-18509, 21618, 21766-21771, 34293, 34325-34345, 37842, 40340, 40358, 42934, 43083-43101, 50623, 63350, 63770-63777, 65991, 68899, 70668; ad:24884-24891; cc:4203-4211 [088-096].
- *Types, data, fields, variants, machines and states by name:* calls:17145, 17443-17447, 17535-17540, 19304-19321, 20613-20630, 20789-20792, 20981, 36777, 36783, 38119, 38318, 7370, 8559, 7931, 8007, 9539-9544 (state named `target`, else `"entry"`, else first); ab:671-677, 1025-1026, 1093, 954-963 (`"{}::plan"`), 1142-1151; ab:2221-2227 (`starts_with("Atomic")`); ad:12734-12736 (field name unique by count); cs:5019, 5303, 5345, 5357, 5364, 5468, 6603-6609, 6615-6630, 6919-6936, 6983; ce:16029-16034, 16062, 16067, 16083-16086, 17840-17851, 18650-18651, 18766, 20665, 20763-20768, 20805, 27945-27954, 31855, 31874, 31903-31912, 49607-49610, 52449, 57950-57957, 63410, 63426, 63433-63447 (`"Ready"`/`"Rejected"`), 63470, 71154-71165; br:1436, 2619-2620 (lifetime names), 3689-3703, 3863, 3897, 4566-4581, 5039, 5088 (identity strings scanned for a unique match) [088-096].
- *Call targets and operations by text:* `asm#...` mnemonic list (calls:874-905), `starts_with("asm#")` (calls:39824), builtin name flag (calls:16335); `"encode"|"decode"` (calls:2062-2069, checked; calls:39210-39217); last `::` segment equals target (calls:932-958); accessor `operation` strings (calls:564, 3500-3507, 41609); citation exemption by machine name (calls:6057-6069, checked); relation call by last-segment name (ce:24041-24050) [090, 092, 094].
- *Ranking views by path text:* `view_path` equals canonical path (cc:1520-1521, 1548-1549, 1581-1582), split on `::` (cc:1681-1687; ce:16595-16605; ce:26451-26500), owner display path (cc:1695-1697), `""` or `"Slice::Length"` (calls:5310-5315), length member by authored spelling (ce:30281) [090, 093, 094].
- *Rendered or display text compared:* Boolean law labels (ce:18335, 18378-18392, 18419-18423); transparent relation formula (ce:23940-23959); canonical source key (ce:17261-17263); diagnostic owner label (ce:41219-41222); carrier identity text (cs:3860, 5065-5066) [092, 093, 094, 095].
- *Other text tests:* state name `contains("const")` (ce:34157-34166, checked); local name `starts_with("__")` (ce:44685-44687); `Opaque` term text `starts_with(name)` then `.` (ce:51173-51179); variable-name parse `$static-type-predicate:` (ce:52086-52106); const argument `parse::<i128>()` (ad:3377-3388; ce:43436); operation identity text equality (ce:38798); diagnostic dedup by message prefix (ad:2666-2671, 7457-7463) [088, 094, 095].
- *Compiler-built identity text (60):* Engine atom names (ce:2634, 2931, 2961, 3013, 3073, 3837, 4002, 4050, 6476, 7319, 7433, 7457) and `integer_source_identity` (ce:7504-7562); completed-value identities (ce:13517, 13660, 20376, 20811, 21654); ranking atoms (ce:26728, 26729, 27868-27871, 28285, 28615, 28654, 33255, 33927); scoped atoms (ce:36081, 36361, 36473, 36476, 36661, 42976, 43308, 43336, 53155, 55048); selected-operation and result ids (ce:38246, 38865-38891, 39997); synthetic binder names (ce:47138, 50137, 50167, 50263, 51007, 51046, 69153, 69173); `format!("{term:?}")` as sort key, hash bucket or atom (ce:43757, 47234-47235, 58147, 58149); atom prefix test (ce:54206); `"invocation result @n:g"` (calls:8895-8899, 20477); `format!("\0capture:{anchor:?}")` (ad:22172); `observation:{position}` (ad:18572); declaration facts compared as `{:?}` text (br:3889, 4024-4026, 4163-4165, 4177) [089-096].

[095] notes that the hash and sort uses at ce:43757 and ce:47234 are followed by payload equality (ce:43767, 47186-47189).

One record calls the joint-guard recognisers "spelling of a pattern" (ad:14401, 14428-14435, 14500, 14536-14543) [089]; they match a fixed expression shape against a literal equal to the type's extreme value. I list them under shapes and did not count them here.

## The same job in more than one place

Site counts are distinct cited locations.

1. **Limits of an integer type** (11 sites): `primitive_range` ad:15226; `signed_primitive_bound` ad:14506; `carrier_integer_range` ad:23082; `integer_carrier_ceiling` ad:21726; inline ad:24167-24176; `signed_minimum` ad:4975; `PrimitiveType::integer_bounds()` used at ad:15257, 18484, 23083; ce:33733, 33750, 35997; `integer_value_lands` ce:2137 [088, 089, 093, 094].
2. **Interval arithmetic** (6 stacks): `Interval` i64 ad:16226; `BigIntInterval` ad:899; `IntegerRange` via `selected_scalar_intervals::binary` ad:22843; `IntegerSet` ad:16481; `unsigned_representability` in u128 ad:25662; `bitwise` ad:1590 [088, 089].
3. **Comparison turned into a bound** (6 sites): `BigIntInterval::from_comparison` ad:1132-1152; `comparison_interval` ad:13781; `float_bound_from` ad:9136-9177; `comparison_bound` ad:13744; `values_relation` ce:37666-37724; `comparison_holds` ce:53242-53270. [088] and [089] each pointed at the other's site; I opened ad:1132-1152 and ad:13781-13800: same five-operator table. They differ in number type (BigInt vs saturating i64) and in the unmatched case (`None` vs unbounded).
4. **Result interval of a call from the callee's `ensures`**: integers ad:1807-2019 and floats ad:9327-9502, "same algorithm shape written twice" [088]; also ad:17561, ad:22353, ad:9943, ce:35284 [089, 094].
5. **Hand-written walks over every `ExpressionNode` child** while `crate::queries::expression_traversal` exists and is used at ab:1309, ab:3884, ad:4792, ad:8715: ab:1306-1393, 3491-3603, 3879-3927; ad:11941-12084, 12236-12292 [088].
6. **Shift-count obligation**: ad:61-124 (abstract specs) and ad:6665-6730 (executable code), near-identical message [088].
7. **Carry facts to a new place by re-keying path strings**: `copy_retained` ad:4655-4677 and `rebound_paths` ad:25840-25853 (used by `rebind` ad:26062, `copy_path_facts` ad:26100). [088] suspected overlap with `ordered_values`; [089] listed the environment side. I opened both: the two string functions do the same job (strip the source prefix, accept a tail that is empty or starts with `.`/`[`, prepend the destination). `rebind_relations` ad:20688 does it for structural `Operand` relations instead of strings.
8. **Drop facts that overlap a write**: `Operand::survives*` ad:20877-21029; `invalidate_storage_by` ad:26294; `EvaluatedWritePrefix::overlaps` ad:2918-2977; all end in `frame_paths_overlap` calls:16026 [088, 089, 091].
9. **Do two places overlap / is one a prefix of the other** (6 sites, two key forms): strings at calls:16026 and `place_suffix` calls:37331 (used 37281); segments at `prefix_matches` calls:35265, `path_covers` calls:35282, `source_reaches_leaf` calls:35972 (different wildcard rules for `Index`/`FixedIndex`); canonical segments at cs:49-67 [091, 092].
10. **Write set of a call: "known, boundary, requirement, syntactic" ladder** (3 copies): calls:15459-15478, 16275-16313, 27161-27187 [091]. Checked: all three chain the same four functions; the gates differ (15470 skips the last rung on a divergent actual; 16290 and 16302 also require an exact or authority receiver; 27176 also tests a divergent receiver).
11. **Effects of a call dispatched by callee kind**: `invocation_reference_effects` calls:33180 (slot effects) and `known_call_written_paths_for_parts` calls:39196 (path strings) [092].
12. **Resolve a call shape** (named conformance, dynamic selector, machine parameter, machine state, boundary signature) in three orders: calls:665-734, 3083-3161, 5113-5136 [090].
13. **"Is this call the enclosing machine's own entry"** (7 sites): calls:5980-5991, 5507-5530, 5945-5978, 932-960; cc:3276, 3296, 2494-2500 [090].
14. **"Is this component a cycle"** written inline at 6 sites: cc:88-91, 302-309, 520-527, 686-690, 1092-1095, 3185 [090].
15. **Statement walks that collect call targets**: cc:3647-3703, cc:4038-4131, `collect_call_sites_in_statement` calls:44094 [090, 092].
16. **Where does a reference-valued expression point** (5 resolvers, different alias models): calls:18172, 20817, 22708/22803/22900, 25217, 38075 [091, 092].
17. **Fixed point over a state graph for reference effects**: `state_equations::solve` calls:24531; `solve_state_effects` calls:28040; cycle frames calls:11396; finiteness of substitutions asked at calls:15237, 24853, 28284 [090, 091].
18. **Type-tree walkers with a `visiting` list** (6): calls:17378 (caller isolation), 9108/9304 (may storage hold a referent), 32346, 38288, 36446; br:1477 [089-092]. Checked for `dyn Trait`: calls:17528 answers "isolated" (`true`), br:2045 answers "no borrow leaves" (`true`), calls:9220 answers "may hold" (`Some(true)`). The records do not say which is intended.
19. **Dynamic slot to its realization** (3): calls:32231, 32434 match rows on `(requirement, declaring_trait)` and require a checked body; calls:41102 matches on `requirement` only and does not require a checked body [092].
20. **Equality of `ReferenceLeaf` / `AggregateMove`**: derived `PartialEq` (calls:18825) but compared field by field at calls:24731-24739, 24753-24760, 25768-25775 and 24670-24676, 24770-24774, 25719-25723 [091].
21. **One method under two names**: `expression_is_fixed_incoming_reference` calls:14826 and `expression_borrows_fixed_parameter_address` calls:14861 (checked: identical bodies) [091].
22. **`source_root_type`** defined twice with different rules: calls:19785, 23172 [091].
23. **Reference access compatibility** (4 sites, 2 rules): equal-or-Mutable at cs:2439, 2566 and br:3403-3411; strict equality at cs:2975 [089, 092].
24. **Projection-machine validity cascade** written 3 times: cs:6276 (with diagnostics), 6418 (silent), 6466 (boolean) [092].
25. **Skip statically unreachable code and short-circuited `&&`/`||`**: call-ordinal walk calls:43567, 43623 and effect walk calls:31632, 31757. Ordinals stay aligned only if both skip the same nodes (inferred, not traced) [092].
26. **Expression or term to polynomial** (7): ce:847, 6938, 5082, 4747, 27311, 54800, 54955 [093, 094, 096].
27. **Does a value fit its integer type** (6): ce:969, 2137, 4921; ad:15246, 24162; ce:21900-23178 (per occurrence) [089, 093, 094].
28. **Split a Boolean guard** (`x == true/false`, `!`, `&&`, `||`) (9): ce:489-585, 5427-5486, 16516-16546, 24014-24027, 33981-34062, 35665-35730, 47020; ad:13858; calls:16917 [089, 091, 093, 094, 095].
29. **Flatten a conjunction** (5 in area): ce:6900, 8976, 8988, 15650, 17692; outside, `checks_contracts_exits.rs:5061` (grepped) [093].
30. **Prefix-walk scaffold** ("acyclic: invocation prefix, else runtime prefix, then private arrivals") 3 times: ce:8378-8478, 10405-10445, 13047-13119; 18 `StructuralPrefixVisitor` impls in the file, each with its own `seen/proven/unresolved` bookkeeping [093].
31. **Provider asks no more, guarantees at least**: `check_contract_refinement` ce:15122 (identity or arithmetic implication) and `check_operator_contract_conformance` ce:17999 (equality-only rung) [093].
32. **Three-valued truth tables**: ce:34689, ce:43619, `StructuralJudgment::connective` (used ce:53685, 71386; defined outside this area), calls:16911 [091, 094, 095, 096].
33. **Proof-body traversal** written twice: prefixes ce:44705 and leaves ce:45421; 44-line blocks at 45040-45085 and 45723-45773 are near copies [095].
34. **Generic data parameter-to-argument list** 3 times: ce:50554, 51205, 51678 [095].
35. **Strict descent of a recursion measure** (7 provers): `proof_ranking` cc:415, `proof_intervals` cc:256, `project_component` cc:2582; five judgments at calls:5597-5638; `discharges_strict_decrease` ce:16581; `prove_ranking_range_call` ce:25476; `compare_captured_integer_rank_sequences` ce:52769 [090, 093, 094, 095]. They read the witness's ranking view four ways (see spelling list).
36. **Toolchain-owned check with "no source metadata means owned" fallback**: cs:6880 and ce:23537 [092, 094].
37. **Commitment recomputation on two record types**: sel:139-146 and `omega/compilation/realization/provider-planning/src/calling_policy_plans/opaque_representations.rs:71` (suspected by [088], not read by it). Checked: both are short methods calling the one shared `selected_application_commitment` with the same four inputs.
38. **Byte size and alignment by hand**: ab:792-840; suspected duplicate of provider-planning `data_layout_queries` (not read by [088], not opened by me).
39. Smaller repeats: float narrowing for plain and element places ad:9202-9233, 9234-9265 [088]; total-specification premise loop ad:24534-24624 and 25387-25423 [089]; longest-path loops ad:19305, 19762 [089]; "unique by identity string" br:3893, 5035, 5081 and member lists built two ways br:5433, 5512 [089]; exclusive-reborrow walks calls:17776, 18001 and `with_invocation` calls:16581, 16609 [091]; three `*_result_origins` calls:8654, 8910, 8963 and `may_declare_origins` closure calls:11091, 11147 [090]; boundary-leaf computation calls:28325 and inline 24980-25001 [091]; unknown-field diagnostic built twice calls:2327-2364, 2446-2484 and one diagnostic text at calls:3233, 3520, 4892, 4979 [090]; "length >= 0" hypothesis ce:28866, 29758, 32625 and "requires facts as expressions" loops ce:18226, 29394, 30894, 32802, 33343, 33775 [094]; `OperationObservations` literals ce:64106, 64149, 64183, 64218 and algebra-law shape detectors ce:57277, 57647, 57807, 57849 [096]; depth caps as literals, 48 matches of `depth >= 128` / `depth >= 64` in `ce` [093].

## Fixed lists of shapes

- Opaque carriers: "v1 carriers must be inert storage" (rs:219); two-operand request only (rs:260); `Reference`, `Slice`, `DynamicTrait`, `ConstExpression` rejected, `FixedArray` needs a literal length (rs:155-157, 200-212) [088].
- Access custody: exactly three `Named` trait arguments (ab:44, 654-662); nested records "acyclic, non-generic, case-free" (ab:746); depth 24, 64, 256 (ab:191, 522-523) [088].
- Write-only referees: the admitted list in the diagnostic at ab:2192; fixed-array range replace "currently requires literal bounds" (ab:2987) [088].
- Integer policy only for Add, Subtract, Multiply, Divide, Modulo, ShiftLeft, ShiftRight (ad:5835-5846); `signed_minimum` only I8-I64 (ad:4975-4983); seeded comparisons only `< <= > >= ==` (ad:3506-3512); match-arm narrowing only for `Value` and `Destructure` patterns (ad:2763-2771) [088].
- Joint guards: only `x <= MAX / y` on unsigned types with `y.low >= 1` (ad:14417-14436), signed forms only against MIN/MAX (ad:14506-14517), negation only `MIN+1` (ad:14536-14543); seven separate pair sets store them (ad:25867-25873) [089].
- `exact_unsigned_widened_multiply_fits`: only a cast from U8/U16/U32 to U16/U32/U64 (ad:15312-15332); monotonic update only `literal + x`, `x + literal`, `x - literal` (ad:18604-18635); `build_operand` fixed node list, `_ => None` (ad:21442); `checked_integer_binary_kind` 36-arm table (ad:22962) [089].
- Borrows: an elided result borrows from exactly one reference parameter with exactly one leaf (br:3445-3454); provider outputs only through exclusive parameters (br:2422-2446) [089].
- Ranking orders: closed list of seven (cc:1475-1495); "a member lacks a supported exact ranking witness" otherwise (cc:2594); range proof for six of the seven (cc:3116-3124); `BoundedDistance` exactly two subjects (cc:1523) [090].
- Proof self-recursion: "the structural measure must be a bare parameter name (rung 1); compound measures over proof data are not proven yet" (calls:5560-5562); `guarded_integer_predecessor_call` exactly `measure - 1` under `measure > 0` (calls:5382-5462) [090].
- Calls: `boundary_return::is_admitted` only a body of `[LocalData, Expression(name of that local)]` (calls:1361-1366); seven build operations with fixed argument counts (calls:6419-6457); nine compiler exemptions for bodyless boundary symbols (calls:3743-3757); twelve builtin scalar atoms (calls:14158-14173); asm result types by explicit list (calls:40911-40929) [090, 092].
- Write frames: syntactic fallback handles Borrow, Name, Member, Indexed, refuses literals and calls, skips the rest (calls:16364-16374); reference leaves `_ => return None` for unlisted types (calls:19377); stored destinations only Field, Case, FixedIndex, or Index with empty expression (calls:28714-28721); "No computed/indexed/cast source is reconstructed from its spelling." (calls:13085) [090, 091].
- Content projections: one state, one statement (cs:6554-6572); scalar fragment list (cs:6652-6739), "closed projection fragment" (cs:6381); "old/current structural places admit only literal fixed-array indices" (cs:5165-5167) [092].
- Engine: `condition_paths` handles Boolean, Atomic, `!`, selected Binary only (ce:489-585); `normalize` node list with Modulo only by a constant (ce:6968-7343); operator contracts: "The first checked-software rung deliberately admits ... typed identity, type equations, declared Boolean calls, and exact `&&` conjunctions." (ce:17986-17997) [093].
- Inductive judgment: exactly one state (ce:16222); view paths `["Nat","IncreasingTo"]`, empty, or `["Nat","Descending"|"BoundedDistance"]` (ce:16595-16605) [093].
- Ranking prover: six measure kinds (ce:32564); `scalar_entry` four view ids, `_ => return None` (ce:26501-26557); `admitted_carrier` node list (ce:28898-29163); captured entry supports four of six kinds (ce:31093-31095); `FieldProjection` needs a `U64` leaf (ce:31880-31889) [094].
- Structural judge: `recognize_guarded_structural_value_arms` accepts one state with exactly two transitions of a fixed form (ce:44281-44331); specification facts by a fixed term list (ce:42611-42690) [095].
- Algebra laws by exact shape: commutativity `op(x,y) == op(y,x)` (ce:57833-57846), associativity (ce:57849-57934), left distributivity only (ce:57647-57728), "detected by SHAPE, not by name" (ce:56927-56935) [096].
- Placement results: cases `"Ready"`, `"Rejected"`, anything else `return None` (ce:63433-63447); returned-field equations only for fixed-integer or Bool fields (ce:68818) [096].
- Work bounds as literals: `1 << 20` conditional work (ce:6237); 4096 (ce:4080, 4275, 12938, 17042, 36508, 43219); 8192 (ce:48929, 66868); 65,536 and 128 (cs:3699, 3614); 4096 projections (cs:1696); 256 terms and 65,536 comparisons (ad:19744-19745); `states.len()+1` rounds (ad:10885). Records say exceeding a cap gives `None`/`Unknown`/`false`, except exhausted conditional work, which is a definitive `false` (ce:3906-3908) [088, 089, 092, 093, 095, 096].

## Accepts when it does not know

### Classed "open" by the readers (no reason at the site, not traced)

Where a record names the wrong program that could pass, it is given. Otherwise the record did not say.

Representation planning
1. rs:136-142 accepts any primitive as an inert carrier; `primitive_type_reference` not traced [088].

Access views and write-only borrows
2. ab:27-189 an ill-formed `AccessCustody` conformance that hits one of six `continue`s gets no agreement check; not traced whether earlier stages reject it [088].
3. ab:1294-1305 (1412, 1415-1420) an atomic accessor use on a place whose declared type cannot be computed is not checked [088].
4. ab:1444-1463 no binding-private check when the call target matches no accessor state and the receiver path does not resolve [088].
5. ab:2993-2996 an out-of-range or inverted fixed-array range store to a write-only array gets no diagnostic here; not traced whether another stage rejects it [088; checked].
6. ab:3365, 3368-3376, 3406-3408, 3416-3418 a call through an unresolved target with a `&write` input is not flagged here [088].

Arithmetic
7. ad:7791-7793 (with 5658, 5717-5718) arithmetic whose operand types do not resolve and that has no target type skips representability and division checks [088].
8. ad:838-864 range diagnostics are written to a throwaway list; presumed reported elsewhere [088].
9. ad:795 (728-733, 791-794) a narrowing store whose interval has a missing end is accepted [088].
10. ad:6463-6500 returns without a diagnostic after queuing an obligation; accepted unless the consumer enforces it; consumers outside the file [088].
11. ad:4975-4983 no MIN/-1 check for a signed type outside I8-I64 [088]. My check: `PrimitiveType` has no other signed integer (see Disagreements).
12. ad:5246 `signed_pair_excluded: true` hard-coded for proof integers (inferred: unbounded) [088].
13. ad:10885-10938 fixed point stops after `states.len()+1` rounds and uses the last environments [088].
14. ad:13395-13416 contradictory `requires` replace the environment; effect on later obligations not traced [088].
15. ad:3185-3202 imports a callee's `ensures` as caller facts; sound only if the callee body is verified elsewhere [088].
16. ad:13462, 13527-13530 `condition_belongs_to_state` true for literals and builtin type members; traced as low risk, "open beyond that" [089].
17. ad:15936 `integer_bound_terms_equal` treats any two integer bounds as equal and ignores `offset` for symbols; one outside caller, not read [089; checked].
18. ad:23738-23746 a stored integer write to a destination that does not accept integer literals is skipped [089].

Borrows
19. br:2045 a `dyn Trait` value that holds a borrow returned from a function would be classified view-free; not traced whether `dyn` can carry lifetimes [089; checked].
20. br:2877-2879, 3004-3006 a required reference with no lifetime name accepts any actual region [089].

Call cycles
21. cc:2765 `Assembly`/`AssemblyFact` statements are accepted without a rank-preservation check; whether assembly can write rank parameters is not decided in this file [090].
22. cc:2624-2626 (2719-2721) a cycle member with no source-body witness is skipped, so the component is judged only by its other members; who supplies witnesses not traced [090].
23. cc:606-636 an out-of-range index reads as "no unranked component"; reachable only with inconsistent adjacency [090].

Call validation
24. calls:2062-2069 `X.encode(..)`/`X.decode(..)` where `X` is spelled like any data definition returns with no diagnostic whether or not the method exists [090; checked].
25. calls:3057-3059 a call with a quotient operation returns before argument validation [090].
26. calls:3171-3176 build-declaration and exact-embed calls return with no argument validation [090].
27. calls:3809-3823 a type-parameter bound is not checked when the argument's type cannot be resolved [090].
28. calls:4152-4155; calls:4709-4713 arguments or const selections with no known type/binding are skipped [090].
29. calls:6059-6069 a discarded non-unit result is accepted when the call's spelling equals the name of any proof machine with no attached data, found by first name match [090; checked].
30. calls:2523, 2701-2702, 2716-2724 scan skips `SavedSubject`, type-predicate binaries and the right operand of case-membership binaries [090].
31. calls:1795 wildcard `_ => true` in `nested_boundary_applications_compose` [090].

Write frames
32. calls:10863-10868 a write-origin query with no prefix evidence is answered with no alias context; two in-range callers treat that as an effect [090].
33. calls:8844-8846 a result that aliases a shared-borrow argument is not recorded as aliasing [090].
34. calls:12128-12129 a substitution source rooted at a local is skipped [090].
35. calls:11870 a `Value` transition target contributes an empty write set [090].
36. calls:16366, 16373 a call argument that is a `Cast`, `Unary`, `Binary` or `Match` is not recorded as a written place in the syntactic fallback [091; checked].
37. calls:16158-16193 value builtins and byte predicates treated as write-free on a builtin flag [091].
38. calls:15923-15928 a statement root that is an atomic-once operation with no calls gets an empty frame; depends on whether `expression_has_calls` counts atomics [091].
39. calls:17528 a type containing `dyn Trait` is treated as holding no caller-reachable reference, so writes through it do not appear in write frames [091; checked].
40. calls:17669-17681 an alias that matches nothing is returned as if rooted at itself [091].
41. calls:19367-19376 primitive, Unit or `dyn` leaf accepted without leaf origins [091].
42. calls:22354-22367 assignment to an exclusive-reference parameter is treated as writing through, not rebinding [091].
43. calls:26047-26056 a coarse origin with an invalid root passes through the call boundary without binding to an argument [091].
44. calls:26161-26186 a local that shares a parameter's name is classified by the parameter branch first; shadowing rules not traced [091].
45. calls:22409-22418 (26827-26841) a template-dependent prefix could reach `expression_sources_before_statement` consumers [091].
46. calls:28393-28399 a may-select slot with no matching leaf is treated as unchanged; open on whether the leaf could be missing for another reason [091].
47. calls:29624-29630, 29833, 30387-30389 sources passed through or returned empty without checks [092].
48. calls:31632-31635, 31666, 31679 effects of statically unreachable code are dropped [092].
49. calls:33194-33203, 33266, 33286-33293 "no reference-slot effects" for asm intrinsics, `IntegerEmbed`, `call_preserves_reference_slots`, static call intrinsics; no comment [092].
50. calls:38739, 38743, 38770, 38784, 38787 `bind_formal_type` returns `true` on shape mismatch; a mismatch that still binds every parameter elsewhere is accepted [092].
51. calls:38952-38957 a call result is admitted in `CallArgument`/`IndexOperand` position without looking at the return type [092].
52. calls:39238, 39250, 39259 empty write set for descriptor Load, activation accessor, access borrow, flag-retaining asm [092].

Completed storage and content
53. cs:1097-1099 (1013-1014) immutable-image rows contribute no backing places [092].
54. cs:1630-1634 falls back to the unmapped segment when no unique mapping exists [092].
55. cs:2244-2254 a view with an empty path resolves to the capture itself when nothing else resolves [092].
56. cs:5468 a contract fact whose root has the same name as the parameter but a different symbol would match [092].
57. cs:6880 with no source metadata every symbol counts as toolchain-owned; reachable by synthetic or test programs, not traced [092].

Contract entailment
58. ce:5365 `selected_binary_meaning` returns true for `And`, `Or`, `CaseMembership` with no overload check [093].
59. ce:7083-7123 `+ - *` folded into polynomial arithmetic when the outside predicate `unselected_or_builtin_arithmetic` holds [093]. My check: that predicate is `all()` over authored selection occurrences, so it is true when there are none (ce:29175-29187), as [094] also says.
60. ce:7331-7340 (7393-7408) `value_preserving_integer_cast` does not check that the target is at least as wide [093].
61. ce:8120-8126, 8774-8780 `static_type_requirement_entailed` accepts without a value judgment; function outside the unit [093].
62. ce:12080-12097 a struct literal with a matching `source_constructor` record gets `Proven` for all its predicates without judging [093].
63. ce:12610 `data_fact_holds_at_zero` accepts a data fact the judge did not prove [093].
64. ce:15229-15239 a machine realizing a requirement whose `ensures` is an equation of two structural terms that is not actually satisfied is accepted through `owes_through_edge`, `boundary_qualification_authorization` or `law_checked` without `entailed`; callers of `check_law_conformance` not traced [093].
65. ce:16029-16086 an `ensures` fact that names proof-only data through a path whose first segment is not the data name is not fenced [093].
66. ce:18278, 19153 law checks return silently for a machine with no states or no entry state [094].
67. ce:21185, 21188, 21206, 21217, 21257-21281, 21283, 21286 a lent scalar whose place cannot be resolved to a restricted-record field is never checked on handback [094].
68. ce:21947-21966 `CarrierFit::retain` returns true for `Pending` and records an obligation checked later by code the reader did not read [094]. My check: `Pending` is built only at ce:4961.
69. ce:23093, 23103 (with 22879-22887) `range_complete` may stay true after an `Unknown` entry; inferred, no concrete program [094].
70. ce:23537 with no source metadata any type named `Nat` with the Zero/Succ shape is the proof `Nat` [094].
71. ce:24048 a different callable with the same last-segment name as the quotient relation, used in a `requires` fact, matches [094].
72. ce:29298-29300 `Modulo` on a carrier-free operand is accepted with no engine binding [094].
73. ce:32895-32909 a `Distance` measure with no range is a member without proof [094].
74. ce:34118-34126, 34129, 34152-34176, 35086-35098 a callee `requires` that is structurally false but matches a skip rule is not reported by this diagnostic (which only reports `Refuted`) [094].
75. Unsatisfiable hypotheses prove the goal, no comment at any site: ce:23648, 23664, 25753, 25775, 25951-25953, 28498, 29954, 31200-31204, 33443, 33480, 33557, 33587, 33631, 33637-33644 [094]; ce:37554, 37706, 43707 [095]; `is_contradictory` ce:53127 "open for the global effect" [095]. A wrong program passes only if the hypotheses are wrongly judged unsatisfiable. 39 occurrences of `requires_unsatisfiable` in the file (grep count, [094]).
76. ce:40706-40708, 41730-41732 a `DynamicSlot` whose declared and instantiated parameter counts differ skips the argument type check and the `requires` proof [095].
77. ce:41529-41533, 41537-41540, 41659 type-predicate binaries, operators with no spelling, and every other node kind return with no check [095].
78. ce:41770-41784 a `Proposition` requirement tied to named evidence is skipped without proof here [095].
79. ce:46198 a crash transition produces no leaf arm, so a crashing branch is not required to carry the theorem's conclusion; callers not traced [095; checked].
80. ce:54329, 54342, 54377, 54419 `Some(true)` shortcuts in `prepared_predicate_proofs` not verified by the reader [095].
81. ce:56590 an unobserved invocation argument becomes an `Opaque` placeholder rather than failing; inferred it cannot satisfy a predicate [096].
82. ce:58147, 58149 polynomial atoms keyed on `format!("{term:?}")`; equal debug text means same atom [096].
83. ce:60372-60402 two applications with the same target and argument count and matching result types are the same denotation; arguments are not compared there [096].
84. ce:63233-63239 an unobserved value is returned unchanged when `proof_value_is_caller_isolated(declared)`; no comment [096].

### Classed "stated" (a comment or the structure at the site gives the reason)

- rs:398-406 no activation and no requests returns an empty list [088].
- ab:3324-3347 `&write` passed where `&write` of the same referee is required; ab:3489-3490 `.len` on a collection measure [088].
- ad:8039-8060 no trap coverage needed when the machine publishes no crash ceiling; error text at ad:8139 names it [088]. ad:2452 deferred observation, by name [088].
- cc:2924-2943 a tail call to a target outside the program is accepted on an unconditional `Terminates` contract; else "no unconditional public termination contract" (cc:2940) [090].
- calls:3044-3056 "Its complete dependent application was checked by the mathematical producer; relevance still visits every operand" [090].
- calls:8180-8192 "Only writable carried borrows can reach caller storage from an owned value." [090].
- calls:11875-11886 active target with `require_complete` false returns an empty write list (by flag name) [090].
- calls:15676-15681 "Unrelated incomplete origins must not turn that proved empty set opaque." [091].
- calls:19484 a callee local root writes nothing visible; calls:23931-23975 `BorrowBytes` activation gives a fixed `Mutable` origin (by contract name only); calls:28566 identity effect skipped [091].
- calls:41516-41518 no receiver or self receiver can dispatch to the machine [092].
- br:5298 `Excluded => Ok(())`, backed by coverage rules at br:5681, 5886 [089].
- ce:9510-9523 "Soundness: the callee's ensures is machine-checked in this same validation batch" (citations with no `requires`, ce:9880-9888); the claim is not verified at the site [093].
- ce:17002-17016 "no Boolean call supplies value identity" [093].
- ce:18763-18765 "an unbound slot only matters if a law mentions it"; ce:19121-19122, 19241 result-mentioning conjuncts are "functional specs, not laws" [094].
- ce:41492-41497 "Mathematical definitions own checked dependent applications, not machine contract telescopes." [095].
- ce:43217-43223 zero generic-tuple requirements gives `Proven` (by structure) [095].
- ce:55896-55898 unclassified head matches everything; the full matcher still decides [096].

### Classed "traced" (the reader followed callers)

- br:4427-4437 an `OwningScope` claim whose requester identity resolves to no symbol is accepted on replay, and construction never checks `OwningScope` (br:3938). Traced within the file as reachable; how a requester identity is produced upstream was not read [089; checked].
- ce:31921 `measure_constraints_cover_subject` returns true because its `constraints` argument is always empty (ce:32015, 31820, 31737), so the `ParameterForward` bound check has no effect [094; checked].
- ce:69315-69324 with 71081-71086: when a callee `requires` is not proven, a callee whose every exit is one Boolean constant still contributes `result == literal`; one exit shape counted as `true` is a parameter projection that is also one of the callee's own `requires`. The record first classed this open, then traced in part; whether the caller separately discharges the `requires` was not traced [096; checked].
- calls:22187 a shared reference whose type is judged unable to carry writes gets a name-only coarse origin and is accepted as a known supplier at calls:25629-25633 and 27294-27305 [091].
- calls:19441-19445 a by-value argument of caller-isolated type writes nothing; "the weak point is the isolation verdict" [091].
- Traced as sound or unreachable: rs:71-73; ad:13613, 13625, 14942, 14996, 18037, 20518, 21002-21004, 22045, 22060, 23738-23745(b); br:2256-2261, 3699-3703; cc:974-996, 2012-2018, 2956-2960, 2983-2991, 3780-3799; calls:5503-5505, 5532-5534, 9134-9135, 9221, 11236-11242 (predicate only), 15541, 20369, 24159, 24186-24188, 32353, 32405, 33311, 33432, 34964, 35786-35791, 35931-35932, 40521-40619 (consumers not traced); cs:434-436 (callers not all traced); ce:3905-3915, 5365 (bitwise and shift only), 8747-8766, 10782-10787, 14629-14632, 15732-15738, 15861, 15929 (inferred), 16365-16367, 16476-16477, 17529, 18128, 38847-38855, 43184-43206, 44843-44850, 45467-45472, 46656, 46666, 51410-51417, 51991, 52077, 54304, 56542-56551, 66293-66295, 72609-72616 (in part) [088-096].

## Silent drops

Places where something is not planned, resolved or checked and no row and no diagnostic results. 43 entries; they are in the validation crate. Several also appear above.

1. ab:32-36, 44-46, 47-53, 54-58, 59-70, 100-102 conformance skipped by `continue` [088].
2. ab:1412, 1415-1420 no plan found, no atomic-permission check [088].
3. ab:1444-1463 no binding-private check [088].
4. ab:2993-2996 out-of-range range store [088].
5. ab:3365, 3368-3376, 3406-3408, 3416-3418 opaque-call input check returns [088].
6. ad:74-94 abstract shift-count check skipped when the width is unknown or there is no direct abstract operand [088].
7. ad:838-864 diagnostics discarded [088].
8. ad:6463-6483, 6485-6500 obligation or pending formation queued, no diagnostic now [088].
9. ad:7430-7440 exact-cast fit returns false with no error [088].
10. ad:11158 (11087-11090) an analysis that produced any diagnostic is discarded and replaced by "unbounded" [088].
11. ad:11247-11250 parameter/argument count mismatch joins an empty environment [088].
12. ad:10885-10938 round budget exhausted, no error [088].
13. ad:18438 a relation with an unobservable side is skipped [089].
14. ad:23738-23746 no stored-integer write rows [089].
15. ad:25073-25081 erasure-formation check returns for non-direct operands [089].
16. cc:2624-2626, 2719-2721 member without witness skipped; result `Ok(None)` [090].
17. cc:1948, 2668-2671 no natural component becomes an empty component [090].
18. calls:2062-2069 [090].
19. calls:3057-3059 [090].
20. calls:3171-3176 [090].
21. calls:3809-3823 [090].
22. calls:4152-4155 [090].
23. calls:4709-4713 [090].
24. calls:2523, 2701-2702, 2716-2724 [090].
25. calls:26682-26707 an assignment that exposes a frozen binding is absent from the roster (inferred fail-closed) [091].
26. calls:31632-31635, 31666, 31679 [092].
27. cs:6418-6441 the plan builder drops an invalid projection machine with `?`; diagnostics come only from the separate validator at cs:6276 [092].
28. ce:16222, 16241-16267 the inductive judge returns with no verdict for unsupported bodies; ce:16355-16358 an `Unknown` arm is neither proven nor reported unless the machine is fully visible [093].
29. ce:18278, 19153 [094].
30. ce:18763-18765 slot with no candidate skipped [094].
31. ce:21185, 21188, 21206, 21217, 21257-21281, 21283, 21286 [094].
32. ce:29640-29647 hypotheses with an unadmitted carrier dropped; ce:30013, 30026, 30061 failures discarded with `let _ =` [094].
33. ce:34118-34126, 34129, 34152-34176, 35086-35098 [094].
34. ce:37594-37601, 37606 negated hypotheses skipped [095].
35. ce:40706-40708, 41519-41524, 41529-41533, 41537-41540, 41659, 41730-41732, 41770-41784 [095].
36. ce:43168-43173 static parameters other than `Const`/`Value` are not bound, so their requirements are not judged here [095].
37. ce:46198 [095].
38. ce:54637-54655 an unprojectable premise is skipped or marks the set incomplete [096].
39. ce:56590 unobserved argument replaced by a placeholder [096].
40. ce:58827-58835 array-element equations not recorded on a length mismatch [096].
41. ce:61468-61499 (61525-61529) call guarantees not recorded when bindings are unavailable; write effects still observed [096].
42. ce:68772, 68777 an empty returned-field template is stored and reused after a decline [096].
43. ce:71897-71903 `capture_guarantees` drops guarantees it cannot express [096].

## Unused or unwired

- `occurrence_integer_expressions_bounds` (ad:10535, `pub`): 0 call sites; only the definition, a re-export at `lib.rs:1905` and two in-file `use` lists (ad:13560, 26782) [088; checked].
- `form_opaque_representation_request` (rs:251, `pub`, re-exported rp:74): 0 external call sites, 1 internal (rs:417) [088; checked].
- `exact_access_entry_for_field` (ab:887, `pub(crate)`): 0 external, 1 internal (ab:1044). `RetainedFieldIdentity` (ab:875, `pub(crate)`): 0 uses outside the file [088; checked].
- `unit_result_initializer_call_is_admitted` (calls:1432, `pub`): 1 internal call (calls:1407), 0 external; re-exported at `lib.rs:1505`, calls:2864, 39738 [090; checked].
- `check_conformance_refines` (br:5188): every path ends in `Err`, so selecting a conformance is never accepted (br:5222-5225) [089; checked].
- `measure_constraints_cover_subject` (ce:31915): 1 call (ce:31737), always true as wired [094; checked].
- ce:44316 compares `first_condition` with itself; it says nothing about the second statement [095; checked].
- `impl SchemaQueryAuthority {}` empty (br:4933); `Operand::CompletedCall` exists only under `cfg(test)` (ad:20817) [089].
- `validate_return_value_range` has two branches that call the same function with identical arguments (ad:22547-22574; inferred) [089].
- `fact_mentions_proof_only_data`, `fact_mentions_zero_value` (ce:16017, 16141): no uses outside `ce`; called at ce:90713-90715, 90863-90864 [093].
- Documented as shared, few users: `select_slot_candidate` "under the same source family preference everywhere" (ce:18659): 2 call sites, both in this file (ce:18766, 56918) [094]. "Call ordinals and their owning target lanes come from one traversal" (calls:43407): `CallSiteTraversal` is used in `calls` only, and `expression_effects` repeats its skip rules (calls:31632, 31757) [092].
- One caller each (grep counts from the records): `structural_return_descends` (calls:5490); `ranged_member_site_invariant` (ce:29664); `call_argument_destinations`; `declaration_call_type_arguments`; `shared_proof_walks::proven` (ce:8383); `case_predicate_owner` (ce:41315); `recognize_guarded_structural_value_arms` (ce:90474); `inductive_transition_entailment` (ce:90893); `suggest_missing_citation` (ce:90825); `AppliedProofBindings::for_call(` (ce:41526); `proof_edge_strict_decrease_judged` (calls:5608); `proof_interval_strict_decrease_judged` (cc:357); `check_contract_refinement` (`traits.rs:2523`); `expression_borrows_fixed_parameter_address` (1 outside caller in `flow.rs`) [090, 091, 093, 094, 095].
- `memoized(` (calls:39526): 21 matches, all in `calls` [092]. `frame_paths_overlap`: 28 occurrences in 12 files [091]. `selected_scalar_interval` family: 74 hits in 12 files, the shared evaluator for the proof checker and lowering [089].
- Not grepped by the readers and so not claimed: `has_builtin_bound_arithmetic` (ad:13597), `IntegerSet` (ad:16481), `assignment_rebinds_parameter`, `assignment_stored_value_type`, `frame_coordinate_path`, `frame_length_coordinate_path`, `instantiate_written_path`, `state_result_origins`, `struct_literal_field_type` (calls), and the count-1 or count-2 names listed at the end of [096].

## Stated limits

- rs:219 "v1 carriers must be inert storage" [088].
- ab:2181 "the current milestone proves non-observation only for checked Omega bodies; bodyless boundary, accepted, requirement, and external-provider declarations require an admitted write-only boundary claim"; ab:3078 "range projection is not implemented"; ab:3432 "an opaque selected implementation requires an admitted write-only boundary claim" [088].
- ad:798 "decision 17 -- a narrowing store is a proof obligation"; ad:6164 "Decision 17 forbids implicit domain mixing"; ad:6720 "ch5 shift-count ruling -- proof-or-policy"; ad:4748 "has no declaration formation evidence for {obligation}"; ad:4784 "has an open construction without a declaration construction judgment" [088].
- ad:23753 "IntegerAt store in {owner} has no complete exact layout alternative roster" [089].
- br:552 "unimplemented: callable input region frontier is incomplete"; br:774 "unimplemented: callable returned-source relation is ambiguous or incomplete" [089].
- cc:3791-3794 "termination across implementation selection is not yet ranked"; cc:2952-2953 "a subordinate-state call needs its own arrival evidence" [090].
- calls:1708-1709 "a value-call argument cannot itself be a machine call yet"; calls:2013 "storage address_of for write-only references is not implemented"; calls:3233 "value-qualified machine calls are retired"; calls:5561-5562 "compound measures over proof data are not proven yet"; calls:6017-6018 "Runtime recursion is TAIL-ONLY (measured recursion amendment)"; calls:953 "tail recursion spelled as a call, which Omega does not support" [090].
- calls:13085 "No computed/indexed/cast source is reconstructed from its spelling."; calls:14048 "stored call has no complete statement-successor effects: ..." [090]. calls:19724 "physical reference view has no exact scalar storage translation" [091].
- cs:3341-3344 "never reintroduce a moved caller input or use an earlier callee entry value"; cs:5165-5167 "old/current structural places admit only literal fixed-array indices"; cs:4945 "there is no generic `content(...)` intrinsic" [092].
- ce:17986-17997 the equality/`&&` rung comment; ce:18025 "outside checked operator-contract entailment's equality/`&&` rung" [093].
- ce:14299-14301 "A callable law is established under its declared entry contract." [093].
- ce:42579-42581 "Keep that route closed until its exact source carrier needs no type or region substitution" [095].
- ce:47250-47251 the frontier cap over-refuses and is "never unsound" (as the record reports the comment) [095].
- ce:56964-56972 "An unconditional rewrite has no premises to discharge" [096].
- The records note that `ce` and `calls` carry few "unsupported" comments; most limits are `Err(&'static str)` strings or early returns ([090] cc; [092]; [093] ce:7622-7973; [094]).

## Disagreements

1. **`checked_body_alternatives` (calls:9562).** [090] found "0 call sites" apart from grep hits it did not read and labelled it suspected unused. [091] (calls:25063-25131) and [092] (calls:33410-33421) each describe a use. Code: two callers, calls:25097 and calls:33412. [091] and [092] are right.
2. **`EntryMachines::of` / `LicenseCandidates::of`.** [096] read a count of 1 as "definition only". The definition is `fn of` inside an `impl` and cannot match that search. The single hits are callers at ce:81148 and ce:81141. Both functions have one caller, in the part of `ce` outside this area.
3. **Owner of `integer_term_fits`.** [094] calls it `Engine::integer_term_fits` "outside unit". [095] lists it as a `StructuralJudge` method at ce:53414. Code: `fn integer_term_fits(&self, value: StructuralTerm, ...)` inside `structural_judgment::integer_predicates`. [095] is right.
4. **Where `CarrierFit::Pending` comes from.** [094] says its producer is outside its unit and was not found. [093] lists `integer_value_fit` (ce:4921) as returning `CarrierFit` but does not describe `Pending`. Code: the only construction is ce:4961, reached when the target is the address model, the maximum is the address-model bound, the minimum is 0 and the value is a constant that fits `u64`. Both records are consistent with this; neither had the whole picture.
5. **Where `requires_unsatisfiable` is set.** [094] and [095] say it is computed outside their units. It is set at ce:6709 (a constant lower bound that is false) and ce:6733 (a `!=` whose two sides are proven equal), inside [093]'s range; copied at ce:6277. [093] lists the field and `install_hypotheses` (ce:6664) but does not describe the setting.
6. **Signed integer types.** [088] left open whether signed types beyond I8-I64 exist (ad:4975); [095] says an atom is unsigned "not `Usize` or `Addr`" (ce:43661-43667). Code: `PrimitiveType` is Bool, F32, F64, I8, I16, I32, I64, U8, U16, U32, U64, Addr, Selector (`psi/compilation/representations/typed-trees/src/type_system.rs:4591-4605`). There is no `Usize` and no wider signed integer.
7. **Length of `calls.rs`.** [091] says "39k+ line file"; [090] and [092] say 44,215. `wc -l` gives 44,215. The `write_frames` module ends near line 39721 ([092]), which may be what [091] meant.
8. **Cyclic-component predicate count.** [090] says "Three separate" predicates and then cites six sites (cc:88-91, 302-309, 520-527, 686-690, 1092-1095, 3185). I use six.
9. **`frame_paths_overlap`.** [089] "suspected string-prefix logic, not read"; [091] read it. Code at calls:16026-16043 is a pure string test. No contradiction; the suspicion is confirmed.
10. **ce:69315-69324.** [096] classes it "open" in one section and "traced" in a later one. The code matches both descriptions; I list it under "traced" with the untraced part stated.

## What I checked myself

| # | Claim | Cite | Result |
|---|---|---|---|
| 1 | Out-of-range write-only range store returns `true` with no diagnostic | ab:2993-2996 | matches; both branches return `true`, so the `if` changes nothing |
| 2 | `check_conformance_refines` always ends in `Err` | br:5188-5226 | matches |
| 3 | `OwningScope` claim accepted when the requester resolves to no symbol | br:4427-4437 | matches |
| 4 | Guard comparison of `first_condition` with itself | ce:44316 | matches |
| 5 | `measure_constraints_cover_subject` always true as wired | ce:31915-31923, 32008-32033; grep of `ParameterForward` (ce:31733, 31820) | matches for the lines opened; I did not open ce:31812-31824 |
| 6 | `integer_bound_terms_equal`: Integer vs Integer true, Symbol ignores offset | ad:15933-15981 | matches |
| 7 | Discarded-result exemption by first machine with the same name | calls:6057-6069 | matches |
| 8 | Two methods with identical bodies | calls:14826-14836, 14861-14871 | matches |
| 9 | Syntactic fallback skips shared borrows and unlisted argument kinds | calls:16363-16374 | matches |
| 10 | `occurrence_integer_expressions_bounds` has no call sites | grep over `omega-rust`: 4 lines (def, re-export, 2 use lists) | matches |
| 11 | `unit_result_initializer_call_is_admitted` has no external caller | grep: def, calls:1407, 3 re-exports | matches |
| 12 | `exact_access_entry_for_field`, `RetainedFieldIdentity`, `form_opaque_representation_request` have no outside users | grep | matches |
| 13 | `checked_body_alternatives` suspected unused | grep: calls:25097, 33412 | does not match; two callers |
| 14 | `EntryMachines::of` count 1 is the definition | grep: ce:81148; `LicenseCandidates::of` ce:81141 | does not match; the hit is a caller |
| 15 | Write ladder exists three times | calls:15459-15478, 16275-16313, 27161-27187 | matches; gates differ |
| 16 | `dyn Trait` treated as carrying nothing | calls:17528; br:2045; calls:9220 | partly: two sites say so, calls:9220 returns `Some(true)` (may hold) |
| 17 | `CarrierFit::Pending` producer unknown | grep and ce:4921-4966 | resolved: only ce:4961 |
| 18 | `unselected_or_builtin_arithmetic` true with no occurrences | ce:29167-29188 | matches |
| 19 | Crash transitions produce no leaf arm | ce:46198-46200 | matches |
| 20 | Unproven `requires` falls back to `recorded_body_boolean` | ce:69315-69324 | matches |
| 21 | `from_comparison` and `comparison_interval` are the same job | ad:1132-1152, 13781-13800 | matches |
| 22 | `copy_retained` overlaps with environment rebinding | ad:4655-4677, 25840-25853, 26062-26130 | matches; same string re-keying |
| 23 | `frame_paths_overlap` is string logic | calls:16026-16043 | matches |
| 24 | Other signed primitives may exist; `Usize` | type_system.rs:4591-4605 | partly: no such variants exist |
| 25 | `integer_term_fits` is an `Engine` method | ce:53414 | does not match; it is on the structural judge |
| 26 | `encode`/`decode` on a data-named receiver returns silently | calls:2062-2069 | matches |
| 27 | State name `contains("const")` | ce:34157 | matches |
| 28 | Where `requires_unsatisfiable` is set | grep `requires_unsatisfiable = `: ce:6277, 6709, 6733 | found; fills a gap in [094], [095] |
| 29 | File lengths | `wc -l` | calls 44,215; ce 91,057; ad 26,886 |
| 30 | `selection_for_opaque` has 3 external call sites | grep | matches |
| 31 | `OpaqueRepresentation` recognised by name, shape, Toolchain origin and path suffix | rp:35-69 | matches |
| 32 | Commitment rederivation mirrored in provider-planning | sel:139-146; `opaque_representations.rs:71-78` | matches; both call the same shared function |

I did not build, run or edit anything.

## Not covered

- **Records:** all nine were read in full. There are no `unit_NNN_gap_*.md` files for units 088-096.
- **Source ranges inside the units:** every record reports its whole range read, none skimmed. [094] notes it followed the mechanical arithmetic code (ce:25445-26656, 27188-28877, 33077-33698) for structure, not re-derived. [096] read test modules at lower attention (ce:55051-55143, 56313-56344, 58158-58211, 58399-58810, 63147-63212, 67399-68605).
- **Outside this area but needed to judge its findings:** `ce` 72849-91057 (unit 097), which holds the `StructuralJudge` and `StructuralTerm` definitions, `StructuralJudgment::connective`, `open_closed_type_atom` (ce:89429), and the callers at ce:90xxx; the other 22 files of the validation crate (147,446 of its 331,160 lines), including `lib.rs`; the stage-05 crate that supplies ranking witnesses and consumes `ArithmeticDecisions`; `static_type_requirement_entailed`, `data_fact_holds_at_zero`, `call_preserves_reference_slots`, `primitive_type_reference`, `proof_value_is_caller_isolated`.
- **Reachability:** for the 84 "open" entries no reader traced whether a wrong program can reach the site, except where an entry says otherwise. I did not trace reachability either; my checks confirm what the cited lines do.
- **Counts:** the spelling-site and silent-drop counts are of locations the records cite. A record that summarised a family in one line contributes one location.

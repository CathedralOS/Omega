# Spec requirements and evidence coverage map

Snapshot: `9a0775cf7b893963d4d1441b8135c966315301d6` · 2026-10-18 UTC.

For future assignments, use the [agent handoff guide](spec_coverage_handoff.md)
and [saved inventory validator](spec_coverage_map_validate.py).

The full current spec is inventoried below: **98 documents, 744 headings, and 1,024 overlapping requirement families**. This is the expected requirement side of the coverage map, with existing fact rows, other evidence candidates, and remaining qualification needs. A green sample queue cannot establish that these requirements are all checked.

The families are review-sized groups, not a count of independent axioms or every normative sentence. Source pointers are at document granularity; the accompanying [structured map](spec_coverage_map.json) records every heading and line, source hash, family ID, file candidate, and uncertainty. All documents were reviewed; long encoding/acceptance tables remain governed by their source rather than copied into a second specification. Drafts/proposals are excluded. This audit changes no accepted rule or task priority.

## Reading the evidence

**Located** means the file or area exists. **Source-inspected** means the stated contract/oracle was inspected. **Qualified** additionally requires a recorded passing command, original source and compiler/library identity, target/engine, meaningful oracle, and intended rejection. There are **zero fresh execution qualifications in this audit**. README run claims and runtime headers do not cross that boundary. Area links below are search starting points, never evidence for every family in their chapter.

Each concrete fact needs establishment, identity/lifetime, composition, and soundness evidence. Nine places qualify transport of the same small fact; they do not replace all-predecessor, mutation, proof-denotation, cleanup, artifact, or physical-consumer checks. Kernel metatheory needs its stated mathematical evidence; native ABI, installation and device promises need their actual consumers.

## Scope census

| Area | Documents | Headings | Requirement families |
| --- | ---: | ---: | ---: |
| [language](#area-language) | 22 | 180 | 272 |
| [proofs](#area-proofs) | 7 | 67 | 84 |
| [resources](#area-resources) | 13 | 69 | 110 |
| [layouts](#area-layouts) | 3 | 18 | 21 |
| [terminal-psi](#area-terminal-psi) | 15 | 133 | 181 |
| [build](#area-build) | 30 | 214 | 276 |
| [packages](#area-packages) | 8 | 63 | 80 |

These counts measure reviewed scope, not implementation completeness. Families overlap across layers, and no percentage of tested spec coverage follows from them.

## Existing fact rows

At this revision there are 26 fact folders: **25 have nine positive source places**, **24 have a located control candidate for each place**, and **17 have runtime oracle headers at all nine places**. Eight rows explicitly remain check-only. The `repro` folder has no nine-place row. Controls can live under a place, under the row’s `controls`, or in the regression corpus; the structured map resolves all three. None of these counts asserts a current passing row or a meaningful single-fact difference.

| Fact | Positive places | Places with control candidates | Places with runtime headers | Intended narrow fact |
| --- | ---: | ---: | ---: | --- |
| [alias_fact_transfer](../../../samples/cli/facts/alias_fact_transfer) | 9 | 9 | 0 | Alive membership follows a bare alias into an inspect precondition. |
| [as_slice_views](../../../samples/cli/facts/as_slice_views) | 9 | 9 | 9 | A shared whole-array view of four noncopy elements retains extent four. |
| [bounded_products](../../../samples/cli/facts/bounded_products) | 9 | 9 | 9 | Two u64 operands in 0..=50 have an exact product in 0..=2500. |
| [bounded_scalar_sums](../../../samples/cli/facts/bounded_scalar_sums) | 9 | 9 | 0 | Two bounded u64 operands admit their exact sum and its receiving bound. |
| [call_result_bound_discharge](../../../samples/cli/facts/call_result_bound_discharge) | 9 | 9 | 0 | A closed call result of four discharges a later scale <= 5 precondition. |
| [case_where_equality](../../../samples/cli/facts/case_where_equality) | 9 | 9 | 0 | Case field equality justifies exact subtraction without a separate guard. |
| [compound_index_guard](../../../samples/cli/facts/compound_index_guard) | 9 | 9 | 0 | A guard on b * 20 + a proves that same two-variable grid index safe. |
| [computed_record_window](../../../samples/cli/facts/computed_record_window) | 9 | 9 | 9 | A computed matched length-two record window preserves writes to original f64 backing. |
| [data_where_standing](../../../samples/cli/facts/data_where_standing) | 9 | 9 | 9 | A standing field bound justifies exact arithmetic after field observation. |
| [dependent_subtract](../../../samples/cli/facts/dependent_subtract) | 9 | 9 | 0 | used <= count with count <= 11 justifies exact subtraction and narrowing. |
| [disjoint_windows](../../../samples/cli/facts/disjoint_windows) | 9 | 9 | 9 | Checked disjoint windows retain independent exclusive access to original storage. |
| [exact_sub_guard](../../../samples/cli/facts/exact_sub_guard) | 9 | 9 | 0 | remaining >= 2 justifies exact remaining - 2 under the original upper bound. |
| [field_where_index](../../../samples/cli/facts/field_where_index) | 9 | 9 | 0 | Standing k <= 3 proves arr[k + 1] within declared extent five. |
| [float_range_delivery](../../../samples/cli/facts/float_range_delivery) | 9 | 9 | 9 | Inclusive same-format f64 range 0..=0.25 discharges wider 0..=0.5. |
| [guarded_line_loan](../../../samples/cli/facts/guarded_line_loan) | 9 | 9 | 9 | A guarded line count/extent reaches the bounded input loan operation. |
| [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan) | 9 | 9 | 9 | A guarded record line loan retains the returned count and caller field writeback. |
| [index_every_width](../../../samples/cli/facts/index_every_width) | 9 | 9 | 9 | Every fixed integer width with 0 <= index <= 3 indexes four elements. |
| [indexed_element_mut_loan](../../../samples/cli/facts/indexed_element_mut_loan) | 9 | 9 | 9 | A checked element index lends original record storage for mutation. |
| [length](../../../samples/cli/facts/length) | 9 | 0 | 9 | A nonempty runtime subslice length names its last byte. |
| [matched_field_windows](../../../samples/cli/facts/matched_field_windows) | 9 | 9 | 9 | A matched length-two window lends original backing for write_pair. |
| [range_fact_delivery](../../../samples/cli/facts/range_fact_delivery) | 9 | 9 | 9 | index < 4 entails index <= 4 at the count precondition. |
| [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff) | 9 | 9 | 9 | Moving an exclusive descriptor preserves its original writable referent. |
| [remainder_width_index](../../../samples/cli/facts/remainder_width_index) | 9 | 9 | 9 | A positive runtime divisor equal to view length bounds its remainder index. |
| [repro](../../../samples/cli/facts/repro) | 0 | 0 | 0 | Scratch float reproduction; no nine-place requirement row established. |
| [returned_write_destination](../../../samples/cli/facts/returned_write_destination) | 9 | 9 | 9 | A returned write-only loan writes caller storage while preserving its neighbor. |
| [runtime_count_index](../../../samples/cli/facts/runtime_count_index) | 9 | 9 | 9 | A runtime generic count indexes the exact byte view under its declared extent. |

`length` has no attributed per-place rejecting twins in this inventory. `bounded_products` and `bounded_scalar_sums` still use scalar-range suffixes in their nine-place sources, despite the current proposition-based contract. `field_where_index` is check-only in this snapshot. Those distinctions prevent historical or structural evidence from silently becoming current canonical native qualification.

Outside the fact tree, the corpus contains **3,706 pass**, **2,996 fail**, **164 run**, and **7 pending** `main.omg` roots. These are file counts, not results; pass roots can include executable canaries. Unit/codec/package tests are additional candidate evidence. Their existence is why “no dedicated fact row” below does not mean “no tests anywhere.”

## Specific evidence inspected

- **effects-block**: Published blocking ceiling still requires acknowledgement for a selected nonblocking implementation. Sources: [tests/omega/pass/effects/finite_requirement_block_acknowledged_exit/main.omg](../../../tests/omega/pass/effects/finite_requirement_block_acknowledged_exit/main.omg), [tests/omega/fail/effects/finite_requirement_block_required/main.omg](../../../tests/omega/fail/effects/finite_requirement_block_required/main.omg), [tests/omega/fail/effects/finite_requirement_block_required/expected.txt](../../../tests/omega/fail/effects/finite_requirement_block_required/expected.txt). Sources differ in additional declarations; not certified as a single-change twin.
- **recast-implication**: Shared same-format record views may weaken, but cannot strengthen, predicates. Sources: [tests/omega/pass/recast/runtime_shared_record_float_range_weakening_exit/main.omg](../../../tests/omega/pass/recast/runtime_shared_record_float_range_weakening_exit/main.omg), [tests/omega/fail/recast/recast_shared_record_where_strengthening_rejected/main.omg](../../../tests/omega/fail/recast/recast_shared_record_where_strengthening_rejected/main.omg), [tests/omega/fail/recast/recast_shared_record_where_strengthening_rejected/expected.txt](../../../tests/omega/fail/recast/recast_shared_record_where_strengthening_rejected/expected.txt). Source-inspected implication examples; no fresh run or mutable bidirectional qualification.
- **wire-canonical-varint**: Overlong and overflowing varints reject while u64::MAX canonical encoding remains valid. Sources: [tests/omega/pass/wire/runtime_wire_decode_rejects_noncanonical_varint_exit/main.omg](../../../tests/omega/pass/wire/runtime_wire_decode_rejects_noncanonical_varint_exit/main.omg). Authored source observations, not universal codec laws or current executed result.
- **build-seal**: Required output completion needs its actual sealed current staged file; unsealed failure stays unsuccessful. Sources: [tests/omega/pass/build/artifact_only_required_output_completion/build.omg](../../../tests/omega/pass/build/artifact_only_required_output_completion/build.omg), [tests/omega/fail/build/artifact_only_unsealed_output_refused/build.omg](../../../tests/omega/fail/build/artifact_only_unsealed_output_refused/build.omg), [tests/omega/fail/build/artifact_only_unsealed_output_refused/expected.txt](../../../tests/omega/fail/build/artifact_only_unsealed_output_refused/expected.txt). Build sources inspected; full publication/authority/platform qualification not executed.
- **bad-arrival**: One unbounded predecessor cannot donate another arrival's facts to exact addition. Sources: [tests/omega/fail/arithmetic/composite_bound_arrival_unbounded_predecessor/main.omg](../../../tests/omega/fail/arithmetic/composite_bound_arrival_unbounded_predecessor/main.omg), [tests/omega/fail/arithmetic/composite_bound_arrival_unbounded_predecessor/expected.txt](../../../tests/omega/fail/arithmetic/composite_bound_arrival_unbounded_predecessor/expected.txt). Negative source only; neither a full nine-place row nor a fresh rejection result.
- **strict-core**: Empty elimination checks target/scrutinee sorts and rejects foreign relevant values. Sources: [omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/strict_layer.rs](../../../omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/strict_layer.rs). Inspected first test; file presence/test assertions are not a fresh result or metatheory proof.
- **family-codec**: Named roundtrip and malformed-family tests locate encoding candidates. Sources: [omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/semantic_module/target_family_tests.rs](../../../omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/semantic_module/target_family_tests.rs). Test signatures located; complete body and execution not qualified.

All seven entries remain unexecuted candidates at this revision. They are individually attributed only where a family ID is explicitly listed in the structured map.

## Reconciliation notes

- **canonical-scalar-ranges (witness/spec mismatch)**: Both nine-place source rows contain bracketed scalar-range suffixes; current numeric contract requires ordinary propositions. Runtime headers and historical compatibility cannot certify canonical syntax acceptance. Sources: [wiki/spec/language/numeric_values.md](../../spec/language/numeric_values.md) line 1, [samples/cli/facts/bounded_products/parameter/main.omg](../../../samples/cli/facts/bounded_products/parameter/main.omg) line 1, [samples/cli/facts/bounded_scalar_sums/parameter/main.omg](../../../samples/cli/facts/bounded_scalar_sums/parameter/main.omg) line 1.
- **unlocked-native (contract wording conflict)**: Review says missing accepted state stops native production; acceptance/locks explicitly permit absent-lock development native compilation with no invented grants. Reconcile before using the conflicting sentence as a new requirement. Sources: [wiki/spec/packages/review.md](../../spec/packages/review.md) line 21, [wiki/spec/packages/acceptance.md](../../spec/packages/acceptance.md) line 62, [wiki/spec/packages/locks.md](../../spec/packages/locks.md) line 128.
- **vocabulary-marker (encoding wording conflict)**: Module layout names vocabulary marker 123 while section identities say shared marker 122. Source and codec must be reconciled before deriving exact marker vectors from prose. Sources: [wiki/spec/terminal-psi/encoding.md](../../spec/terminal-psi/encoding.md) line 618, [wiki/spec/terminal-psi/encoding.md](../../spec/terminal-psi/encoding.md) line 1690.
- **length-twins (unattributed evidence)**: length has nine positive sources but no per-place twins in its sample tree or matching fail/facts naming inventory. Other slice failures are not attributed as its nine rejecting twins. The older filled-row statement is not qualification evidence. Sources: [AGENTS.md](../../../AGENTS.md) line 101, [samples/cli/facts/length/local/main.omg](../../../samples/cli/facts/length/local/main.omg) line 1.
- **status-not-a-result (implementation status uncertainty)**: Observer source-to-Terminal status and other process-exit status narratives are not fresh compatible execution results. Keep normative trace requirements separate from those status claims. Sources: [wiki/spec/terminal-psi/observations.md](../../spec/terminal-psi/observations.md) line 152, [wiki/spec/language/process_exit.md](../../spec/language/process_exit.md) line 111.
- **incomplete-formats (explicit specification boundary)**: Both documents explicitly distinguish required semantics from incomplete physical formats. Unassigned fields cannot become invented compatibility requirements; no complete-format implementation claim follows. Sources: [wiki/spec/build/compiler_request.md](../../spec/build/compiler_request.md) line 1, [wiki/spec/build/executable_installation.md](../../spec/build/executable_installation.md) line 1.

These are audit findings, not language decisions. Conflicting prose and stale implementation status must not be turned into new compiler requirements by inference.

## Next small qualification slices

These are bounded ways to start closing the map, in order. First inspect/reuse the named existing source; write a new fact only if the exact requirement is still missing. A currently passing case needs no invented compiler repair. Existing task priorities remain authoritative.

1. Qualify exact bounded sum/product with canonical requires/ensures syntax and preserve original operands, bounds and operation. Requirements: `language-numeric-values-001`, `language-numeric-values-006`. Negative: Remove one necessary bound; rejection must identify arithmetic or receiving-range failure, not obsolete syntax. Oracle: Check interior and inclusive endpoint payloads in native and Terminal, with a deliberately wrong expected result trapping.
2. Qualify one same-subject bound that must hold on every join arrival. Requirements: `terminal-psi-fact-validity-002`, `language-state-contracts-004`. Negative: One predecessor lacks the bound; retain the same downstream exact operation. Oracle: Execute both safe alternatives, preserve the partial operation, and match the negative diagnostic to that operation.
3. Qualify one saved scalar snapshot while an alias changes current storage. Requirements: `language-dependent-values-015`, `language-state-contracts-007`. Negative: Use the new mutable read with the old bound instead of the saved SSA value. Oracle: Distinct before/after values show snapshot validity and stale-storage rejection without folding the exercised operation away.
4. Qualify shared predicate weakening and exclusive bidirectional representation compatibility. Requirements: `layouts-recasts-002`, `layouts-recasts-003`. Negative: Strengthen target predicate or attempt a one-way mutable recast. Oracle: Read original payload and, for admitted mutable form, observe writes through original caller storage.
5. Qualify one mixed signed-index/unsigned-length comparison without losing the signed lower-bound obligation. Requirements: `terminal-psi-integer-certificates-001`, `language-numeric-values-008`, `language-numeric-values-019`. Negative: Remove only nonnegativity while retaining the mixed upper comparison. Oracle: Distinct signed/unsigned endpoint payloads; current candidate is reused only after exact source/control review.
6. Qualify the existing required-output sealed/unsealed source pair at its actual Build consumer. Requirements: `build-scoped-execution-013`, `build-scoped-execution-014`. Negative: Omit writer sealing and verify no new successful result set is published. Oracle: Inspect exact committed manifest/content and rejected-output state; this is not a nine-place transport row.

After each slice, record only the exact family obligations its evidence establishes. Keep other families and all unsupported targets/consumers open. For source transport, keep the same bound and operation across the nine places. For wire, kernel, Build and installation requirements, use the appropriate consumer rather than forcing every case into a source-place row.

## Earlier isolated execution trial

The separate `field_where_index` Sol medium trial qualified nine native and nine Terminal positives plus nine intended rejections on Linux x86-64, with independent parent replay. It used source `3918e1378f52828e813cc2f41957d0e9a8e68293`, producer `07af46d6c250f9d9dff99af4b724f8ca30bd4b3a`, and binary SHA-256 `6e55ead04a5afa1887f94d9267662904aee5cbaeed44026d1c1d6d6cc6af2054`. It added observable initialized-payload oracles and required no compiler repair. Those exact results remain valid for that trial; they are excluded from current snapshot coverage until applicability is established.

## Complete document map

Every family below still needs exact execution/proof/consumer qualification. **Source** names its normative document; **candidate areas** only locate related existing material. **Remaining** describes the boundary not established by that material’s presence. The JSON section index accounts for all 744 headings; it does not claim each heading is independently discharged.

<a id="area-language"></a>

### language

<a id="language-assembly"></a>

#### assembly

**Source:** [wiki/spec/language/assembly.md](../../spec/language/assembly.md) · 7 headings · SHA-256 `bb3049067240176bcc2d126947957740ac751bea1421db5c464b0506669e2c02`.

**Evidence needed:** source assembly facts and target/environment integration.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/asm (19 roots)](../../../tests/omega/pass/asm), [fail/asm (55 roots)](../../../tests/omega/fail/asm), [pass/inline_asm (24 roots)](../../../tests/omega/pass/inline_asm), [fail/inline_asm (24 roots)](../../../tests/omega/fail/inline_asm).

**Required families:**

- `language-assembly-001` — Only known catalog instructions form checked assembly; raw bytes and unmodeled exits reject.
- `language-assembly-002` — Every label incoming edge proves exact requires and every outgoing path proves ensures, including fallthrough and ranked backedges.
- `language-assembly-003` — Assembly entry requirements are caller goals and memory propositions grant no runtime access.
- `language-assembly-004` — Declared clobbers exactly equal catalog changes, and separate bodies share no implicit scratch inputs.
- `language-assembly-005` — Instruction semantics retain exact operands, features, memory/authority, ordering, regimes and all control exits.
- `language-assembly-006` — A linear consuming effect is proved on every normal assembly path; nop, wrong place, missing ordering and bypasses leave custody live.
- `language-assembly-007` — Interpreted operations execute the same catalog semantics under an explicit admitted model without acquiring actual host hardware authority.
- `language-assembly-008` — Persistent state, scratch and completed effects survive interpreter pauses without replay, fabricated success or native fallback.
- `language-assembly-009` — The full selected executable closure has demanded semantics/environment coverage before execution and dynamic installation.
- `language-assembly-010` — Control/return state remains compiler-owned and numeric addresses cannot replace compatible sealed entries.
- `language-assembly-011` — Regime and floating-control changes prove transitions/restoration against the final artifact and selected plan.
- `language-assembly-012` — Hardware loads/stores and executable publication retain separate extent, mapping, instruction-fetch and installation obligations.

**Remaining:** No existing fact folder establishes assembly label state/edge contracts, exact effect-based linear retirement or interpreted environment coverage. Dedicated checked assembly and matching-target tests are required.

<a id="language-concurrency"></a>

#### concurrency

**Source:** [wiki/spec/language/concurrency.md](../../spec/language/concurrency.md) · 5 headings · SHA-256 `d6debda68039239e943bc59c02bff40b7f22d1b25530630f72f7dda9714c2cea`.

**Evidence needed:** atomic facts, protocol proofs and matching-host evidence.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/concurrency (13 roots)](../../../tests/omega/pass/concurrency), [fail/concurrency (7 roots)](../../../tests/omega/fail/concurrency), [pass/atomics (21 roots)](../../../tests/omega/pass/atomics), [fail/atomics (22 roots)](../../../tests/omega/fail/atomics).

**Required families:**

- `language-concurrency-001` — Activating ordinary machines preserves separate copyability, transfer, shared-access and carry contracts.
- `language-concurrency-002` — Atomic operations require exact admitted width/alignment/encoding laws and cannot be synthesized from unrelated reads/writes.
- `language-concurrency-003` — Swap/fetch return the instruction-observed prior value, and zero/default outcome construction proves no event.
- `language-concurrency-004` — Compare-exchange uses exact representation/key laws, retains custody and distinguishes decisive mismatch from uncommitted single attempts.
- `language-concurrency-005` — Orderings are proof-static and operation-specific; failure ordering is read-only and cannot exceed success.
- `language-concurrency-006` — Receive keeps the required consistency baseline unless a complete weaker-target protocol refinement is proved.
- `language-concurrency-007` — Fences synchronize only through qualifying reads-from relationships and do not publish memory by themselves.
- `language-concurrency-008` — Interruption, device/DMA and coherent cross-core participants retain distinct ordering and installed-root contracts.
- `language-concurrency-009` — Source event relations and exact target refinements remain separate from instruction selection and incomplete metatheory.
- `language-concurrency-010` — Protocol topology accounts for waits, ownership, releases, wakeups, close/cancel paths and external admitted waits.
- `language-concurrency-011` — Fairness, arrival-rate and multicore response guarantees require exact provider/rate-limit/resource-sharing premises.
- `language-concurrency-012` — Bounded exploration remains a test and does not prove unbounded dynamic-topology correctness.

**Remaining:** None of the existing scalar bound/loan rows establishes atomic event semantics or protocol composition. Separate source/placed-atomic controls, protocol proofs and target execution are required.

<a id="language-conformances"></a>

#### conformances

**Source:** [wiki/spec/language/conformances.md](../../spec/language/conformances.md) · 7 headings · SHA-256 `ad05016888741faf8ab795246b210ac4202351935fc63bccee27b0a4f11c1f18`.

**Evidence needed:** trait and source identity fixtures.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/traits (92 roots)](../../../tests/omega/pass/traits), [fail/traits (117 roots)](../../../tests/omega/fail/traits), [run/traits (3 roots)](../../../tests/omega/run/traits), [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers).

**Required families:**

- `language-conformances-001` — Named code implementations supply traits; data, Self, implicit receivers and retired reference-row assignments do not.
- `language-conformances-002` — Every whole-map selection supplies exact explicit evidence and generic non-lifetime arguments without ambient search.
- `language-conformances-003` — Implementation visibility and direct dependencies remain independent of carrier/trait visibility and Build provider selection.
- `language-conformances-004` — Lifetime arguments retain declaration-order mappings, and elision requires a unique complete borrow-derived mapping.
- `language-conformances-005` — Every inherited overload has exact declaring-trait/signature/result-domain identity and appears once through inline body, default or authorized synthesis.
- `language-conformances-006` — Defaults call requirements through the same selected map and forwarding retains distinct authority and trust provenance.
- `language-conformances-007` — An exact satisfaction edge does not establish a whole map, license dyn or provide a missing ordinary body.
- `language-conformances-008` — Signature-free requirement references resolve independently of visible satisfiers and reject overloaded families.
- `language-conformances-009` — Transparent refinements meet complete contract axes without becoming new satisfaction targets or assumed-binding evidence.
- `language-conformances-010` — Sealed equality synthesis is explicit for records/payload sums and preserves active-payload semantics; proof identity selects no Equatable.
- `language-conformances-011` — Public implementation identity retains exact telescope changes and law/proof dependencies even when runtime code is erased.

**Remaining:** No dedicated existing fact folder establishes exact implementation selection, map completeness, forwarding authority, lifetime mappings or refinement laws. Corpus candidates must be linked separately.

<a id="language-constants"></a>

#### constants

**Source:** [wiki/spec/language/constants.md](../../spec/language/constants.md) · 3 headings · SHA-256 `6975db19f2273e00754b71109ee78bd7a95970be2d4a21e118ebd1b272f328e6`.

**Evidence needed:** source facts, semantic evaluation and materialization.

**Fact area candidates:** [length](../../../samples/cli/facts/length), [case_where_equality](../../../samples/cli/facts/case_where_equality), [runtime_count_index](../../../samples/cli/facts/runtime_count_index), [float_range_delivery](../../../samples/cli/facts/float_range_delivery).

**Other candidate areas:** [pass/constants (10 roots)](../../../tests/omega/pass/constants), [fail/constants (12 roots)](../../../tests/omega/fail/constants), [pass/comptime (15 roots)](../../../tests/omega/pass/comptime), [fail/comptime (16 roots)](../../../tests/omega/fail/comptime).

**Required families:**

- `language-constants-001` — Constants are copy-eligible immutable values with no cleanup/interior mutability, stable address, storage authority or struct field.
- `language-constants-002` — Concrete constant evaluation admits ordinary invocations under the same evaluation rules and does not make values anonymous.
- `language-constants-003` — Runtime addresses and late native code layout cannot become canonical semantic inputs to the same product.
- `language-constants-004` — Target-dependent constants retain exact target/provenance dependencies without changing declarations or field rosters.
- `language-constants-005` — Executable demand requires admitted computation or a checked realization, including branches depending on unrealized choices.
- `language-constants-006` — Every active materialized bit has selected layout/meaning, padding is zero and unspecified NaN payload cannot become image data.
- `language-constants-007` — Opaque quotient materialization retains its chosen representative; representative-independent uses need checked canonical form.
- `language-constants-008` — Borrowed materialized copies follow ordinary temporary lifetime and const names grant no global mutable storage.

**Remaining:** Const-name transports address narrow named values; they do not establish invocation admission, representation demand, target-dependency closure, NaN bits, quotient canonicalization or constant lifetime as a complete family.

<a id="language-counts-and-addresses"></a>

#### counts and addresses

**Source:** [wiki/spec/language/counts_and_addresses.md](../../spec/language/counts_and_addresses.md) · 9 headings · SHA-256 `380c2a4b53f089bed06cb00c3dcfa299ebd6cad04af6a895094ecfb91b5e9802`.

**Evidence needed:** source bounds facts and realization/address fixtures.

**Fact area candidates:** [index_every_width](../../../samples/cli/facts/index_every_width), [runtime_count_index](../../../samples/cli/facts/runtime_count_index), [field_where_index](../../../samples/cli/facts/field_where_index), [length](../../../samples/cli/facts/length), [remainder_width_index](../../../samples/cli/facts/remainder_width_index).

**Other candidate areas:** [pass/arithmetic (103 roots)](../../../tests/omega/pass/arithmetic), [fail/arithmetic (128 roots)](../../../tests/omega/fail/arithmetic), [run/arithmetic (6 roots)](../../../tests/omega/run/arithmetic), [pending/arithmetic (1 roots)](../../../tests/omega/pending/arithmetic), [pass/arrays (12 roots)](../../../tests/omega/pass/arrays), [fail/arrays (3 roots)](../../../tests/omega/fail/arrays), [pass/slices (176 roots)](../../../tests/omega/pass/slices), [fail/slices (130 roots)](../../../tests/omega/fail/slices), [run/slices (10 roots)](../../../tests/omega/run/slices), [pass/memory (10 roots)](../../../tests/omega/pass/memory), [fail/memory (9 roots)](../../../tests/omega/fail/memory).

**Required families:**

- `language-counts-and-addresses-001` — Counts use u64 and indexes any justified integer, while addr has an independent target model and grants no access authority.
- `language-counts-and-addresses-002` — Storage address exposure names the actual referent and preserves compatible-view identity without extending its loan.
- `language-counts-and-addresses-003` — Machine address exposure retains one exact selected application and executable demand without evaluating arguments or capturing context.
- `language-counts-and-addresses-004` — An address cannot become a callable, borrow, canonical host pointer or proof of byte geometry.
- `language-counts-and-addresses-005` — Interpreted live entries/storage have stable nonzero collision-free disjoint-kind identities, with no truncation or false byte adjacency.
- `language-counts-and-addresses-006` — Late code regions cover only actual attributed instruction spans and cannot feed circular same-product layout/evaluation.
- `language-counts-and-addresses-007` — Scalar indexing proves signed lower bounds and exact current live length independently of nominal index qualification.
- `language-counts-and-addresses-008` — Narrowing uses positive fit evidence and preserves inclusive whole-address-space counts rather than replacing <= with <.
- `language-counts-and-addresses-009` — Shrinking storage invalidates old index bounds; an index domain alone supplies no generation or runtime recheck.

**Remaining:** Bound/index rows cover only part of current-length and signed-bound rules. Address identity, entry retention, code-layout cycles and address-space fit need separate facts and realization evidence.

<a id="language-data-and-literals"></a>

#### data and literals

**Source:** [wiki/spec/language/data_and_literals.md](../../spec/language/data_and_literals.md) · 5 headings · SHA-256 `659b54824e6a108b08c8062dbf05e10192d2c09cb7690ab5021321593f07ad64`.

**Evidence needed:** source fact and lexical fixtures.

**Fact area candidates:** [case_where_equality](../../../samples/cli/facts/case_where_equality), [data_where_standing](../../../samples/cli/facts/data_where_standing), [field_where_index](../../../samples/cli/facts/field_where_index), [float_range_delivery](../../../samples/cli/facts/float_range_delivery).

**Other candidate areas:** [pass/data (21 roots)](../../../tests/omega/pass/data), [fail/data (21 roots)](../../../tests/omega/fail/data), [pass/arrays (12 roots)](../../../tests/omega/pass/arrays), [fail/arrays (3 roots)](../../../tests/omega/fail/arrays), [pass/structs (31 roots)](../../../tests/omega/pass/structs), [fail/structs (2 roots)](../../../tests/omega/fail/structs), [pass/cases (27 roots)](../../../tests/omega/pass/cases).

**Required families:**

- `language-data-and-literals-001` — Omitted runtime fields require complete zero-validity; erased omissions require an accessible nullary constructor.
- `language-data-and-literals-002` — Construction and restoring writes prove the complete data and active-case where predicates.
- `language-data-and-literals-003` — Mixed common/payload fields evaluate once in authored order while inactive payloads incur no obligations.
- `language-data-and-literals-004` — Case/domain/member identity is nominal and collisions or unqualified case value names reject.
- `language-data-and-literals-005` — Schema identity does not grant a runtime integer tag or raw-integer-to-case conversion.
- `language-data-and-literals-006` — Zero selects the first declared case without inventing emptiness, authority or a valid payload niche.
- `language-data-and-literals-007` — Executable structural equality observes common fields, active case and payload, whereas fact equality preserves proof-relevant identity.
- `language-data-and-literals-008` — Case predicates scope only actual common/own payload names and enclosing binders, not sibling payloads.
- `language-data-and-literals-009` — Matching an established case transports its exact constraints; contradictions do not convert generic arguments.
- `language-data-and-literals-010` — Case replacement and payload writes invalidate dependent facts and require restoration before observation.
- `language-data-and-literals-011` — Recursive constrained cases require the selected checked inductive interpretation.
- `language-data-and-literals-012` — Only specified ASCII syntax and whitespace tokenize; UTF-8 literal bytes are not Unicode identifiers.
- `language-data-and-literals-013` — Quoted byte literals use the exact escape roster, reject raw newlines/codepoint escapes, and grant no encoding qualification.
- `language-data-and-literals-014` — Literal-to-fixed-array materialization requires exact byte length and no escaping temporary evaluator views.

**Remaining:** Constrained integer/float field rows address a subset. Lexical framing, literal lifetime, nominal cases, zero gates, proof-relevant equality and recursive-case obligations need distinct linked fixtures or fact rows.

<a id="language-dependent-values"></a>

#### dependent values

**Source:** [wiki/spec/language/dependent_values.md](../../spec/language/dependent_values.md) · 8 headings · SHA-256 `5a7998d6c34f82234114a63e93007852f05bd54619864380f9b5561bde60c432`.

**Evidence needed:** source fact.

**Fact area candidates:** [data_where_standing](../../../samples/cli/facts/data_where_standing), [field_where_index](../../../samples/cli/facts/field_where_index), [case_where_equality](../../../samples/cli/facts/case_where_equality), [runtime_count_index](../../../samples/cli/facts/runtime_count_index), [guarded_line_loan](../../../samples/cli/facts/guarded_line_loan), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan), [returned_write_destination](../../../samples/cli/facts/returned_write_destination), [compound_index_guard](../../../samples/cli/facts/compound_index_guard), [indexed_element_mut_loan](../../../samples/cli/facts/indexed_element_mut_loan).

**Other candidate areas:** [pass/dependent (87 roots)](../../../tests/omega/pass/dependent), [fail/dependent (46 roots)](../../../tests/omega/fail/dependent), [pass/domains (81 roots)](../../../tests/omega/pass/domains), [fail/domains (161 roots)](../../../tests/omega/fail/domains), [run/domains (55 roots)](../../../tests/omega/run/domains), [pending/domains (1 roots)](../../../tests/omega/pending/domains), [pass/borrows (287 roots)](../../../tests/omega/pass/borrows), [fail/borrows (286 roots)](../../../tests/omega/fail/borrows), [run/borrows (30 roots)](../../../tests/omega/run/borrows), [pending/borrows (1 roots)](../../../tests/omega/pending/borrows).

**Required families:**

- `language-dependent-values-001` — Runtime-capable value indices retain exact subjects but never become const specialization identities.
- `language-dependent-values-002` — A view's range/stride is justified against its actual backing extent, not compiled record size.
- `language-dependent-values-003` — Runtime witnesses allocate no hidden layout descriptors, variable stack objects or proof tuples.
- `language-dependent-values-004` — A validator establishes only the predicates it checks; unproved refinement receives no inserted residual check or authority.
- `language-dependent-values-005` — A dependent borrow pins every witness needed for validity until the borrow ends.
- `language-dependent-values-006` — Call/state substitutions retain exact subjects and stale entry facts cannot justify a backedge.
- `language-dependent-values-007` — Zeroed storage establishes a value only when its complete active default domain holds, including generic-body obligations.
- `language-dependent-values-008` — Zero gating propagates through containment and does not establish partially initialized array elements.
- `language-dependent-values-009` — Mutable arguments assume and restore declared default domains but preserve additional input qualifications only when contracted.
- `language-dependent-values-010` — A total collection predicate needs checked reasoning to derive an indexed element property.
- `language-dependent-values-011` — A domain invariant window and an ownership hole carry separate restoration debt.
- `language-dependent-values-012` — Invariant debt must close before dependent reads, borrows, calls, transitions, returns, scope expiration or boundary calls.
- `language-dependent-values-013` — Write-only restoration cannot observe old contents; recoverable failure does not cancel debt.
- `language-dependent-values-014` — Mutation frames include argument evaluation and authorized callee state; incomplete origins use the conservative ceiling.
- `language-dependent-values-015` — Invalidate only overlapping atoms and preserve distinct snapshots; frames grant no access authority.

**Remaining:** No identified complete row set for dependent-witness pinning, generic zero gates, collection theorem extraction, separate domain/ownership debt and unknown-versus-empty mutation frames.

<a id="language-domains"></a>

#### domains

**Source:** [wiki/spec/language/domains.md](../../spec/language/domains.md) · 12 headings · SHA-256 `3c492ca9fb561a19781beaf2f0447eaa990fc4cb225c9a896d088975516d6af1`.

**Evidence needed:** source facts and qualification/authority fixtures.

**Fact area candidates:** [data_where_standing](../../../samples/cli/facts/data_where_standing), [float_range_delivery](../../../samples/cli/facts/float_range_delivery), [range_fact_delivery](../../../samples/cli/facts/range_fact_delivery), [guarded_line_loan](../../../samples/cli/facts/guarded_line_loan), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan).

**Other candidate areas:** [pass/domains (81 roots)](../../../tests/omega/pass/domains), [fail/domains (161 roots)](../../../tests/omega/fail/domains), [run/domains (55 roots)](../../../tests/omega/run/domains), [pending/domains (1 roots)](../../../tests/omega/pending/domains), [pass/capabilities (13 roots)](../../../tests/omega/pass/capabilities), [fail/capabilities (9 roots)](../../../tests/omega/fail/capabilities).

**Required families:**

- `language-domains-001` — Qualification preserves carrier representation, fixed length, capacity and storage kind.
- `language-domains-002` — A domain binds its refinement subject explicitly and substitutes exact generic arguments without hidden self or runtime storage.
- `language-domains-003` — Predicate-only membership proves its predicates; routed membership additionally retains exact authorized provenance.
- `language-domains-004` — Predicate-free route-free qualification adds no issuer requirement, including linear carriers whose ownership remains intact.
- `language-domains-005` — Separate domains retain exact nominal identity even with equal predicates and unrelated domains cannot alter minting rules.
- `language-domains-006` — Moves/copies transport qualification but rebuilding equal carrier fields cannot mint it.
- `language-domains-007` — Well-formed uninhabited domains remain legal and their conditional entry assumptions cannot manufacture a reachable inhabitant.
- `language-domains-008` — Refinement inherits predicates and routes; unions/intersections and ordered executable tests retain their distinct proof rules.
- `language-domains-009` — Executable domain tests require finite pure predicates and cannot manufacture routed provenance.
- `language-domains-010` — Exact 'as' preserves denotation/representability; semantic/policy erasure is explicit and owned claims cannot be cast away.
- `language-domains-011` — Role normalization permits compatible predicate/meaning/policy contributions but rejects competing meanings and policies.
- `language-domains-012` — Named result-domain overloads select exact semantic/routed/tag projections before checking predicate obligations.
- `language-domains-013` — Aliases expand acyclic compatible atoms and publish every constituent; stronger entailment never changes identity or operator meaning.
- `language-domains-014` — Indexed families retain exact static arguments, roles and licensed algebra without inferred maxima, hidden conformance selection or implicit variance.
- `language-domains-015` — Encoding predicates preserve live byte geometry, require explicit checked validation and retain codepoint-boundary obligations on UTF-8 slices.

**Remaining:** Existing anonymous bounds and loans cannot stand in for nominal domain identity, provenance, uninhabitedness, role normalization, explicit erasure, result-domain dispatch or encoding-preservation evidence.

<a id="language-effects"></a>

#### effects

**Source:** [wiki/spec/language/effects.md](../../spec/language/effects.md) · 13 headings · SHA-256 `439f937a3544d74341ba378eedc031970e2878da5f14ecb6f36aa355e6e23fb6`.

**Evidence needed:** source effect facts and boundary integration.

**Fact area candidates:** [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan), [call_result_bound_discharge](../../../samples/cli/facts/call_result_bound_discharge).

**Other candidate areas:** [pass/effects (14 roots)](../../../tests/omega/pass/effects), [fail/effects (13 roots)](../../../tests/omega/fail/effects), [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers).

**Required families:**

- `language-effects-001` — Boundary calls contribute their explicit reach rows, not an implicit containing-trait service.
- `language-effects-002` — Direct service use requires authored reach while ordinary checked calls propagate normalized transitive reach.
- `language-effects-003` — Reach identities, runtime authority, provider trust, suspension, blocking, crashes and positive progress remain independent.
- `language-effects-004` — Published bodyless omissions are empty/no-pause ceilings while private checked operational summaries may be inferred.
- `language-effects-005` — Recoverable outcomes retain exact case guarantees and non-Unit discard still requires legal ownership disposition.
- `language-effects-006` — Reach normalization uses exact identities, idempotent union and boundary-parent closure across recursive fixed points.
- `language-effects-007` — Opaque/dynamic calls keep public ceilings and product exclusions cannot silently narrow them.
- `language-effects-008` — Nominal static callback dependencies specialize finite bounded unions by exact selected public contracts.
- `language-effects-009` — Synchronous invokes edges retain exact bindings and their component graph is acyclic; durable external registration is separate.
- `language-effects-010` — Call acknowledgement markers equal the complete static suspend/block envelope and do not change semantics or authority. Candidate: `effects-block`.
- `language-effects-011` — Suspending calls occur only at permitted continuation boundaries and retain held loans/carry obligations.
- `language-effects-012` — Every actual crash-site guard implies the same-cause union of published alternatives after exact substitution.
- `language-effects-013` — Crash/restart and process-exit survivor safety need independent isolation/recovery contracts.
- `language-effects-014` — Published rows, invocation identity and installation-bound reach retain their exact distinct normalization rules.

**Remaining:** Ordinary scalar/loan rows do not establish normalized reach, callback dependency specialization, synchronous topology, acknowledgement legality or complete crash coverage.

<a id="language-evaluation"></a>

#### evaluation

**Source:** [wiki/spec/language/evaluation.md](../../spec/language/evaluation.md) · 8 headings · SHA-256 `dbb75cb5e53cda4f2586afb2d0fddb601ec433d7081130cf633e65596fa883b0`.

**Evidence needed:** semantic evaluation facts and reproducibility controls.

**Fact area candidates:** [case_where_equality](../../../samples/cli/facts/case_where_equality), [runtime_count_index](../../../samples/cli/facts/runtime_count_index), [length](../../../samples/cli/facts/length).

**Other candidate areas:** [pass/comptime (15 roots)](../../../tests/omega/pass/comptime), [fail/comptime (16 roots)](../../../tests/omega/fail/comptime), [pass/constants (10 roots)](../../../tests/omega/pass/constants), [fail/constants (12 roots)](../../../tests/omega/fail/constants), [pass/proofs (115 roots)](../../../tests/omega/pass/proofs), [fail/proofs (191 roots)](../../../tests/omega/fail/proofs), [run/proofs (2 roots)](../../../tests/omega/run/proofs), [pending/proofs (1 roots)](../../../tests/omega/pending/proofs).

**Required families:**

- `language-evaluation-001` — Concrete semantic evaluation preserves target execution meaning and selected operation/conformance identities without phase observations.
- `language-evaluation-002` — Invocation admission independently proves termination, empty reach, no pauses, no unhandled abnormal route, no authority consumption and no escaping mutation.
- `language-evaluation-003` — Axiomatic or assumed runtime binding relationships cannot supply executable computation or bypass selection authority.
- `language-evaluation-004` — Invocation-specific facts may discharge a trap route without pretending an opaque contract has globally narrower behavior.
- `language-evaluation-005` — Every evaluation owns a fresh value graph and returns only a valid owned snapshot, never interpreter cells or aliases.
- `language-evaluation-006` — Address queries and late code layout cannot smuggle host identities or unfinished native geometry into constants.
- `language-evaluation-007` — Proof-only references preserve assumption closure while relevant control/data dependencies still demand an algorithm.
- `language-evaluation-008` — Root sponsorship bounds canonical work/live storage and unobservable exhaustion cannot change the returned value.
- `language-evaluation-009` — The target capsule fixes exact semantic subjects and excludes ambient host environment, clock, pointers and physical state.
- `language-evaluation-010` — Target-dependent values retain both observation and selected-realization dependencies through signatures, caches, proofs and plans.
- `language-evaluation-011` — Canonical static atoms require decidable exact encoding and reject floats, references, slices and unproved quotient representatives as identity keys.
- `language-evaluation-012` — Result caching and usage accounting retain distinct versioned identities and warm hits receive equivalent canonical charge.
- `language-evaluation-013` — Generated/default bodies preserve author authority, ordinary evaluation eligibility, exact selection snapshots and complete validated coverage.

**Remaining:** Named constants and selected generic uses are not a complete semantic-evaluation admission/materialization/cache test suite. Add exact eligibility and negative dependency/escape controls.

<a id="language-expressions"></a>

#### expressions

**Source:** [wiki/spec/language/expressions.md](../../spec/language/expressions.md) · 9 headings · SHA-256 `db7de5634a5231d2c1c51a0a7ade63c367d955a78429672f1b21431de35696a0`.

**Evidence needed:** source fact and resolution fixtures.

**Fact area candidates:** [compound_index_guard](../../../samples/cli/facts/compound_index_guard), [computed_record_window](../../../samples/cli/facts/computed_record_window), [guarded_line_loan](../../../samples/cli/facts/guarded_line_loan), [indexed_element_mut_loan](../../../samples/cli/facts/indexed_element_mut_loan), [returned_write_destination](../../../samples/cli/facts/returned_write_destination), [alias_fact_transfer](../../../samples/cli/facts/alias_fact_transfer), [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff).

**Other candidate areas:** [pass/expressions (79 roots)](../../../tests/omega/pass/expressions), [fail/expressions (28 roots)](../../../tests/omega/fail/expressions), [pass/operators (27 roots)](../../../tests/omega/pass/operators), [fail/operators (13 roots)](../../../tests/omega/fail/operators), [pass/calls (289 roots)](../../../tests/omega/pass/calls), [fail/calls (117 roots)](../../../tests/omega/fail/calls), [run/calls (28 roots)](../../../tests/omega/run/calls).

**Required families:**

- `language-expressions-001` — Operands, fields, indexes and ranges evaluate exactly once in the specified authored order.
- `language-expressions-002` — Short-circuit facts stay within their selected path and side effects invalidate stale guard subjects.
- `language-expressions-003` — Destination capture precedes RHS evaluation and later selector/reference changes cannot redirect the store.
- `language-expressions-004` — Named calls use static qualified paths and explicit ordinary parameters, with no implicit receiver or dot-call lookup.
- `language-expressions-005` — Partial mathematical application is distinct from a complete executable machine invocation.
- `language-expressions-006` — Shared, mutable and write-only argument admission preserve declared access and temporary backing lifetime.
- `language-expressions-007` — Operator tokens belong to one exact machine/signature, with structural duplicate rejection and no result-only overload distinction.
- `language-expressions-008` — Trait-backed token use needs exact explicit conformance evidence; Build providers cannot change source token meaning.
- `language-expressions-009` — Direct, trait, boundary and compiler-catalog declarations each use their authorized single supply route.
- `language-expressions-010` — Closed-family declaration ownership and complete normalized operand tuples prevent unrelated imports changing resolution.
- `language-expressions-011` — Fixed-array/slice indexing adaptation borrows only the admitted collection operand and never grants extra write authority.
- `language-expressions-012` — Index results may be values or exact views; a returned value is not writable storage.
- `language-expressions-013` — A new mutable range view needs explicit '&mut', including expected mutable destinations and empty ranges.
- `language-expressions-014` — Inclusive endpoints prove successor representability and correct exclusive/inclusive bounds without wrapping.

**Remaining:** No full fact-row set for once-only side effects, captured destinations under later writes, operator supply/ownership, scoped trait selection, lifetime-bound temporary results and endpoint overflow.

<a id="language-generics"></a>

#### generics

**Source:** [wiki/spec/language/generics.md](../../spec/language/generics.md) · 15 headings · SHA-256 `5b12f2f8150501a1a800ac0175cd75ead7c72da77ef32f3adb6d20ba3e4e22d8`.

**Evidence needed:** source facts and generic-contract fixtures.

**Fact area candidates:** [runtime_count_index](../../../samples/cli/facts/runtime_count_index), [case_where_equality](../../../samples/cli/facts/case_where_equality), [length](../../../samples/cli/facts/length), [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff).

**Other candidate areas:** [pass/generics (131 roots)](../../../tests/omega/pass/generics), [fail/generics (221 roots)](../../../tests/omega/fail/generics), [run/generics (1 roots)](../../../tests/omega/run/generics), [pass/traits (92 roots)](../../../tests/omega/pass/traits), [fail/traits (117 roots)](../../../tests/omega/fail/traits), [run/traits (3 roots)](../../../tests/omega/run/traits).

**Required families:**

- `language-generics-001` — Generic bodies check declared requirements independently of later selected instances and validate all argument kinds including unused binders.
- `language-generics-002` — Substitution uses exact declaration identities throughout types/contracts and preserves static carrier widths and runtime captured subjects.
- `language-generics-003` — Const binders require static knowledge; runtime binders retain values without JIT, hidden boxing, maximum-sized storage or silent contract strengthening.
- `language-generics-004` — Later mutation of an index source cannot retag existing applications; different runtime subjects need checked compatibility.
- `language-generics-005` — Static type equality preserves nominal owners, access, lifetimes, domain roles and exact normalized arguments without layout-based equivalence.
- `language-generics-006` — Structural equations recover only unique explicit-binder arguments with repeated-occurrence consistency and occurs checks.
- `language-generics-007` — Flow bounds and private stronger contracts cannot choose a generic argument or change declared application identity.
- `language-generics-008` — Canonical domain indices use exact licensed normalization, not general predicate equivalence or greatest-member search.
- `language-generics-009` — Explicit static capacity remains distinct from live length, byte supply, initialization and dynamic allocation.
- `language-generics-010` — Finite specialization preserves complete correlated tuples, admitted alternatives and exact membership without a guessed fallback.
- `language-generics-011` — Named implementation evidence supplies exact complete trait rows; neither data subjects nor ambient search select it.
- `language-generics-012` — Structural callable, nominal callable and declaration-identity binders retain different authorized uses and complete contracts.
- `language-generics-013` — Invocation-lifetime families refine every fresh arbitrary invocation rather than one captured environment lifetime.
- `language-generics-014` — Nested machine contracts preserve binder-positional shape and all reach/crash/termination/authority axes.
- `language-generics-015` — Mathematical term and universe binders remain distinct from executable declaration supply and runtime values.
- `language-generics-016` — Published alpha-normalized binder identity and exact axiom-template receipts survive specialization without laundering trust.

**Remaining:** Runtime count and selected const witnesses cover narrow binders. No full row set establishes generic identity, inference rejection, correlated specialization, callable categories, lifetime universality, nested contracts or axiom-template assumptions.

<a id="language-lifetimes"></a>

#### lifetimes

**Source:** [wiki/spec/language/lifetimes.md](../../spec/language/lifetimes.md) · 4 headings · SHA-256 `eb7b68ef7d341b4a3fa9d91d196c620ecadf98685b6022bf7310b794f7df96ad`.

**Evidence needed:** source facts and returned-view fixtures.

**Fact area candidates:** [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff), [returned_write_destination](../../../samples/cli/facts/returned_write_destination), [guarded_line_loan](../../../samples/cli/facts/guarded_line_loan), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan), [computed_record_window](../../../samples/cli/facts/computed_record_window), [indexed_element_mut_loan](../../../samples/cli/facts/indexed_element_mut_loan).

**Other candidate areas:** [pass/borrows (287 roots)](../../../tests/omega/pass/borrows), [fail/borrows (286 roots)](../../../tests/omega/fail/borrows), [run/borrows (30 roots)](../../../tests/omega/run/borrows), [pending/borrows (1 roots)](../../../tests/omega/pending/borrows), [pass/borrow (51 roots)](../../../tests/omega/pass/borrow), [fail/borrow (71 roots)](../../../tests/omega/fail/borrow).

**Required families:**

- `language-lifetimes-001` — Lifetime binders have lexical identity and declared arity and their erasure removes no loan obligation.
- `language-lifetimes-002` — Fresh invocation lifetimes refine every admissible invocation independently of captured environment lifetimes.
- `language-lifetimes-003` — An output lifetime relates exact reference leaves to live input backing without itself granting access or establishing an origin.
- `language-lifetimes-004` — Output elision requires one unambiguous reference source, not first-argument or same-spelling guesses.
- `language-lifetimes-005` — Nested records, active payloads, arrays, domains and concrete generic arguments retain the complete returned-loan frontier.
- `language-lifetimes-006` — Moving/enclosing/projecting a carrier transports only its exact captured loan paths; unresolved dynamic selection retains all possible sources.
- `language-lifetimes-007` — Replacement evaluates with old loans live, ends only overwritten carriers, then installs new loans without prematurely restoring the parent.
- `language-lifetimes-008` — Filling an ownership hole does not end the already-moved old loans twice.
- `language-lifetimes-009` — State transport requires backing outlive the destination, and same-carrier erasure preserves access and source identities.

**Remaining:** Selected loan journeys cover only part of elision ambiguity, full structural frontiers, sibling replacement, per-invocation lifetimes and moved-hole replacement evidence.

<a id="language-machines"></a>

#### machines

**Source:** [wiki/spec/language/machines.md](../../spec/language/machines.md) · 8 headings · SHA-256 `cd85a6e01df931b0148b9a3be96ef02c00e5c4363846ce124af78359da8df146`.

**Evidence needed:** source fact and contract-refinement fixtures.

**Fact area candidates:** [call_result_bound_discharge](../../../samples/cli/facts/call_result_bound_discharge), [range_fact_delivery](../../../samples/cli/facts/range_fact_delivery), [float_range_delivery](../../../samples/cli/facts/float_range_delivery), [field_where_index](../../../samples/cli/facts/field_where_index).

**Other candidate areas:** [pass/machines (9 roots)](../../../tests/omega/pass/machines), [fail/machines (6 roots)](../../../tests/omega/fail/machines), [pass/calls (289 roots)](../../../tests/omega/pass/calls), [fail/calls (117 roots)](../../../tests/omega/fail/calls), [run/calls (28 roots)](../../../tests/omega/run/calls), [pass/parameters (2 roots)](../../../tests/omega/pass/parameters).

**Required families:**

- `language-machines-001` — Machines and named implementations are static code identities with all state and authority supplied explicitly.
- `language-machines-002` — Invocation scratch remains ordinary local storage and crosses states only through explicit arguments.
- `language-machines-003` — Every runtime, token, compile-time and mathematical call proves all selected requires even when its result is discarded.
- `language-machines-004` — Recursive proof calls additionally prove exact well-founded descent; preconditions do not substitute for descent.
- `language-machines-005` — Checked, required, external, admitted-claim and exact catalog supplies remain distinct and unauthorized bodyless free declarations reject.
- `language-machines-006` — Every target-scoped body is checked before realization selects a target, and missing realization bodies reject only that realization.
- `language-machines-007` — Requirement refinement independently checks premises, guarantees, reach, suspension/blocking, outcomes, progress, resources and calling plans.
- `language-machines-008` — Observation projection cannot conceal authority use, blocking, resources or required temporal behavior.
- `language-machines-009` — Published semantic identity preserves the public contract/supply and excludes private ranking witness details.

**Remaining:** Bounded call facts do not establish complete multi-axis contract refinement, declaration supply, unconditional target checking or observation-compatible replacement.

<a id="language-modules"></a>

#### modules

**Source:** [wiki/spec/language/modules.md](../../spec/language/modules.md) · 7 headings · SHA-256 `16581a5107eb9f67c34ac05835adc97782b984f2654122cddbe426517edcd8d4`.

**Evidence needed:** resolution and package-boundary fixtures.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/modules (66 roots)](../../../tests/omega/pass/modules), [fail/modules (47 roots)](../../../tests/omega/fail/modules), [pending/modules (1 roots)](../../../tests/omega/pending/modules), [pass/packages (2 roots)](../../../tests/omega/pass/packages), [fail/packages (3 roots)](../../../tests/omega/fail/packages).

**Required families:**

- `language-modules-001` — Logical imports respect requester-local direct dependencies and product/build scope without filesystem reach or fallback.
- `language-modules-002` — Imports are file-local and nontransitive while module-owned declarations retain their specified visibility.
- `language-modules-003` — Only the exact selected domain or directly declared module domains become exposed; carriers and ordinary machine imports expose no sibling extensions.
- `language-modules-004` — Foreign attached domains retain distinct declaring owner and carrier identity under shadowing.
- `language-modules-005` — Lookup prioritizes lexical/state/machine bindings and rejects ambiguous visible declarations independently of traversal order.
- `language-modules-006` — Case value names require carrier qualification and unresolved bare names cannot become inferred cases.
- `language-modules-007` — Attached declarations own visibility independently; public signatures cannot expose unauthorized private declarations.
- `language-modules-008` — Private issuer metadata and conformance implementations grant no consumer selection authority.
- `language-modules-009` — Public structural shape permits only invariant/loan/qualification-respecting operations and grants no routed authority or ABI guarantee.

**Remaining:** Existing fact transport does not establish import/visibility/dependency resolution. Link focused modules/package fixtures and keep these checks visible outside transport rows.

<a id="language-numeric-values"></a>

#### numeric values

**Source:** [wiki/spec/language/numeric_values.md](../../spec/language/numeric_values.md) · 16 headings · SHA-256 `37f3212dd6faa9292cdda2b5bddd56857765bfb17c7f23219f0f055d8d3298a0`.

**Evidence needed:** source facts and matching-host numeric qualification.

**Fact area candidates:** [bounded_scalar_sums](../../../samples/cli/facts/bounded_scalar_sums), [exact_sub_guard](../../../samples/cli/facts/exact_sub_guard), [dependent_subtract](../../../samples/cli/facts/dependent_subtract), [data_where_standing](../../../samples/cli/facts/data_where_standing), [field_where_index](../../../samples/cli/facts/field_where_index), [float_range_delivery](../../../samples/cli/facts/float_range_delivery), [range_fact_delivery](../../../samples/cli/facts/range_fact_delivery), [index_every_width](../../../samples/cli/facts/index_every_width), [remainder_width_index](../../../samples/cli/facts/remainder_width_index), [compound_index_guard](../../../samples/cli/facts/compound_index_guard), [length](../../../samples/cli/facts/length), [bounded_products](../../../samples/cli/facts/bounded_products).

**Other candidate areas:** [pass/arithmetic (103 roots)](../../../tests/omega/pass/arithmetic), [fail/arithmetic (128 roots)](../../../tests/omega/fail/arithmetic), [run/arithmetic (6 roots)](../../../tests/omega/run/arithmetic), [pending/arithmetic (1 roots)](../../../tests/omega/pending/arithmetic), [pass/float (124 roots)](../../../tests/omega/pass/float), [fail/float (101 roots)](../../../tests/omega/fail/float), [run/float (1 roots)](../../../tests/omega/run/float), [pass/core (41 roots)](../../../tests/omega/pass/core), [fail/core (54 roots)](../../../tests/omega/fail/core).

**Required families:**

- `language-numeric-values-001` — Scalar bounds are ordinary propositions in data/case where and requires/ensures, not bracketed scalar type suffixes.
- `language-numeric-values-002` — Numeric helper names resolve ordinary library machines and derive facts from checked contracts rather than spellings.
- `language-numeric-values-003` — Anonymous arithmetic keeps exact rational intermediates until landing; typed operands retain their selected carrier.
- `language-numeric-values-004` — A suffixed or already landed value cannot be made anonymous or silently change width/format by folding or a destination.
- `language-numeric-values-005` — Fractional integer landing rejects; a fractional intermediate with integral final landing retains its specified warning origin.
- `language-numeric-values-006` — Every Exact intermediate proves representability in its own carrier, even for full-width unsigned values.
- `language-numeric-values-007` — Selected operation/policy identity survives negation, const normalization, contract reasoning and target realization.
- `language-numeric-values-008` — Mixed signed/unsigned comparisons use exact mathematical values with no implicit common carrier or widening.
- `language-numeric-values-009` — Wrapping, Saturating and Trapping affect each operation locally; explicit erasure exposes the stored result to later Exact operations.
- `language-numeric-values-010` — Division/remainder require nonzero divisors and preserve truncation toward zero including signed-minimum edge policies.
- `language-numeric-values-011` — Shift-count laws remain policy-specific and left-shift result representability is a separate proof.
- `language-numeric-values-012` — Bitwise bounds respect full carrier width and cannot be inferred by evaluating only interval endpoints.
- `language-numeric-values-013` — IEEE f32/f64 identities, rounding and special values remain target-independent; fused and unfused operations are distinct.
- `language-numeric-values-014` — NaN comparisons, signed zero, operand-ordered minimum/maximum and totalOrder keep their distinct meanings.
- `language-numeric-values-015` — FloatMeaning loses NaN payload only in proof meaning; raw-bit materialization needs the promised refinement.
- `language-numeric-values-016` — Finite qualifications and float policy adapters preserve separate zero-divisor, result-finiteness and invalid-operation obligations.
- `language-numeric-values-017` — Destination-owned conversions retain rounding mode, representability, exact result-domain selection and explicit policy erasure.
- `language-numeric-values-018` — Canonical floating controls and foreign/callback restoration need matching-host execution evidence.
- `language-numeric-values-019` — Scalar/range/nested indexes prove each exact extent and signed lower bound; bounds grant no loan or element-domain authority.

**Remaining:** Arithmetic bound rows are a subset. Literal landing, selected nonbuiltin semantics, policies, full-width edges, IEEE distinctions, conversion identities and host floating-control preservation need explicit separate controls and qualification.

<a id="language-ownership"></a>

#### ownership

**Source:** [wiki/spec/language/ownership.md](../../spec/language/ownership.md) · 9 headings · SHA-256 `0d7d7c91a3801fc8235e564f2ce5b55ad907b081bd129133f77c6047a18fc54f`.

**Evidence needed:** source ownership facts and cleanup fixtures.

**Fact area candidates:** [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff), [guarded_line_loan](../../../samples/cli/facts/guarded_line_loan), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan), [returned_write_destination](../../../samples/cli/facts/returned_write_destination), [indexed_element_mut_loan](../../../samples/cli/facts/indexed_element_mut_loan), [computed_record_window](../../../samples/cli/facts/computed_record_window).

**Other candidate areas:** [pass/ownership (140 roots)](../../../tests/omega/pass/ownership), [fail/ownership (105 roots)](../../../tests/omega/fail/ownership), [run/ownership (7 roots)](../../../tests/omega/run/ownership), [pass/drops (90 roots)](../../../tests/omega/pass/drops), [fail/drops (56 roots)](../../../tests/omega/fail/drops), [pass/borrows (287 roots)](../../../tests/omega/pass/borrows), [fail/borrows (286 roots)](../../../tests/omega/fail/borrows), [run/borrows (30 roots)](../../../tests/omega/run/borrows), [pending/borrows (1 roots)](../../../tests/omega/pending/borrows).

**Required families:**

- `language-ownership-001` — Copy, affine and linear multiplicity retain contained obligations, active-case identity and exact cleanup responsibility.
- `language-ownership-002` — Copied shared-reference carriers retain original backing/lifetimes but cannot acquire exclusive access or carry permission.
- `language-ownership-003` — Implicit zero filling creates no linear value; explicit valid zero construction creates an ordinary exact obligation.
- `language-ownership-004` — Logical propositions remain reusable while erased and zero-sized Type permissions retain exact ownership.
- `language-ownership-005` — Every continuing path accounts for live claims and actual joins agree on the exact frontier.
- `language-ownership-006` — Partial moves preserve siblings, reject overlapping extraction and cannot bypass nominal whole-value cleanup.
- `language-ownership-007` — Shared, exclusive and write-only loans retain original backing and exact reborrow lineage through closure and suspension.
- `language-ownership-008` — A borrowed-storage ownership hole transfers old custody and prohibits ancestor observation until exact restoration.
- `language-ownership-009` — Restoration preserves removed/replacement contained loans independently and is required at every fence, return, transition and resume/cancel path.
- `language-ownership-010` — Consuming outcomes preserve exact input provenance; borrowed calls cannot silently dispose or return fabricated copies.
- `language-ownership-011` — Automatic cleanup requires an authorized infallible normally returning nonblocking/nonsuspending no-exit contract.
- `language-ownership-012` — Only the type owner declares its single exact drop hook and the whole valid hook precedes structural cleanup.
- `language-ownership-013` — Cleanup prerequisites are proved locally without inventing new public caller demands or dropping generic element obligations.
- `language-ownership-014` — Destination-first replacement uses the same captured place for displacement and installation.
- `language-ownership-015` — Partial staging cleans in reverse establishment order; complete aggregates and roots clean in reverse declaration order, with exact active residual arrays.
- `language-ownership-016` — Whole-claim conservation, content decomposition and carry remain independent; every permission event has valid realization or checked no-code reason.

**Remaining:** Borrow-focused rows do not establish exact-once owned disposition, partial moves, ownership holes, generic cleanup prerequisites, cleanup ordering or content/carry conservation.

<a id="language-patterns"></a>

#### patterns

**Source:** [wiki/spec/language/patterns.md](../../spec/language/patterns.md) · 3 headings · SHA-256 `4a7db061e9fa062b59e5310857283b110ab8a10e6153d7ddbfaae7d9328872e2`.

**Evidence needed:** source fact and dispatch fixtures.

**Fact area candidates:** [case_where_equality](../../../samples/cli/facts/case_where_equality), [matched_field_windows](../../../samples/cli/facts/matched_field_windows), [computed_record_window](../../../samples/cli/facts/computed_record_window), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan), [float_range_delivery](../../../samples/cli/facts/float_range_delivery).

**Other candidate areas:** [pass/cases (27 roots)](../../../tests/omega/pass/cases), [pass/control_flow (173 roots)](../../../tests/omega/pass/control_flow), [fail/control_flow (31 roots)](../../../tests/omega/fail/control_flow).

**Required families:**

- `language-patterns-001` — Dispatch evaluates the subject once, tests alternatives in authored order and executes only the selected body.
- `language-patterns-002` — Field bindings preserve exact saved-subject ownership; complete patterns reject declaration drift without explicit '..'.
- `language-patterns-003` — A constrained-case match introduces only that exact active case's propositions and exact type equations.
- `language-patterns-004` — Every authored dispatch independently covers its admitted subject; later dispatches or crashes cannot fill its missing arms.
- `language-patterns-005` — Finite cases, Boolean tuples and complementary guards use their specified coverage rules.
- `language-patterns-006` — A case is excluded only by checked contradiction, not proof-search or construction failure.
- `language-patterns-007` — Wildcard Unit completion supplies no result guarantee, linear cleanup or arbitrary migration completeness.
- `language-patterns-008` — Executable domain tests require finite pure checks and routed provenance, and overlapping domains retain first-match order.

**Remaining:** Existing selected-arm and match-join rows do not establish exhaustive coverage, pattern drift, impossible-case proof or domain-test eligibility as a complete family.

<a id="language-process-exit"></a>

#### process exit

**Source:** [wiki/spec/language/process_exit.md](../../spec/language/process_exit.md) · 6 headings · SHA-256 `7f8838e9d168a28cc3296e669f98cce9499c728e92fad5f7970b35e2ac7136af`.

**Evidence needed:** source terminal facts and admitted host integration.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host), [pass/termination (136 roots)](../../../tests/omega/pass/termination), [fail/termination (91 roots)](../../../tests/omega/fail/termination), [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers).

**Required families:**

- `language-process-exit-001` — Only the exact canonical ProcessExit requirement plus actual domain-bound authority grants process-ending semantics.
- `language-process-exit-002` — Reach, core import, provider selection and console I/O cannot create exit authority.
- `language-process-exit-003` — Exit affects only its bound execution domain and preserves exact preceding trace and semantic i32 status.
- `language-process-exit-004` — Exit has no normal continuation, result or cleanup; a conditional helper still has ordinary returning paths.
- `language-process-exit-005` — Termination/ranking neither authorizes exit nor establishes normal-return postconditions or release.
- `language-process-exit-006` — Exit abandons in-domain obligations without creating disposition receipts or merging a post-exit frontier.
- `language-process-exit-007` — Cross-domain survivors require their own isolation/custody/recovery contracts, including DMA and locks.
- `language-process-exit-008` — Graceful entry return requires settling/transferring remaining activations under the root contract.
- `language-process-exit-009` — Cleanup's complete transitive contract excludes process exit even though exit is terminating and noncrashing.
- `language-process-exit-010` — Terminal/interpreter/native representation preserves exact external-completion identity and cannot substitute a returning mock, divergence or abort.

**Remaining:** Exit-zero sample oracles are not evidence for ProcessExit authority, survivor safety, conditional completion or exact full-width status semantics. Dedicated process-exit facts and host controls are needed.

<a id="language-reflection"></a>

#### reflection

**Source:** [wiki/spec/language/reflection.md](../../spec/language/reflection.md) · 13 headings · SHA-256 `ad0ff1852a214fba580bf0bd4bb835d3610775f01ce1f166c58407be79abf32b`.

**Evidence needed:** typed reflection facts and elaboration fixtures.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/generics (131 roots)](../../../tests/omega/pass/generics), [fail/generics (221 roots)](../../../tests/omega/fail/generics), [run/generics (1 roots)](../../../tests/omega/run/generics), [pass/layouts (90 roots)](../../../tests/omega/pass/layouts), [fail/layouts (57 roots)](../../../tests/omega/fail/layouts), [run/layouts (7 roots)](../../../tests/omega/run/layouts).

**Required families:**

- `language-reflection-001` — Owned TypeSchema descriptions preserve complete semantic member/type/relevance identity without borrowing compiler storage or truncating rosters.
- `language-reflection-002` — Describing a qualification or erased field establishes no instance fact, authority or proof.
- `language-reflection-003` — Schema-to-layout correspondence requires the exact validated plan and no member-index-as-offset shortcut.
- `language-reflection-004` — Reflection uses lexical author visibility and explicit delegated operations, never caller authority or opaque representative inspection.
- `language-reflection-005` — Typed visitation elaborates exact per-member applications and fresh subloans, including zero-sized linear fields but no runtime borrow of erased fields.
- `language-reflection-006` — Sum visitation reports the active case once, common fields once and only its payload while checking all admitted case applications.
- `language-reflection-007` — Every included member has exactly one valid policy selection on successful completion and forged/stale member-type pairs reject.
- `language-reflection-008` — Selection contexts are locally owned and freeze into symbolic snapshots independently validated before generated calls.
- `language-reflection-009` — Policy code, schema revision, static inputs, operations and consulted target dependencies retain exact derivation identity.
- `language-reflection-010` — Recursive derivation uses exact pending identities without assuming their proofs or dropping ever-growing specializations.
- `language-reflection-011` — Runtime recursion provisions explicit pending work and no hidden recursive stack or object traversal policy.
- `language-reflection-012` — Descriptions expose no automatic getter, setter, construction, MMIO or routed-authority capability.
- `language-reflection-013` — Editing invariant-preserving values still requires live access and cannot mutate loans held by a fuel-paused activation.
- `language-reflection-014` — Psi elaborates ordinary checked operations; semantic keys and same-spelled APIs cannot manufacture compiler privileges.

**Remaining:** No existing fact row covers semantic reflection, complete member/case selection, per-invocation reborrows, schema authority or paused-activation edit rejection.

<a id="language-state-contracts"></a>

#### state contracts

**Source:** [wiki/spec/language/state_contracts.md](../../spec/language/state_contracts.md) · 5 headings · SHA-256 `471cbba09d31fc62d4a05207a49420675308dd275a36accf63644447721c5668`.

**Evidence needed:** source fact.

**Fact area candidates:** [length](../../../samples/cli/facts/length), [alias_fact_transfer](../../../samples/cli/facts/alias_fact_transfer), [call_result_bound_discharge](../../../samples/cli/facts/call_result_bound_discharge), [matched_field_windows](../../../samples/cli/facts/matched_field_windows), [computed_record_window](../../../samples/cli/facts/computed_record_window), [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff), [float_range_delivery](../../../samples/cli/facts/float_range_delivery).

**Other candidate areas:** [pass/control_flow (173 roots)](../../../tests/omega/pass/control_flow), [fail/control_flow (31 roots)](../../../tests/omega/fail/control_flow), [pass/contracts (49 roots)](../../../tests/omega/pass/contracts), [fail/contracts (86 roots)](../../../tests/omega/fail/contracts), [run/contracts (3 roots)](../../../tests/omega/run/contracts), [pass/facts (204 roots)](../../../tests/omega/pass/facts), [fail/facts (219 roots)](../../../tests/omega/fail/facts).

**Required families:**

- `language-state-contracts-001` — Internal jumps carry explicit arguments and do not recover same-spelled entry or sibling bindings.
- `language-state-contracts-002` — Each state arrival simultaneously substitutes exact subjects and proves every target requires clause.
- `language-state-contracts-003` — Machine entry preconditions do not silently recur at named-state backedges.
- `language-state-contracts-004` — Every predecessor contributes to a join; minimum length does not establish exact extent. Candidate: `bad-arrival`.
- `language-state-contracts-005` — A terminal call evaluates only its selected arm once and preserves result, cleanup, crash and suspension obligations.
- `language-state-contracts-006` — Assignment captures the destination before RHS effects; later selector writes cannot redirect it.
- `language-state-contracts-007` — Value snapshots survive later writes while facts about changed referenced storage expire.
- `language-state-contracts-008` — Reference rebinding preserves old aliases and returned-reference origin needs exact evidence.
- `language-state-contracts-009` — Empty complete mutation frames preserve facts; unknown frames do not.
- `language-state-contracts-010` — Every normal value or Unit exit proves its own postcondition; crashes and jumps are not normal returns.
- `language-state-contracts-011` — Outcome guarantees require the exact selected nominal case and still-live premises.
- `language-state-contracts-012` — Proof terms requiring total denotation reject effectful, observing, crashing or nonterminating calls.
- `language-state-contracts-013` — Proof-search failure establishes neither a fact nor a refutation.

**Remaining:** Existing rows exercise selected substitutions, bounds and reference journeys. No identified row set establishes the full identity, mutation-frame, cyclic-arrival, effectful-operand, normal-exit and total-denotation obligations.

<a id="language-termination"></a>

#### termination

**Source:** [wiki/spec/language/termination.md](../../spec/language/termination.md) · 8 headings · SHA-256 `cd85ce3e49d4c8a5a5a649a9662960a89020651b19a953d51a0809cc9d8a99b4`.

**Evidence needed:** rank facts, progress provenance and native call composition.

**Fact area candidates:** [length](../../../samples/cli/facts/length), [remainder_width_index](../../../samples/cli/facts/remainder_width_index), [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff).

**Other candidate areas:** [pass/termination (136 roots)](../../../tests/omega/pass/termination), [fail/termination (91 roots)](../../../tests/omega/fail/termination), [pass/progress (1 roots)](../../../tests/omega/pass/progress).

**Required families:**

- `language-termination-001` — Termination promises an allowed eventual terminal outcome under exact premises, independently of normal-return ensures, fairness and reach.
- `language-termination-002` — Public promises remain distinct from private acyclic summaries, ranking witnesses and proof-cache identity.
- `language-termination-003` — Each cyclic state edge descends under its exact well-founded view; mutual call components require a strict edge on every complete cycle.
- `language-termination-004` — A rank-preserving recursive edge cannot import a callee contract as an induction hypothesis.
- `language-termination-005` — Rank bounds and computed subjects preserve exact captured values and cannot prove their own no-wrap premise.
- `language-termination-006` — Runtime recursive cycles are tail-position and constant-stack; productive unranked loops imply no eventual response.
- `language-termination-007` — Borrowed projected callees preserve exact referent, loan, ranking, cleanup and caller-visible writes through backedges.
- `language-termination-008` — Opaque progress profiles need owner-classified exact establishment provenance, not predicate knowledge or provider visibility.
- `language-termination-009` — Derived progress premises attach only to exact invoked subjects and must fit public schema, local receipt or build-bound admission.
- `language-termination-010` — Copies, reference loads, rebinding and finite state permutations preserve progress only through proved exact lineage.
- `language-termination-011` — Composition binds every reachable progress occurrence and receipt without rebinding issuer, subject or projection.
- `language-termination-012` — Termination supplies neither fixed work/WCET nor cross-provider temporal guarantees.

**Remaining:** Existing bound/reference rows are only supporting candidates. No complete ranking, mutual-cycle, progress-subject or composed ranked-callee row set is identified.

<a id="area-proofs"></a>

### proofs

<a id="proofs-classicality"></a>

#### classicality

**Source:** [wiki/spec/proofs/classicality.md](../../spec/proofs/classicality.md) · 8 headings · SHA-256 `2a163e3b68c8b09b945abbaada654e763660a02bcc6c6cfe044656315f495e4a`.

**Evidence needed:** kernel rule classification and trusted-surface audit.

**Fact area candidates:** [bounded_scalar_sums](../../../samples/cli/facts/bounded_scalar_sums), [dependent_subtract](../../../samples/cli/facts/dependent_subtract), [length](../../../samples/cli/facts/length).

**Other candidate areas:** [pass/proofs (115 roots)](../../../tests/omega/pass/proofs), [fail/proofs (191 roots)](../../../tests/omega/fail/proofs), [run/proofs (2 roots)](../../../tests/omega/run/proofs), [pending/proofs (1 roots)](../../../tests/omega/pending/proofs), [omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs](../../../omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs).

**Required families:**

- `proofs-classicality-001` — Every accepted certificate rule and obligation lemma has an exhaustive foundation classification.
- `proofs-classicality-002` — Decidable finite-carrier rules cannot silently generalize to undecidable classical reasoning.
- `proofs-classicality-003` — Semantic-axiom use names exact reconstructed admitted premises and never passes as an inference.
- `proofs-classicality-004` — The bounded proposition fragment and the general dependent core retain their distinct formation/elimination scopes.
- `proofs-classicality-005` — Checked recursion establishes its well-founded premise and exact edge decrease.
- `proofs-classicality-006` — All non-rule admissions, quotient assumptions and reconstructed obligation/fact rows retain exact auditable trust classification.
- `proofs-classicality-007` — Certificate-gated path facts require their own valid derivations before becoming premises.

**Remaining:** Arithmetic examples may exercise rule instances but cannot establish exhaustive foundation classification or trusted-surface closure. Kernel/audit evidence is required; the bounded fragment does not define the whole mathematical language.

<a id="proofs-contracts"></a>

#### contracts

**Source:** [wiki/spec/proofs/contracts.md](../../spec/proofs/contracts.md) · 13 headings · SHA-256 `8a25743686d0351cc5c054d220eefe972d32ecd4e11e3f31a590228a56d969a0`.

**Evidence needed:** source proof facts and trust/erasure controls.

**Fact area candidates:** [call_result_bound_discharge](../../../samples/cli/facts/call_result_bound_discharge), [case_where_equality](../../../samples/cli/facts/case_where_equality), [dependent_subtract](../../../samples/cli/facts/dependent_subtract), [bounded_scalar_sums](../../../samples/cli/facts/bounded_scalar_sums), [float_range_delivery](../../../samples/cli/facts/float_range_delivery).

**Other candidate areas:** [pass/contracts (49 roots)](../../../tests/omega/pass/contracts), [fail/contracts (86 roots)](../../../tests/omega/fail/contracts), [run/contracts (3 roots)](../../../tests/omega/run/contracts), [pass/proofs (115 roots)](../../../tests/omega/pass/proofs), [fail/proofs (191 roots)](../../../tests/omega/fail/proofs), [run/proofs (2 roots)](../../../tests/omega/run/proofs), [pending/proofs (1 roots)](../../../tests/omega/pending/proofs), [pass/relevance (17 roots)](../../../tests/omega/pass/relevance), [fail/relevance (11 roots)](../../../tests/omega/fail/relevance), [omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs](../../../omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs).

**Required families:**

- `proofs-contracts-001` — All selected requires are established before importing ensures or forming a mathematical result, even for discarded/erased/operator calls.
- `proofs-contracts-002` — Contextual equality on same nonprimitive data elaborates to proof-relevant Id while executable equality retains selected Equatable meaning.
- `proofs-contracts-003` — Named witness/law bundles retain complete telescope, exact selected rows and distinct relevant witness identities.
- `proofs-contracts-004` — Universal/existential claims accept arbitrary mathematical terms rather than enumeration of executable symbols.
- `proofs-contracts-005` — Contract terms are total per selected operation, with separate Exact representability, primitive definedness and non-self-justifying formation.
- `proofs-contracts-006` — Integer/address embed preserves exact source policy/range and is not runtime conversion or a float projection.
- `proofs-contracts-007` — Nat exact subtraction requires the operand order; saturating subtraction remains a distinct total operation.
- `proofs-contracts-008` — Recursive theorem citation establishes premises and strict edge decrease without using its own conclusions.
- `proofs-contracts-009` — Result-sensitive guarantees retain exact case, argument/result identity, live scope and mutation validity.
- `proofs-contracts-010` — Erased logical facts and erased Type permissions keep separate authority, multiplicity and provenance semantics.
- `proofs-contracts-011` — Erased fields affect semantic identity but no runtime layout/address/read/cleanup and need a valid supplied or accessible nullary term.
- `proofs-contracts-012` — Algebra normalization requires the exact selected checked operation/law map and cannot derive AC laws from punctuation or names.
- `proofs-contracts-013` — Assumption closure survives optimization and roots cannot delegate consumer grants to dependencies.
- `proofs-contracts-014` — Generic grants bind exact templates/contracts while site deferrals grant no reusable fact and cannot cross release.
- `proofs-contracts-015` — Refutable ordinary boundary claims reject grants; the exact accepted runtime assumed relationship keeps its separately reported exception.
- `proofs-contracts-016` — Provider-validation tests cannot rely on the very admitted guarantee they intend to test or establish universal admission by passing.
- `proofs-contracts-017` — Certified elaboration checks evidence rather than rerunning heuristic search; review synopsis reports the authoritative derivation.

**Remaining:** Existing scalar/case rows support narrow contract uses, not general theorem eligibility, strict/relevant equality, erased evidence, algebra licensing or trust-closure controls.

<a id="proofs-foundation"></a>

#### foundation

**Source:** [wiki/spec/proofs/foundation.md](../../spec/proofs/foundation.md) · 10 headings · SHA-256 `c039f7101ed1ae46c2cc837abb6c3fa56558ff817e92f891fd9a39a3d7aeb89b`.

**Evidence needed:** source proof facts, kernel checks and metatheory.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/proofs (115 roots)](../../../tests/omega/pass/proofs), [fail/proofs (191 roots)](../../../tests/omega/fail/proofs), [run/proofs (2 roots)](../../../tests/omega/run/proofs), [pending/proofs (1 roots)](../../../tests/omega/pending/proofs), [pass/relevance (17 roots)](../../../tests/omega/pass/relevance), [fail/relevance (11 roots)](../../../tests/omega/fail/relevance), [omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/strict_layer.rs](../../../omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/strict_layer.rs), [omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs](../../../omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs).

**Required families:**

- `proofs-foundation-001` — Every mathematical judgment retains its exact calculus/context and remains distinct from executable realization, materialization and admission.
- `proofs-foundation-002` — Strict irrelevance identifies only proofs of the same complete strict statement, never Type witnesses, ownership or provenance.
- `proofs-foundation-003` — Relevant identity supports refl/J without equality reflection, UIP/K or IEEE self-equality substitution.
- `proofs-foundation-004` — Squash elimination follows exact strict/relevant rules and cannot extract a witness without a separately admitted choice assumption.
- `proofs-foundation-005` — Universe-polymorphic functions/predicates range over arbitrary eligible mathematical terms with independently checked levels.
- `proofs-foundation-006` — Dependent substitution, pairs and W/indexed encodings preserve exact source declarations and computation correspondence.
- `proofs-foundation-007` — Measure decrease establishes termination separately from denotation correctness and emitted-loop correspondence.
- `proofs-foundation-008` — Noncomputable mathematical values may have representable carriers but cannot control or supply executable bits without checked realization.
- `proofs-foundation-009` — Exact assumption closure traverses stored statements, types and bodies and survives conversion, irrelevance and erasure.
- `proofs-foundation-010` — Unknown calculus identities reject and a cross-profile bridge requires a checked interpretation.
- `proofs-foundation-011` — Untrusted elaborators/search/tactics supply evidence against an independently established question, not success flags.
- `proofs-foundation-012` — Source-produced universe, witness, identity, induction, choice and theorem-dependent program controls have false twins.
- `proofs-foundation-013` — Bootstrap construction equations and foreign proof imports retain their separate source/calc interpretation obligations.

**Remaining:** The current bounds/loan fact folders cannot establish general mathematical formation, source elaboration, assumption closure or calculus metatheory. Small source proof fixtures plus kernel/metatheory evidence are required.

<a id="proofs-inductive-profile"></a>

#### inductive profile

**Source:** [wiki/spec/proofs/inductive_profile.md](../../spec/proofs/inductive_profile.md) · 7 headings · SHA-256 `2d981014bcc774d8e09fcb9d213d74bf45d05647a9d6c693ac69e39dccd577b3`.

**Evidence needed:** source proof controls, kernel encodings and metatheory.

**Fact area candidates:** [case_where_equality](../../../samples/cli/facts/case_where_equality).

**Other candidate areas:** [pass/proofs (115 roots)](../../../tests/omega/pass/proofs), [fail/proofs (191 roots)](../../../tests/omega/fail/proofs), [run/proofs (2 roots)](../../../tests/omega/run/proofs), [pending/proofs (1 roots)](../../../tests/omega/pending/proofs), [pass/dependent (87 roots)](../../../tests/omega/pass/dependent), [fail/dependent (46 roots)](../../../tests/omega/fail/dependent), [omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs](../../../omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs).

**Required families:**

- `proofs-inductive-profile-001` — Typed function eta is checked at one exact dependent Pi type with fresh binders and grants no extensionality or untyped rewrite.
- `proofs-inductive-profile-002` — Id, Two and W formation/elimination/computation preserve explicit levels and add no unsupported eta/reflection rules.
- `proofs-inductive-profile-003` — Strict targets use the existing Box bridge rather than new elimination exceptions.
- `proofs-inductive-profile-004` — Derived indexed families preserve the exact label/index/child relationships as relevant checked evidence.
- `proofs-inductive-profile-005` — Indexed constructor computation holds definitionally with neutral child functions using only selected pair/function eta.
- `proofs-inductive-profile-006` — Each source declaration and each encoding application independently preserve parameters, cases, recursion and dependent induction.
- `proofs-inductive-profile-007` — Relevant derivation plus squash does not supply unauthorized strict elimination or witness extraction.
- `proofs-inductive-profile-008` — Vector, context/conclusion derivation, mutual and nested-positive examples have universe, capture and negative-recursion twins.
- `proofs-inductive-profile-009` — Term/storage/conversion costs are measured separately from semantic justification and bootstrap customer costs.

**Remaining:** Constrained-case subtraction is only a source candidate, not evidence for W encodings, dependent induction or neutral constructor computation. Dedicated source-to-kernel examples and the combined metatheory remain separate requirements.

<a id="proofs-kernel-metatheory"></a>

#### kernel metatheory

**Source:** [wiki/spec/proofs/kernel_metatheory.md](../../spec/proofs/kernel_metatheory.md) · 10 headings · SHA-256 `fa4faf17a91bad38f5f187033696c6cb5d13b04a3b67441278cb5e4bccbb8e91`.

**Evidence needed:** metatheory argument and kernel implementation tests.

**Fact area candidates:** [dependent_subtract](../../../samples/cli/facts/dependent_subtract), [bounded_scalar_sums](../../../samples/cli/facts/bounded_scalar_sums), [case_where_equality](../../../samples/cli/facts/case_where_equality).

**Other candidate areas:** [pass/proofs (115 roots)](../../../tests/omega/pass/proofs), [fail/proofs (191 roots)](../../../tests/omega/fail/proofs), [run/proofs (2 roots)](../../../tests/omega/run/proofs), [pending/proofs (1 roots)](../../../tests/omega/pending/proofs), [omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/strict_layer.rs](../../../omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/strict_layer.rs), [omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs](../../../omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs).

**Required families:**

- `proofs-kernel-metatheory-001` — Context/signature prefix checking and weakening/substitution preserve well-typed terms without capture.
- `proofs-kernel-metatheory-002` — Every selected beta/delta/iota reduction preserves typing and exact dependent eliminator endpoints.
- `proofs-kernel-metatheory-003` — Combined normalization and typed eta conversion have the stated paper-level justification, not an implied mechanized proof.
- `proofs-kernel-metatheory-004` — Conversion checks a valid shared type and applies strict irrelevance only at that exact type.
- `proofs-kernel-metatheory-005` — Conversion ceilings return typed resource refusal rather than false inequality, a hang or an accepted proof.
- `proofs-kernel-metatheory-006` — Level equality normalizes exact max/successor functions without identifying distinct parameters.
- `proofs-kernel-metatheory-007` — Declarations cannot self/forward reference and assumption closure traverses stored statements and bodies rather than reduced terms.
- `proofs-kernel-metatheory-008` — Checked indexed schemes establish exact formation, constructors, neutral-child computation and index correctness.
- `proofs-kernel-metatheory-009` — Quotient interface assumptions and derived constructions retain their separate exact closure and propositional computation.
- `proofs-kernel-metatheory-010` — Bounded automation denotes untrusted terms and every fallback arithmetic/conversion law remains an explicit assumption.
- `proofs-kernel-metatheory-011` — Equality transport, affine bounds and arithmetic correspondence retain exact operand contexts, law closure and malformed-evidence twins.
- `proofs-kernel-metatheory-012` — Source declaration fidelity and operational lowering remain separate from the kernel's checked declaration-graph guarantee.

**Remaining:** Fact execution cannot discharge normalization, preservation or implementation soundness. The named kernel tests and metatheory ledger are candidates whose current paths/results must be checked independently.

<a id="proofs-mathematical-bindings"></a>

#### mathematical bindings

**Source:** [wiki/spec/proofs/mathematical_bindings.md](../../spec/proofs/mathematical_bindings.md) · 8 headings · SHA-256 `134131ecea2c24e224a9459efb8e5fdbfc1c770919108878d3797607ef450055`.

**Evidence needed:** source proof facts and logical-application fixtures.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/proofs (115 roots)](../../../tests/omega/pass/proofs), [fail/proofs (191 roots)](../../../tests/omega/fail/proofs), [run/proofs (2 roots)](../../../tests/omega/run/proofs), [pending/proofs (1 roots)](../../../tests/omega/pending/proofs), [pass/core (41 roots)](../../../tests/omega/pass/core), [fail/core (54 roots)](../../../tests/omega/fail/core), [pass/operators (27 roots)](../../../tests/omega/pass/operators), [fail/operators (13 roots)](../../../tests/omega/fail/operators), [omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs](../../../omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs).

**Required families:**

- `proofs-mathematical-bindings-001` — Universe-level binders and core Type/Strict identities retain checked levels, stable inferred interfaces and no implicit resizing.
- `proofs-mathematical-bindings-002` — Mathematical arrow types bind dependent codomains and ordered top-level typed lets elaborate to capture-avoiding terms.
- `proofs-mathematical-bindings-003` — Prefix application returns a valid remaining function term without closure allocation or omitted ordinary machine arguments.
- `proofs-mathematical-bindings-004` — Definitions name propositions/terms without proving them or demanding constant evaluation.
- `proofs-mathematical-bindings-005` — Logical machine-shaped hypotheses accept exact checked term evidence, including projected or derived evidence, independently of executable symbol supply.
- `proofs-mathematical-bindings-006` — Every logical application proves its substituted premises before result formation and recursive calls additionally prove strict descent.
- `proofs-mathematical-bindings-007` — Operational state changes, callbacks, authority transfer and receipts cannot be supplied by proof-only logical evidence.
- `proofs-mathematical-bindings-008` — Constrained witness records establish complete default domains and distinguish relevant dependent pairs from strict predicate gating.
- `proofs-mathematical-bindings-009` — Squashed existence maps through a derived universal hypothesis without allowing witness escape or invoking choice.
- `proofs-mathematical-bindings-010` — Boundary mathematical assumptions retain exact closure and no executable provider slot.
- `proofs-mathematical-bindings-011` — Choice-dependent data/control rejects materialization, while erased uses remain mathematically valid with retained trust.
- `proofs-mathematical-bindings-012` — Delivery controls preserve shadowing, arbitrary predicates, independent universes, exact function/proposition roles and source-to-kernel correspondence.

**Remaining:** No existing fact folder targets arbitrary predicate/function terms, source mathematical lets, logical evidence application, squash, universe inference or choice-dependent control.

<a id="proofs-quotients"></a>

#### quotients

**Source:** [wiki/spec/proofs/quotients.md](../../spec/proofs/quotients.md) · 11 headings · SHA-256 `a10ca57659185ea32679e8e3b348f0a1b42246de6b252261ac3c17b4d59fcfc4`.

**Evidence needed:** source proof facts, representative execution and admitted laws.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/proofs (115 roots)](../../../tests/omega/pass/proofs), [fail/proofs (191 roots)](../../../tests/omega/fail/proofs), [run/proofs (2 roots)](../../../tests/omega/run/proofs), [pending/proofs (1 roots)](../../../tests/omega/pending/proofs), [pass/domains (81 roots)](../../../tests/omega/pass/domains), [fail/domains (161 roots)](../../../tests/omega/fail/domains), [run/domains (55 roots)](../../../tests/omega/run/domains), [pending/domains (1 roots)](../../../tests/omega/pending/domains), [omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs](../../../omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs).

**Required families:**

- `proofs-quotients-001` — Quotient formation explicitly selects the exact carrier/relation, equivalence and mere-proposition evidence without implicit witness squashing.
- `proofs-quotients-002` — The universe-correct set-quotient interface retains exact project/setness/soundness/effectivity/elimination/beta assumption statements.
- `proofs-quotients-003` — Point computation is relevant identity and does not add kernel conversion or arbitrary higher elimination.
- `proofs-quotients-004` — Coverage, lift and pointwise uniqueness are checked derived constructions and supply no representative extraction or function extensionality.
- `proofs-quotients-005` — Representative results remain acceptable under quotient-refusing policy when their exact closure needs no quotient assumptions.
- `proofs-quotients-006` — Executable quotient operations use ordinary checked representative bodies and validated correspondence, not theorem-produced runtime dictionaries.
- `proofs-quotients-007` — Heterogeneous relation packs remain independent and exact; selected law implementation changes proof provenance, not relation identity.
- `proofs-quotients-008` — Quotient opacity blocks structural/reflection/raw representative observers despite equal ABI and retained representative materialization.
- `proofs-quotients-009` — Representative-independent serialization, const identity and hashing need checked canonical form; custody-bearing initial carriers reject.
- `proofs-quotients-010` — Lift congruence and forward transport use exact selected pure terminating checked theorem schemas, legal paired calls and positional ordinary arguments.
- `proofs-quotients-011` — Define preserves complete argument/result flow with biconditional preconditions while lift retains its permitted explicit adaptations.
- `proofs-quotients-012` — Role-ordered theorem collections reject missing, duplicate, surplus or mismatched evidence.
- `proofs-quotients-013` — Executable equality proves both directions of DecidesEquivalence, not only representative congruence.
- `proofs-quotients-014` — Dependent constructor lifting relates earlier witnesses before later fields and erased relevant components still require transport.

**Remaining:** No existing fact row establishes quotient opacity, exact laws, rejecting assumption policy, forward transport, source/runtime correspondence or executable equality completeness.

<a id="area-resources"></a>

### resources

<a id="resources-activation-storage"></a>

#### activation storage

**Source:** [wiki/spec/resources/activation_storage.md](../../spec/resources/activation_storage.md) · 4 headings · SHA-256 `32fe547ebb664869bbc39c328ac497b1a0aed98f0794589b7c7427086b85ae83`.

**Evidence needed:** activation-local claim facts and native stack replay.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/storage (14 roots)](../../../tests/omega/pass/storage), [pass/calls (289 roots)](../../../tests/omega/pass/calls), [fail/calls (117 roots)](../../../tests/omega/fail/calls), [run/calls (28 roots)](../../../tests/omega/run/calls), [pass/ownership (140 roots)](../../../tests/omega/pass/ownership), [fail/ownership (105 roots)](../../../tests/omega/fail/ownership), [run/ownership (7 roots)](../../../tests/omega/run/ownership).

**Required families:**

- `resources-activation-storage-001` — Runtime-sized activation storage uses an explicit linear bounded extent claim and no implicit dynamic local or value-index allocation.
- `resources-activation-storage-002` — Closed demand charges full simultaneous bounds with alignment, and exclusive alternatives take maximum rather than committed-size guesses.
- `resources-activation-storage-003` — Committed extent exceeding the bound returns ordinary admission failure without trap or clamp.
- `resources-activation-storage-004` — Claims cannot escape their activation, release in reverse establishment order and retain stable addresses/loans across suspension.
- `resources-activation-storage-005` — Each source claim site bijects with published bound/provenance/commit/release/live-suspension rows.
- `resources-activation-storage-006` — Replay proves committed extents fit both site bound and actual frame/probe provision, rejecting missing/invented/duplicate rows.

**Remaining:** No dedicated existing fact row covers bounded runtime stack claims, escape/release order, suspension retention or site-to-plan replay.

<a id="resources-allocation"></a>

#### allocation

**Source:** [wiki/spec/resources/allocation.md](../../spec/resources/allocation.md) · 3 headings · SHA-256 `9f1c8e1e82a44db0619c9139164b3cbfda194ec6b6573b2c1f006ce98f0026a6`.

**Evidence needed:** small allocator programs and source conservation facts.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/collections (131 roots)](../../../tests/omega/pass/collections), [fail/collections (44 roots)](../../../tests/omega/fail/collections), [pass/memory (10 roots)](../../../tests/omega/pass/memory), [fail/memory (9 roots)](../../../tests/omega/fail/memory).

**Required families:**

- `resources-allocation-001` — Allocation uses caller-supplied qualified backing and cannot turn a numeric capacity into placement or ordinary RAM access.
- `resources-allocation-002` — Fallible allocation preserves rejected state while infallible allocation needs exact aligned capacity proof.
- `resources-allocation-003` — Monotonic allocation ends its strategy borrow at return so multiple owned allocations coexist.
- `resources-allocation-004` — Cleanup/extent return makes retired content, not cursor rewind or immediately reusable capacity.
- `resources-allocation-005` — Reset/finish recomposes all tail/retired content and rejects while any live allocation remains.
- `resources-allocation-006` — Fragmented free-byte totals require explicit contiguous placement/reservation evidence and cannot assume bump semantics.

**Remaining:** No existing fact row establishes coexisting allocation, live-reset rejection, cleanup/recomposition and exact backing return.

<a id="resources-authority"></a>

#### authority

**Source:** [wiki/spec/resources/authority.md](../../spec/resources/authority.md) · 8 headings · SHA-256 `017c4cd397156d46a12ea71bdde67baca5a35447f8fce89dcb5b7792be02c897`.

**Evidence needed:** source provenance facts and admitted crossing evidence.

**Fact area candidates:** [returned_write_destination](../../../samples/cli/facts/returned_write_destination), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan).

**Other candidate areas:** [pass/capabilities (13 roots)](../../../tests/omega/pass/capabilities), [fail/capabilities (9 roots)](../../../tests/omega/fail/capabilities), [pass/domains (81 roots)](../../../tests/omega/pass/domains), [fail/domains (161 roots)](../../../tests/omega/fail/domains), [run/domains (55 roots)](../../../tests/omega/run/domains), [pending/domains (1 roots)](../../../tests/omega/pending/domains), [pass/memory (10 roots)](../../../tests/omega/pass/memory), [fail/memory (9 roots)](../../../tests/omega/fail/memory).

**Required families:**

- `resources-authority-001` — Provider selection and equal carrier fields cannot create authority or discharge linear custody.
- `resources-authority-002` — Completion accepts transferred exact domain provenance outside its issuer without inventing a new authority type.
- `resources-authority-003` — Establishment routes retain exact requirement-versus-machine identities, independent visibility and one domain-owned issuer catalog.
- `resources-authority-004` — Authorized checked result introduction still proves carrier/predicate/custody validity without assuming its own qualification.
- `resources-authority-005` — Owned result/case-payload issuance binds the exact authorized carrier and selected occurrence; existing input content is transferred rather than minted.
- `resources-authority-006` — Private issuer metadata stays verifier-visible without granting consumer selection or conformance authority.
- `resources-authority-007` — Ordinary parameter qualification is a precondition; only the exact installed crossing or accepted assumed relation introduces arrival provenance.
- `resources-authority-008` — Fresh issuance separately proves per-invocation geometry, separation from live grants and shared cross-provider custody roots.
- `resources-authority-009` — Installed finite parameter introductions are reconstructed from authorized slots, not constructors, ordinary calls or producer totals.
- `resources-authority-010` — Reports retain exact subject/route/provider/receipt identity and compact fingerprints grant no authority.

**Remaining:** Loan transport is not issuance/completion provenance. Dedicated exact-route, wrong-carrier, direct-adapter, private-issuer, replay and finite-root controls are needed.

<a id="resources-bounded-growth"></a>

#### bounded growth

**Source:** [wiki/spec/resources/bounded_growth.md](../../spec/resources/bounded_growth.md) · 4 headings · SHA-256 `5203b4505bc0b89cf87e95bb3ff94cb5edba47a79a8f9bf26783b3bd6369bc39`.

**Evidence needed:** source length facts and small container witnesses.

**Fact area candidates:** [length](../../../samples/cli/facts/length), [range_fact_delivery](../../../samples/cli/facts/range_fact_delivery), [dependent_subtract](../../../samples/cli/facts/dependent_subtract), [remainder_width_index](../../../samples/cli/facts/remainder_width_index).

**Other candidate areas:** [pass/collections (131 roots)](../../../tests/omega/pass/collections), [fail/collections (44 roots)](../../../tests/omega/fail/collections), [pass/text (74 roots)](../../../tests/omega/pass/text), [fail/text (22 roots)](../../../tests/omega/fail/text).

**Required families:**

- `resources-bounded-growth-001` — Every length-changing operation proves resulting live length/capacity and initialized-prefix validity without guessed defaults or truncation.
- `resources-bounded-growth-002` — Bounded-input prefixes preserve unread input separately from owner growth obligations.
- `resources-bounded-growth-003` — Finite append cycles retain relational progress/length summaries and exact selected length laws.
- `resources-bounded-growth-004` — Unconstrained nonlinear growth cannot borrow a finite bound from independent intervals or prover failure.
- `resources-bounded-growth-005` — Variable-fill carriers retain exact capacity/live length through calls/results/payloads and never expose spare storage as a live slice.
- `resources-bounded-growth-006` — A fixed-storage proof neither invents backing nor silently selects an ambient allocator or hides reachable allocation reach.
- `resources-bounded-growth-007` — Local fit establishes neither global peak, fragmentation, prompt reuse nor allocator selection.

**Remaining:** Existing length/index rows do not establish growing-owner initialization, nonlinear/relational loop bounds or preserved failure semantics.

<a id="resources-bounded-input"></a>

#### bounded input

**Source:** [wiki/spec/resources/bounded_input.md](../../spec/resources/bounded_input.md) · 6 headings · SHA-256 `357aed8fe072bd79a75a36c21a6675533d59c52f757a592cd756e6c21d61cfae`.

**Evidence needed:** small input-contract facts and host/provider execution.

**Fact area candidates:** [guarded_line_loan](../../../samples/cli/facts/guarded_line_loan), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan).

**Other candidate areas:** [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host), [pass/text (74 roots)](../../../tests/omega/pass/text), [fail/text (22 roots)](../../../tests/omega/fail/text).

**Required families:**

- `resources-bounded-input-001` — Read fills only the exact caller slice prefix, preserves its tail and changes no owner allocation/live-length metadata.
- `resources-bounded-input-002` — Only actual normal read outcomes establish exact invocation-bound count and initialized prefix; Invalid is zero absence, not success/error/EOF.
- `resources-bounded-input-003` — Zero capacity returns Full(0) without input; LF is stored and wins even in the last slot.
- `resources-bounded-input-004` — EOF and Full preserve distinct stopping reasons and Full performs no extra read or discard of remaining input.
- `resources-bounded-input-005` — Raw bytes preserve NUL/CR/LF and no encoding/transcoding/terminator/truncation is implicit.
- `resources-bounded-input-006` — Short host reads are not EOF and buffered readahead retains exact stream custody across later calls and pauses.
- `resources-bounded-input-007` — Read failures obey the selected byte leaf's actual crash/outcome contract rather than masquerading as Invalid or rollback.
- `resources-bounded-input-008` — UTF-8 validation applies only to the exact completed prefix and result tags do not supply encoding facts.

**Remaining:** The line-loan rows exercise supplied outcomes and loan transport, not the byte provider's greedy stopping, stream consumption, short-read behavior or encoding validation.

<a id="resources-carry"></a>

#### carry

**Source:** [wiki/spec/resources/carry.md](../../spec/resources/carry.md) · 3 headings · SHA-256 `c6644500921612e4b1ed002d5f1e309b554027360d139e41b41b90b98551daed`.

**Evidence needed:** source liveness/carry facts and runtime preservation.

**Fact area candidates:** [guarded_line_loan](../../../samples/cli/facts/guarded_line_loan), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan), [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff).

**Other candidate areas:** [pass/concurrency (13 roots)](../../../tests/omega/pass/concurrency), [fail/concurrency (7 roots)](../../../tests/omega/fail/concurrency), [pass/borrows (287 roots)](../../../tests/omega/pass/borrows), [fail/borrows (286 roots)](../../../tests/omega/fail/borrows), [run/borrows (30 roots)](../../../tests/omega/run/borrows), [pending/borrows (1 roots)](../../../tests/omega/pending/borrows).

**Required families:**

- `resources-carry-001` — Carry properties specify all four closed axes and compose the most restrictive live structural demands.
- `resources-carry-002` — Positive permissions retain subject provenance and cannot erase an undischarged claim through casts or forgetting.
- `resources-carry-003` — Conserved children inherit permissions and a wider successor requires actual discharge/new establishment.
- `resources-carry-004` — Shared cross-activation access separately requires a sanctioned sharing contract, not copyability.
- `resources-carry-005` — Suspending calls preserve exact loan, backing, parent restrictions, invariant debt and resource demands until legal completion/cancellation.
- `resources-carry-006` — Fixed nonmoving activation storage and selected runtime CPU/thread preservation satisfy live carry requirements.
- `resources-carry-007` — Architectural preemption is distinct from semantic suspension and cannot bypass activation-wide affinity.
- `resources-carry-008` — Interrupt masking and scheduler-switch suppression remain separate linear tokens and neither rewrites public effect ceilings.

**Remaining:** Existing same-activation loan rows do not establish suspension/CPU/thread/address crossings or runtime affinity admission.

<a id="resources-content-custody"></a>

#### content custody

**Source:** [wiki/spec/resources/content_custody.md](../../spec/resources/content_custody.md) · 9 headings · SHA-256 `8782b2d235d684e9adf14993034c7dea8e002b47c21caf5684c55e3dcbba80e5`.

**Evidence needed:** source conservation facts and installed lifecycle evidence.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/core (41 roots)](../../../tests/omega/pass/core), [fail/core (54 roots)](../../../tests/omega/fail/core), [pass/domains (81 roots)](../../../tests/omega/pass/domains), [fail/domains (161 roots)](../../../tests/omega/fail/domains), [run/domains (55 roots)](../../../tests/omega/run/domains), [pending/domains (1 roots)](../../../tests/omega/pending/domains), [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers).

**Required families:**

- `resources-content-custody-001` — Each exact qualification has one owner-unique closed-fragment content projection with exact algebra/unit/coordinate identity.
- `resources-content-custody-002` — Interval separation/residual and counted quantities retain their distinct canonical algebra laws.
- `resources-content-custody-003` — Callable-entry old(place) retains exact revision without copying owned values or introducing unrestricted history.
- `resources-content-custody-004` — Every frontier proves n-to-m separated conservation, not only child containment or equal scalar sums.
- `resources-content-custody-005` — Claim occurrences, paths, lineage, carry and content remain independently tracked through construction/moves/outcomes.
- `resources-content-custody-006` — Call composition binds exact substitution, authored conservation guarantee and returned claims, available only after successful completion.
- `resources-content-custody-007` — Partial boundary residual arithmetic is derived separately from admitted custody exit and no exit implies reclamation/reissue.
- `resources-content-custody-008` — Related resources require joint correspondence rather than independent conserved totals.
- `resources-content-custody-009` — Installed introductions reconstruct exact authorized finite parameter capacity and parent/lifecycle/artifact lineage.
- `resources-content-custody-010` — Complete cohort sealing is transactional and coexistence charges every live era without merging unlike roots.
- `resources-content-custody-011` — One nonclonable installation ledger and one epoch runtime establish exact dormant-member accounts without reusable mint grants.
- `resources-content-custody-012` — Every claim-local introduction/forwarding/transformation/exit/loan role retains its own proof and authorized subject.

**Remaining:** No dedicated fact row currently establishes separated content conservation, exact residual boundaries, installed introductions or cross-era ledger accounting.

<a id="resources-device-access"></a>

#### device access

**Source:** [wiki/spec/resources/device_access.md](../../spec/resources/device_access.md) · 4 headings · SHA-256 `007848856cfedf0615222a90a88a1159a3a2907da4b2b07ee92fef7ef81373d8`.

**Evidence needed:** device protocol facts and admitted hardware correspondence.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/asm (19 roots)](../../../tests/omega/pass/asm), [fail/asm (55 roots)](../../../tests/omega/fail/asm), [pass/atomics (21 roots)](../../../tests/omega/pass/atomics), [fail/atomics (22 roots)](../../../tests/omega/fail/atomics), [pass/memory (10 roots)](../../../tests/omega/pass/memory), [fail/memory (9 roots)](../../../tests/omega/fail/memory).

**Required families:**

- `resources-device-access-001` — External device reads/writes create direction-specific CPU exclusion under an exact confined range/era/rights receipt.
- `resources-device-access-002` — Completion restores CPU custody only after exact loan/borrower release and ordering obligations, independently of device success status.
- `resources-device-access-003` — Failed start/completion retains the appropriate original borrow or pending token for recovery.
- `resources-device-access-004` — Publication, cache maintenance, notification, completion and acquisition have distinct scoped role identities.
- `resources-device-access-005` — Writes invalidate publication evidence before notification and erased facts alone do not order emitted instructions.
- `resources-device-access-006` — Runtime device/session occurrences come from selected installed providers, not constructible source IDs.
- `resources-device-access-007` — CPU fences alone do not prove posted-write observation, DMA completion or target coherence.
- `resources-device-access-008` — Hostile shared-memory peers need copy-and-validate or actual write revocation/quiescence before zero-copy validation.

**Remaining:** No existing fact folder exercises DMA confinement, scoped ordering roles, completion custody or hostile-peer validation.

<a id="resources-entry-stacks"></a>

#### entry stacks

**Source:** [wiki/spec/resources/entry_stacks.md](../../spec/resources/entry_stacks.md) · 4 headings · SHA-256 `9e8ca256d45e9859fd0bce2ead6cdb8ed5feac7f378baad4660571e4208727fc`.

**Evidence needed:** external-entry physical composition.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/calling (1 roots)](../../../tests/omega/pass/calling), [fail/calling (2 roots)](../../../tests/omega/fail/calling), [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host), [pass/entry (2 roots)](../../../tests/omega/pass/entry), [fail/entry (1 roots)](../../../tests/omega/fail/entry).

**Required families:**

- `resources-entry-stacks-001` — Every admitted arrival context retains a complete ordered Enter/Body/Exit epoch roster with exact active/occupied domains.
- `resources-entry-stacks-002` — Live domain occupancy and nested demand add with alignment while alternative contexts/epochs take maxima.
- `resources-entry-stacks-003` — Nesting depth includes the current occurrence and every unresolved/unbounded context prevents a bounded theorem.
- `resources-entry-stacks-004` — Architectural and generated adapter evidence derives from installed facts/final instructions while opaque adapters retain exact complete admitted rosters.
- `resources-entry-stacks-005` — Context-indexed stack disposition agrees with every target arrival and cannot be substituted by equal numeric totals.
- `resources-entry-stacks-006` — Callback entry fits exact WCSU and margins through its selected containment/preflight/owned-stack contract, independently of backing permission.

**Remaining:** No fact row establishes external arrival epochs, nesting closure or target callback-stack admission; final-stack artifacts and malformed-roster controls are required.

<a id="resources-extents"></a>

#### extents

**Source:** [wiki/spec/resources/extents.md](../../spec/resources/extents.md) · 4 headings · SHA-256 `e0b231554f674c70dcff8df56434054db3ef64a6da3d9cad06c11b5f67fbd758`.

**Evidence needed:** source geometry/authority facts and provider/mapping integration.

**Fact area candidates:** [length](../../../samples/cli/facts/length), [disjoint_windows](../../../samples/cli/facts/disjoint_windows), [remainder_width_index](../../../samples/cli/facts/remainder_width_index).

**Other candidate areas:** [pass/memory (10 roots)](../../../tests/omega/pass/memory), [fail/memory (9 roots)](../../../tests/omega/fail/memory), [pass/core (41 roots)](../../../tests/omega/pass/core), [fail/core (54 roots)](../../../tests/omega/fail/core), [pass/domains (81 roots)](../../../tests/omega/pass/domains), [fail/domains (161 roots)](../../../tests/omega/fail/domains), [run/domains (55 roots)](../../../tests/omega/run/domains), [pending/domains (1 roots)](../../../tests/omega/pending/domains).

**Required families:**

- `resources-extents-001` — Extent fields and no-wrap proof establish geometry but not Granted provenance or access rights.
- `resources-extents-002` — One-past proof endpoints may equal the exclusive address-space bound without fitting addr.
- `resources-extents-003` — Fresh grants retain stable backing, exact address space, rights, issuance and shared custody roots.
- `resources-extents-004` — Split/merge proves exact separated content with compatible lineage/rights/era and returns every authority on failure.
- `resources-extents-005` — Attenuation cannot regain discarded rights and subrange loans do not implicitly split owned authority.
- `resources-extents-006` — Succession consumes old issuance authority while retaining backing history and classified live delegations.
- `resources-extents-007` — Pending mappings need independent source/destination authority and installed-translation activation before access.
- `resources-extents-008` — Unmapping/reseat retains custody until stale translations and foreign pins are released under an exact completion receipt.
- `resources-extents-009` — Asynchronous revocation and virtual/physical decomposition require their separate quiescence and joint-correspondence contracts.

**Remaining:** Ordinary slice geometry is only a supporting analogy; it does not establish Granted extents, fresh issuance, mapping activation or custody succession.

<a id="resources-logical-work"></a>

#### logical work

**Source:** [wiki/spec/resources/logical_work.md](../../spec/resources/logical_work.md) · 6 headings · SHA-256 `497d1762b08e2bc2d3aadf1ff663210aee479e4a47eaaef7ef0e88aac29782b6`.

**Evidence needed:** resource certificates and interpreter/native integration.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/control_flow (173 roots)](../../../tests/omega/pass/control_flow), [fail/control_flow (31 roots)](../../../tests/omega/fail/control_flow), [pass/termination (136 roots)](../../../tests/omega/pass/termination), [fail/termination (91 roots)](../../../tests/omega/fail/termination).

**Required families:**

- `resources-logical-work-001` — Canonical logical charges are versioned independently of meaning and depend on semantic sites rather than host time.
- `resources-logical-work-002` — Fuel charges before sites, cannot be observed/caught by evaluated code and creates no native meter or source suspension.
- `resources-logical-work-003` — Sequential work adds, exclusive outcomes take maxima and crash paths exclude unreachable continuations.
- `resources-logical-work-004` — Cleanup work repeats on every actual backedge and bounded iteration needs quantitative evidence beyond ranking.
- `resources-logical-work-005` — Whole-entry, segment and safe-point certificates bind distinct exact subjects and cannot substitute for one another.
- `resources-logical-work-006` — Provider summaries and installed correspondence retain exact schedule, code occurrence, entry and receipts.
- `resources-logical-work-007` — Unknown and no-finite-guarantee verdicts retain exact causes without becoming execution outcomes.
- `resources-logical-work-008` — Response/WCET needs separate waiting, physical-timing and target-path evidence.

**Remaining:** Runtime exit tests cannot establish canonical work accounting, certificate scope or physical response bounds. Dedicated meter/certificate/provider controls are required.

<a id="resources-placed-access"></a>

#### placed access

**Source:** [wiki/spec/resources/placed_access.md](../../spec/resources/placed_access.md) · 8 headings · SHA-256 `58dc1fd7918a87ae06a910e469934cf07f41dfea1d7b2e26ec5e076bd7e2da0a`.

**Evidence needed:** source placed-access facts and provider/target integration.

**Fact area candidates:** [returned_write_destination](../../../samples/cli/facts/returned_write_destination), [indexed_element_mut_loan](../../../samples/cli/facts/indexed_element_mut_loan).

**Other candidate areas:** [pass/layouts (90 roots)](../../../tests/omega/pass/layouts), [fail/layouts (57 roots)](../../../tests/omega/fail/layouts), [run/layouts (7 roots)](../../../tests/omega/run/layouts), [pass/asm (19 roots)](../../../tests/omega/pass/asm), [fail/asm (55 roots)](../../../tests/omega/fail/asm).

**Required families:**

- `resources-placed-access-001` — Every runtime field has one exact validated layout/access decision; erased fields have no physical field key.
- `resources-placed-access-002` — Admitted supply binds immutable normalized profile, range, rights and provenance and subranges only attenuate.
- `resources-placed-access-003` — Compatibility proves exact physical footprint, absolute alignment residue, operation/reach subsets and observation mode.
- `resources-placed-access-004` — Total decoding/encoding and whole-container External transfer cannot be replaced by silent narrowing, hidden RMW or arbitrary retry loops.
- `resources-placed-access-005` — View, initialize and validate retain distinct custody/establishment outcomes and exact static validator contracts.
- `resources-placed-access-006` — Resident/Vacant are mutually exclusive protected custody facts with one authorized route set and no implicit protection from linearity.
- `resources-placed-access-007` — Non-runtime Type fields retain exact authored custody correspondence and every moved input is returned, embedded or consumed.
- `resources-placed-access-008` — Owned/borrowed retirement retains exact dormant claims or releases only the exact loan; partial moves restore/dispose all residuals.
- `resources-placed-access-009` — Projected events retain exact occurrence, field, receipt, displacement, footprint, lifetime, revision and custody without backend name/layout re-resolution.
- `resources-placed-access-010` — Stable read/write/take/swap and External operations enforce independent ownership and physical-container conflicts.
- `resources-placed-access-011` — Atomic operations need exact admitted widths/encoding/orderings; exclusive loans create no Atomic permission.
- `resources-placed-access-012` — Observing versus nonobserving and decisive versus single-attempt exchange retain canonical result shapes and exact displaced/proposed custody.
- `resources-placed-access-013` — Generic resident-transfer schemas instantiate every actual closed application; a provider family claim supplies no missing realization.

**Remaining:** Ordinary RAM loans do not qualify placed/profile/resident/atomic operations. Dedicated plan, wrong-receipt, partial-resident, rejection-custody and physical-footprint evidence is needed.

<a id="resources-storage"></a>

#### storage

**Source:** [wiki/spec/resources/storage.md](../../spec/resources/storage.md) · 6 headings · SHA-256 `ecc29108ff35fdd8db9703106c270de89fae6345e9124325a70bf82b0b6b2f6e`.

**Evidence needed:** physical stack composition and provisioning evidence.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/storage (14 roots)](../../../tests/omega/pass/storage), [pass/memory (10 roots)](../../../tests/omega/pass/memory), [fail/memory (9 roots)](../../../tests/omega/fail/memory), [pass/calls (289 roots)](../../../tests/omega/pass/calls), [fail/calls (117 roots)](../../../tests/omega/fail/calls), [run/calls (28 roots)](../../../tests/omega/run/calls).

**Required families:**

- `resources-storage-001` — Storage uses concrete extents, allocators, stack leases and installed image authority rather than an ambient byte meter.
- `resources-storage-002` — WCSU binds final physical frames, spills, alignment and exact call/return/cleanup paths for one realization.
- `resources-storage-003` — Sequential and alternative demands take maxima while nested calls include their caller-live frontier.
- `resources-storage-004` — Opaque leaves contribute exact admitted caller-live bytes and retain plan/requirement/receipt identity.
- `resources-storage-005` — Runtime-sized claims charge their full live bounds and entry/body/adapter epochs retain separate domains.
- `resources-storage-006` — Provisioned backing independently proves access, alignment, lifetime and closed call-chain demand before execution.
- `resources-storage-007` — Compiler spill faults under established backing are defects, not new source traps or per-call exhaustion.
- `resources-storage-008` — Installed local content roots have finite exact occurrences and ordinary reinvocation cannot reset capacity.
- `resources-storage-009` — Capacity arithmetic does not establish contiguous placement, fragmentation recovery or recomposed backing.

**Remaining:** Source value facts do not establish final physical frames or installed backing. Use matching-host stack/provisioning and malformed-certificate tests.

<a id="area-layouts"></a>

### layouts

<a id="layouts-codecs"></a>

#### codecs

**Source:** [wiki/spec/layouts/codecs.md](../../spec/layouts/codecs.md) · 8 headings · SHA-256 `0e3a304eb20c2e9474370a4febea73fbfdf81012c833d9167026031a50abc81e`.

**Evidence needed:** Codec semantic proofs, parser controls, custody and history fixtures.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/wire (20 roots)](../../../tests/omega/pass/wire), [fail/wire (11 roots)](../../../tests/omega/fail/wire), [pass/versioning (2 roots)](../../../tests/omega/pass/versioning).

**Required families:**

- `layouts-codecs-001` — Authored and generated codecs remain checked ordinary programs; Generated, Derived and compiler-admitted have distinct evidence obligations.
- `layouts-codecs-002` — Current-shape round trip and immutable historical lineage are separate obligations; differential examples are not universal agreement.
- `layouts-codecs-003` — Inbound validation establishes predicates but never authority; outbound encoding is distinct from representation recast.
- `layouts-codecs-004` — Destination policy is explicit with no ambient conformance search.
- `layouts-codecs-005` — Missing required fields reject; Optional omission, home-zero, fixed/live/growable storage have separate rules.
- `layouts-codecs-006` — Strict, Projecting and Preserving decode retain their distinct guarantees.
- `layouts-codecs-007` — Relayed unknown remainder preserves exact codec, ordering and custody; allocation and loans stay explicit.
- `layouts-codecs-008` — Migrations select exact immutable historical shapes; wildcard matching does not prove completeness.
- `layouts-codecs-009` — Compact binary encodings enforce canonical ULEB, zigzag, booleans, lengths, and reject malformed, truncated and noncanonical inputs. Candidate: `wire-canonical-varint`.

**Remaining:** No dedicated codec fact row identified; parser examples and quantified codec laws require different evidence.

<a id="layouts-plans"></a>

#### plans

**Source:** [wiki/spec/layouts/plans.md](../../spec/layouts/plans.md) · 7 headings · SHA-256 `91b5bbf85b86ca37270e764cf8b73c03226e9510f750bc231bf971c486005d1d`.

**Evidence needed:** Layout validation, source semantics, ABI and mutation controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/layouts (90 roots)](../../../tests/omega/pass/layouts), [fail/layouts (57 roots)](../../../tests/omega/fail/layouts), [run/layouts (7 roots)](../../../tests/omega/run/layouts).

**Required families:**

- `layouts-plans-001` — Validate an ordinary build-admissible policy plan against layout Schema before any consumer uses it; semantic TypeSchema is distinct.
- `layouts-plans-002` — Stable member numbers are all-or-nothing, unique u64, disjoint from retired numbers, and include erased members semantically.
- `layouts-plans-003` — Geometry preserves all u64 bits and opaque field keys; positions or compact IDs do not replace field identity.
- `layouts-plans-004` — size_of and align_of expose only exact validated target geometry; no dynamic/proof-only fake extent or circular self-layout.
- `layouts-plans-005` — Closed placement forms, fragment tiling, widths, overlap/overlay agreement, dynamic bounds and private slots must validate exactly.
- `layouts-plans-006` — Plan identity does not change with prover strength; every consumer and by-value ABI uses the same physical geometry.
- `layouts-plans-007` — Fit checks precede mutation; erased semantic custody and layout do not manufacture storage authority.

**Remaining:** No dedicated layout-plan fact row identified; target/layout validation and ABI fixtures need separate qualification.

<a id="layouts-recasts"></a>

#### recasts

**Source:** [wiki/spec/layouts/recasts.md](../../spec/layouts/recasts.md) · 3 headings · SHA-256 `2e65f54629c9c9ba874f21455a5f70fb45f2ff9dcd91928994e849ef7acbbd24`.

**Evidence needed:** Representation and loan controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/recast (86 roots)](../../../tests/omega/pass/recast), [fail/recast (49 roots)](../../../tests/omega/fail/recast).

**Required families:**

- `layouts-recasts-001` — A recast preserves storage, backing, lifetime and provenance.
- `layouts-recasts-002` — Shared S-to-T recasts require S implies T; mutable recasts require both directions. Candidate: `recast-implication`.
- `layouts-recasts-003` — Equal byte size or raw bytes alone cannot establish stronger predicates. Candidate: `recast-implication`.
- `layouts-recasts-004` — Unsized recasts require exact tiling and alignment.
- `layouts-recasts-005` — Placed accessors/views reject representation recasts; detached snapshots follow ordinary rules.

**Remaining:** No dedicated recast row identified, including shared versus mutable implication twins.

<a id="area-terminal-psi"></a>

### terminal-psi

<a id="terminal-psi-boundary-calls"></a>

#### boundary calls

**Source:** [wiki/spec/terminal-psi/boundary_calls.md](../../spec/terminal-psi/boundary_calls.md) · 12 headings · SHA-256 `d1b63eeab1738938faa6790a805bf193f461007d3d35392ea07574dd0f472b01`.

**Evidence needed:** Boundary contracts, provider admission and physical settlement controls.

**Fact area candidates:** [guarded_line_loan](../../../samples/cli/facts/guarded_line_loan), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan).

**Other candidate areas:** [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers), [pass/calls (289 roots)](../../../tests/omega/pass/calls), [fail/calls (117 roots)](../../../tests/omega/fail/calls), [run/calls (28 roots)](../../../tests/omega/run/calls), [pass/calling (1 roots)](../../../tests/omega/pass/calling), [fail/calling (2 roots)](../../../tests/omega/fail/calling), [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host).

**Required families:**

- `terminal-psi-boundary-calls-001` — Retain exact requirement identity and scalar/structural runtime interleave, independently of provider choice and lane-local contract positions.
- `terminal-psi-boundary-calls-002` — Borrowed views retain original caller storage; bounded input does not resize or replace live length through a slice.
- `terminal-psi-boundary-calls-003` — Trait requirements retain owning service and parent closure; retired standalone identities do not synthesize authority.
- `terminal-psi-boundary-calls-004` — Crash routes substitute exact scalar actuals in mathematical semantics; unsupported evaluation cannot authorize crash or count as false.
- `terminal-psi-boundary-calls-005` — Structural membership requirements carry exact qualification identities rather than trivially true proof rows.
- `terminal-psi-boundary-calls-006` — Scalar requires are discharged before the call; provider requires must follow declaration rows and guarantees cannot silently strengthen caller assumptions.
- `terminal-psi-boundary-calls-007` — Checked providers join exact requirement, implementation, adapter, artifact and signature, with path-exact result custody only after successful completion.
- `terminal-psi-boundary-calls-008` — Canonical ProcessExit preserves terminal identity, argument/domain and progress contracts; returning/divergent/aborting bodies are not substitutes.
- `terminal-psi-boundary-calls-009` — Assumed bindings retain full assumptions and exact runtime compatibility, differ from proved conformance, and reject during Build/automatic tests.
- `terminal-psi-boundary-calls-010` — Host result shape validation precedes custody changes and cannot establish external allocation freshness or roll back host effects.
- `terminal-psi-boundary-calls-011` — Fixed reach and installation bounds stay distinct; complete root closure rejects missing, padded, stale or unresolved rows.
- `terminal-psi-boundary-calls-012` — Explicit contexts require actual initialized provisioned storage; names, zero layout and requirement attachment grant no storage.
- `terminal-psi-boundary-calls-013` — Target-selected provider demands bind exact call occurrence, closed application, target slot/schema and plan with complete ordered settlement.
- `terminal-psi-boundary-calls-014` — Builtin conversion is one exhaustive checked mapping; commitments alone do not reconstruct retained admitted execution content.
- `terminal-psi-boundary-calls-015` — Specialization reaches a fixed point over immutable templates; complete tuples, exact owner reviews and selected plans remain separate from generic coverage.
- `terminal-psi-boundary-calls-016` — Every surviving boundary occurrence has exactly one role-tagged physical child with complete spans, relocations and final-symbol identity; checked eliminated occurrences need none.

**Remaining:** Line-input rows are candidate semantic witnesses, not evidence of complete provider, cross-artifact or physical settlement admission.

<a id="terminal-psi-byte-views"></a>

#### byte views

**Source:** [wiki/spec/terminal-psi/byte_views.md](../../spec/terminal-psi/byte_views.md) · 10 headings · SHA-256 `46d9e651457d4d2fb4193be509b9ad56a985645f4d0cb74716f8e36987edbb24`.

**Evidence needed:** View bounds, identity, mutation, suspension and native controls.

**Fact area candidates:** [length](../../../samples/cli/facts/length), [as_slice_views](../../../samples/cli/facts/as_slice_views), [disjoint_windows](../../../samples/cli/facts/disjoint_windows), [matched_field_windows](../../../samples/cli/facts/matched_field_windows), [computed_record_window](../../../samples/cli/facts/computed_record_window), [remainder_width_index](../../../samples/cli/facts/remainder_width_index), [index_every_width](../../../samples/cli/facts/index_every_width).

**Other candidate areas:** [pass/slices (176 roots)](../../../tests/omega/pass/slices), [fail/slices (130 roots)](../../../tests/omega/fail/slices), [run/slices (10 roots)](../../../tests/omega/run/slices), [pass/collections (131 roots)](../../../tests/omega/pass/collections), [fail/collections (44 roots)](../../../tests/omega/fail/collections), [pass/text (74 roots)](../../../tests/omega/pass/text), [fail/text (22 roots)](../../../tests/omega/fail/text).

**Required families:**

- `terminal-psi-byte-views-001` — Exact octets, including empty/non-UTF-8 literals, require a unique dominating establishment on every execution.
- `terminal-psi-byte-views-002` — Read/write bounds use a dominating length of the identical descriptor, path and access; equal-valued or sibling counts do not substitute.
- `terminal-psi-byte-views-003` — Subslice checks start <= end <= length even for empty windows, preserves backing and access, and cannot gain mutability from a receiving annotation.
- `terminal-psi-byte-views-004` — Subslice predicates preserve ASCII/no-NUL, but not arbitrary UTF-8 or nonempty; derive them from exact current producer chains.
- `terminal-psi-byte-views-005` — Returned views reconstruct every ingress/literal origin and reject local backing escape, unknown origins and cyclic justification.
- `terminal-psi-byte-views-006` — Literal length equations arise at observation; subslice length is exactly captured end minus start, without implying rank descent.
- `terminal-psi-byte-views-007` — Evaluated content predicates expire after overlapping writes; extent observations survive only extent-preserving writes.
- `terminal-psi-byte-views-008` — Fixed-array call windows retain original initialized backing, exact bounds and conservative overlap; no resize or owned cleanup is granted.
- `terminal-psi-byte-views-009` — State parameters bind exact incoming descriptors on every arrival; mutable transfer is affine, simultaneous and preserves extent evidence only across unchanged paths.
- `terminal-psi-byte-views-010` — Source range endpoints evaluate in authored argument order after branch selection; signed indices do not silently become unsigned.
- `terminal-psi-byte-views-011` — Collection establishment distinguishes shared containment from mutable representation equality and independently preserves qualifications.
- `terminal-psi-byte-views-012` — RepresentationBytes captures u64 endpoints, nonzero stride and exact tiling; total leaf validity and target unaligned semantics remain separate.
- `terminal-psi-byte-views-013` — Element counts/types never arise from byte arithmetic; by-value element reads require copy, and projected writes preserve bounded leaf invariants.
- `terminal-psi-byte-views-014` — Descriptor release, descendants and last observations govern backing loans independently of no-code cleanup.
- `terminal-psi-byte-views-015` — Fuel charges before mutation; resume does not replay committed writes, and unsupported native descriptors reject before projection.
- `terminal-psi-byte-views-016` — Indexed contract observations retain exact root and selector substitution but supply no access or bounds themselves.

**Remaining:** Existing bounds/window rows are narrow candidates; origin, predicate invalidation, suspension and element/byte separation require additional qualification.

<a id="terminal-psi-calls-and-outcomes"></a>

#### calls and outcomes

**Source:** [wiki/spec/terminal-psi/calls_and_outcomes.md](../../spec/terminal-psi/calls_and_outcomes.md) · 13 headings · SHA-256 `ea59b70944bd676570bda7b88186dd177d08eadd63d7884fa11fa2d4599ad11b`.

**Evidence needed:** Call, cleanup, suspension and frontier controls.

**Fact area candidates:** [call_result_bound_discharge](../../../samples/cli/facts/call_result_bound_discharge), [returned_write_destination](../../../samples/cli/facts/returned_write_destination), [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff).

**Other candidate areas:** [pass/calls (289 roots)](../../../tests/omega/pass/calls), [fail/calls (117 roots)](../../../tests/omega/fail/calls), [run/calls (28 roots)](../../../tests/omega/run/calls), [pass/ownership (140 roots)](../../../tests/omega/pass/ownership), [fail/ownership (105 roots)](../../../tests/omega/fail/ownership), [run/ownership (7 roots)](../../../tests/omega/run/ownership), [pass/drops (90 roots)](../../../tests/omega/pass/drops), [fail/drops (56 roots)](../../../tests/omega/fail/drops).

**Required families:**

- `terminal-psi-calls-and-outcomes-001` — Unit creates no fake result; scalar pseudo-values and structural argument/result places carry exact distinct claims.
- `terminal-psi-calls-and-outcomes-002` — Return claims transfer existing ownership only after successful completion; crashes produce no result.
- `terminal-psi-calls-and-outcomes-003` — Scalar qualification sets change only through explicit semantic edges; qualified host inputs without an evidence input reject.
- `terminal-psi-calls-and-outcomes-004` — Scalar array establishment supplies the complete exact row-major payload, leaf types and dimensions, including zero dimensions, without rounding or hidden shape.
- `terminal-psi-calls-and-outcomes-005` — Record construction preserves complete field roster, authored evaluation, nested ownership and actual predicate operands; copies differ from original borrowed referents.
- `terminal-psi-calls-and-outcomes-006` — Each requires clause is discharged under exact positional substitution before ensures; guarantees cannot prove their own preconditions.
- `terminal-psi-calls-and-outcomes-007` — Entry snapshots, current values and result names retain distinct identities.
- `terminal-psi-calls-and-outcomes-008` — Mixed arguments evaluate in authored order and preserve captured values across later side effects.
- `terminal-psi-calls-and-outcomes-009` — Owned, shared and partial result transfers leave exact residual cleanup.
- `terminal-psi-calls-and-outcomes-010` — Normal cleanup is ordered, captures before action, charges before edge, and does not replay after resumption.
- `terminal-psi-calls-and-outcomes-011` — External termination creates no result, cleanup or frontier merge.
- `terminal-psi-calls-and-outcomes-012` — Suspension keeps calls incomplete, preserves live loans and complete frontier rosters, and matches plans/sites and runtime backing exactly.
- `terminal-psi-calls-and-outcomes-013` — Task activation uses a strong commitment rather than a compact key.
- `terminal-psi-calls-and-outcomes-014` — Crash routes preserve exact cause and path guard with local frontier lower bounds, without claiming survivor theorems.

**Remaining:** Existing scalar/loan rows cover narrow call facts; cleanup ordering, suspension and partial results need distinct evidence.

<a id="terminal-psi-control-flow"></a>

#### control flow

**Source:** [wiki/spec/terminal-psi/control_flow.md](../../spec/terminal-psi/control_flow.md) · 5 headings · SHA-256 `0b9847a1da8acd4a3543280f3bc4df0859d539db394c4258866a1130fe1a157c`.

**Evidence needed:** Control-flow, invariant and termination admission controls.

**Fact area candidates:** [range_fact_delivery](../../../samples/cli/facts/range_fact_delivery), [field_where_index](../../../samples/cli/facts/field_where_index).

**Other candidate areas:** [pass/control_flow (173 roots)](../../../tests/omega/pass/control_flow), [fail/control_flow (31 roots)](../../../tests/omega/fail/control_flow), [pass/termination (136 roots)](../../../tests/omega/pass/termination), [fail/termination (91 roots)](../../../tests/omega/fail/termination).

**Required families:**

- `terminal-psi-control-flow-001` — SCCs use every actual block and edge; no hidden acyclic residue omits exits or frontiers.
- `terminal-psi-control-flow-002` — Successor telescopes and ownership frontiers reach the structural fixed point.
- `terminal-psi-control-flow-003` — Edges perform only authored transfers and cleanup; cyclicity alone cannot dispose resources.
- `terminal-psi-control-flow-004` — Structural case access retains exact projected root/case proof and displacement; layout alone cannot authorize payload access.
- `terminal-psi-control-flow-005` — Safety holds independently of ranking.
- `terminal-psi-control-flow-006` — Destination scalar invariant is one conjunctive row discharged simultaneously on every incoming edge; entry has no cyclic row and prefix facts are not cyclic hypotheses.
- `terminal-psi-control-flow-007` — Partial operation safety precedes using its result equation.
- `terminal-psi-control-flow-008` — Ranking covers all internal edges, strict-edge removal is acyclic, and authored transitions retain their decrease obligations.
- `terminal-psi-control-flow-009` — Descriptor bounds and rank substitutions are exact; grouped evidence changes invalidate dependent admission.
- `terminal-psi-control-flow-010` — Proof recursion components remain separate from executable SCCs and require no runtime layout.
- `terminal-psi-control-flow-011` — Source-to-Terminal and Terminal-to-target correspondence are separate obligations.

**Remaining:** Fact rows offer local/cross-state candidates, not complete SCC, ranking or fixed-point coverage.

<a id="terminal-psi-dynamic-dispatch"></a>

#### dynamic dispatch

**Source:** [wiki/spec/terminal-psi/dynamic_dispatch.md](../../spec/terminal-psi/dynamic_dispatch.md) · 10 headings · SHA-256 `09bcb36e36054486c73d7e0b50d6046d2425043323fe57f98c79831c612ea2b2`.

**Evidence needed:** Dispatch selection, whole-table completeness, custody and native replay.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/traits (92 roots)](../../../tests/omega/pass/traits), [fail/traits (117 roots)](../../../tests/omega/fail/traits), [run/traits (3 roots)](../../../tests/omega/run/traits), [pass/generics (131 roots)](../../../tests/omega/pass/generics), [fail/generics (221 roots)](../../../tests/omega/fail/generics), [run/generics (1 roots)](../../../tests/omega/run/generics), [pass/calls (289 roots)](../../../tests/omega/pass/calls), [fail/calls (117 roots)](../../../tests/omega/fail/calls), [run/calls (28 roots)](../../../tests/omega/run/calls).

**Required families:**

- `terminal-psi-dynamic-dispatch-001` — One named conformance supplies every exact inherited overload; ambiguous names, partial maps and mixed conformances reject.
- `terminal-psi-dynamic-dispatch-002` — Selector is a distinct runtime operand, with ordinary explicit data arguments and no hidden instance/receiver authority.
- `terminal-psi-dynamic-dispatch-003` — Joins, forwarding, storage and rebinding retain actual selection and exact requirement-owned call shape.
- `terminal-psi-dynamic-dispatch-004` — Eligibility requires concrete representations and public input-relative result origins; implementation-private identities and unsupported generic binders reject.
- `terminal-psi-dynamic-dispatch-005` — Operational envelopes compose demands by union/maximum and guarantees/carry by intersection; only reachable requirements contribute inferred parameter contracts.
- `terminal-psi-dynamic-dispatch-006` — Finite generic families retain the declaration's complete canonical tuple roster, exact row shape/envelope and ownership; membership alone grants no uniform ABI.
- `terminal-psi-dynamic-dispatch-007` — ClosedConformanceApplication retains full telescope, ordered rows, registry and strong identity, including overwritten initializer versus latest selection.
- `terminal-psi-dynamic-dispatch-008` — CallDynamic preserves full argument-relative claim paths, guarded activity, exact Unit/scalar/structural result semantics and successful-return correspondence.
- `terminal-psi-dynamic-dispatch-009` — Native tables retain every canonical row including zero trap slots; replay checks descriptor/table/function relocations and final bytes.
- `terminal-psi-dynamic-dispatch-010` — Ordered stores must affect original caller referents; known targets retain direct-call cost and escaping selectors retain tables/adapters.
- `terminal-psi-dynamic-dispatch-011` — Local tables do not cross replaceable components; component calls use evaluated boundary plans.

**Remaining:** No dedicated dynamic-selector fact row identified; nongeneric support does not establish finite-family expansion or physical table replay.

<a id="terminal-psi-encoding"></a>

#### encoding

**Source:** [wiki/spec/terminal-psi/encoding.md](../../spec/terminal-psi/encoding.md) · 28 headings · SHA-256 `af238d57e6ac79d510f15fe2ed11418ac46b85787cf472c794c239760921a221`.

**Evidence needed:** Canonical wire/schema roundtrips and exhaustive malformed-input/corruption controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/terminal_psi (11 roots)](../../../tests/omega/pass/terminal_psi), [omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/semantic_module/target_family_tests.rs](../../../omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/semantic_module/target_family_tests.rs).

**Required families:**

- `terminal-psi-encoding-001` — Canonical magic/markers, fixed-width little-endian values, nonzero identities and exact execution order reject all alternative spellings, unknown tags, truncation and trailing bytes.
- `terminal-psi-encoding-002` — Every accepted decode re-encodes byte-for-byte; golden current bytes are not historical compatibility or admission.
- `terminal-psi-encoding-003` — Qualification catalogs, change edges, entry ranges and block invariant/arrival rosters retain exact canonical order and semantic identity.
- `terminal-psi-encoding-004` — Closed scalar/math/proof/content/proposition grammars preserve typed payloads, scope, distinct tag spaces and depth bounds.
- `terminal-psi-encoding-005` — Operation/result/terminator rows preserve every operand, obligation, selected evidence, cleanup and crossing field; retired operation tags reject.
- `terminal-psi-encoding-006` — Machine/module tables preserve exact declared order, parameter interleave, structural result/claim sources, content partitions and service closures.
- `terminal-psi-encoding-007` — Target families retain checked head commitments and all members; target layout applications retain complete validated alternatives without host-selected geometry. Candidate: `family-codec`.
- `terminal-psi-encoding-008` — Structural declarations and identity-only referents share a disjoint namespace; pointer target identity grants no concrete storage shape.
- `terminal-psi-encoding-009` — Boundary/candidate rows preserve exact schema, selecting declaration, application, public region order, completion, termination and membership contracts.
- `terminal-psi-encoding-010` — Retained-borrow segment tags differ from content-place tags and cannot share decode tables.
- `terminal-psi-encoding-011` — Float-meaning, proposition/evidence, proof-output and recursive-component rows preserve dense/ordered identities, provenance and exact dependencies.
- `terminal-psi-encoding-012` — Conformance migration removes retired subject semantics; legacy fields in current tables do not authorize compatibility dialects.
- `terminal-psi-encoding-013` — Suspension preserves every demand occurrence, cardinality, capture, supplier alternatives and loan graph rather than deduplicating equal coordinates.
- `terminal-psi-encoding-014` — Artifact, debug and optimization sections rebind exact semantic subjects; empty optimization preserves input/output identity and absent differs from present-empty.
- `terminal-psi-encoding-015` — Mathematical certificates use a canonical postorder DAG of earlier handles, exact signature/context/roots and reject Dummy/unreachable/alternate tables.
- `terminal-psi-encoding-016` — Semantic, debug, optimization and installation commitments have separate domains and grant no executable authority by decoding.
- `terminal-psi-encoding-017` — Placement/installation rosters retain strong commitments, complete emitted hashes, admitted settlements, spans and relocations, not compact report substitutes.

**Remaining:** Needs codec/schema and corruption evidence, not a nine-place row. The document itself says it is not yet a complete byte-level decoder specification; marker/vocabulary migration narratives require exact source reconciliation.

<a id="terminal-psi-fact-validity"></a>

#### fact validity

**Source:** [wiki/spec/terminal-psi/fact_validity.md](../../spec/terminal-psi/fact_validity.md) · 3 headings · SHA-256 `56b8a01777c6eac5519cc97db47f535f9ce6ed61716337420aa8a3ee96cdac6c`.

**Evidence needed:** source facts and Terminal checking.

**Fact area candidates:** [length](../../../samples/cli/facts/length), [alias_fact_transfer](../../../samples/cli/facts/alias_fact_transfer), [call_result_bound_discharge](../../../samples/cli/facts/call_result_bound_discharge), [matched_field_windows](../../../samples/cli/facts/matched_field_windows), [float_range_delivery](../../../samples/cli/facts/float_range_delivery), [repro](../../../samples/cli/facts/repro).

**Other candidate areas:** [pass/facts (204 roots)](../../../tests/omega/pass/facts), [fail/facts (219 roots)](../../../tests/omega/fail/facts), [pass/contracts (49 roots)](../../../tests/omega/pass/contracts), [fail/contracts (86 roots)](../../../tests/omega/fail/contracts), [run/contracts (3 roots)](../../../tests/omega/run/contracts), [pass/control_flow (173 roots)](../../../tests/omega/pass/control_flow), [fail/control_flow (31 roots)](../../../tests/omega/fail/control_flow).

**Required families:**

- `terminal-psi-fact-validity-001` — A usable premise dominates its use and remains valid on every path.
- `terminal-psi-fact-validity-002` — Acyclic joins intersect matching valid predecessor tokens; cyclic joins additionally prove establishment and preservation. Candidate: `bad-arrival`.
- `terminal-psi-fact-validity-003` — Every scalar block predicate arrival uses exact simultaneous substitution over all actual incoming edges.
- `terminal-psi-fact-validity-004` — A partial operation's result equation is available only after its own safety proof on the normal successor.
- `terminal-psi-fact-validity-005` — Stored-value/SSA equality requires the same exact reaching value on every predecessor, with no unknown aliasing write or unproved cyclic origin.
- `terminal-psi-fact-validity-006` — Each callee requires clause has its own exact obligation including binder kind, substitution, pre/post versions and evidence lifetime.
- `terminal-psi-fact-validity-007` — Invocation-entry snapshots remain distinct from current values and named-state arrival assumptions.
- `terminal-psi-fact-validity-008` — Crash guards are conclusions to prove, never assumptions; every incoming crash path is checked.
- `terminal-psi-fact-validity-009` — A published call-ceiling union does not establish an individual alternative.
- `terminal-psi-fact-validity-010` — Boolean denotation and comparison polarity preserve the selected operation and operand identity.
- `terminal-psi-fact-validity-011` — PredicateDenotation checks its unchanged-premise child and exact outer goal; budget exhaustion rejects conversion.
- `terminal-psi-fact-validity-012` — Only specified same-carrier literal integer relations normalize to Truth/Falsehood; IEEE/storage/open terms remain distinct.
- `terminal-psi-fact-validity-013` — ValueEqualityTransport retains every proved oriented equation, exact goal structure and dependencies and rejects cyclic expansion.

**Remaining:** Fact rows are source-level candidates, not proof that every Terminal validity rule or independent equality conversion is covered. Cyclic, partial-operation, crash-site and exact conversion controls require additional evidence.

<a id="terminal-psi-integer-certificates"></a>

#### integer certificates

**Source:** [wiki/spec/terminal-psi/integer_certificates.md](../../spec/terminal-psi/integer_certificates.md) · 10 headings · SHA-256 `73a934b36b8dc25ea06b22b4c2e893ecf9414c1251d47098a40690669416e009`.

**Evidence needed:** Canonical-goal derivation and adversarial certificate controls.

**Fact area candidates:** [exact_sub_guard](../../../samples/cli/facts/exact_sub_guard), [dependent_subtract](../../../samples/cli/facts/dependent_subtract), [remainder_width_index](../../../samples/cli/facts/remainder_width_index), [bounded_scalar_sums](../../../samples/cli/facts/bounded_scalar_sums), [compound_index_guard](../../../samples/cli/facts/compound_index_guard), [index_every_width](../../../samples/cli/facts/index_every_width), [bounded_products](../../../samples/cli/facts/bounded_products).

**Other candidate areas:** [pass/arithmetic (103 roots)](../../../tests/omega/pass/arithmetic), [fail/arithmetic (128 roots)](../../../tests/omega/fail/arithmetic), [run/arithmetic (6 roots)](../../../tests/omega/run/arithmetic), [pending/arithmetic (1 roots)](../../../tests/omega/pending/arithmetic), [pass/facts (204 roots)](../../../tests/omega/pass/facts), [fail/facts (219 roots)](../../../tests/omega/fail/facts), [pass/proofs (115 roots)](../../../tests/omega/pass/proofs), [fail/proofs (191 roots)](../../../tests/omega/fail/proofs), [run/proofs (2 roots)](../../../tests/omega/run/proofs), [pending/proofs (1 roots)](../../../tests/omega/pending/proofs).

**Required families:**

- `terminal-psi-integer-certificates-001` — Reconstruct the exact canonical operation question; mixed integer comparisons retain original carriers, polarity and mathematical denotation without unsigned promotion.
- `terminal-psi-integer-certificates-002` — Wrapping polynomial equality proves only a zero modular polynomial, never order, nonunit cancellation or ranking.
- `terminal-psi-integer-certificates-003` — Canonical divisor, signed MIN/-1, one-bit, shift and representability questions preserve exact schema and conjunction/disjunction order.
- `terminal-psi-integer-certificates-004` — Branch consequences preserve all-path scope, endpoint adjacency and IEEE distinctions.
- `terminal-psi-integer-certificates-005` — Every elementary child rule checks typed endpoints, exact identity, direction and premise restrictions independently.
- `terminal-psi-integer-certificates-006` — Endpoint certificates replay source-ordered definitions and strictly earlier literal landings, rejecting stale, missing, unused or mistyped premises.
- `terminal-psi-integer-certificates-007` — Unsigned runtime remainder requires exact positive divisor and retains formation safety separately from result < divisor.
- `terminal-psi-integer-certificates-008` — Relative wrapping translations require every intermediate no-wrap condition; final-result bounds alone are insufficient.
- `terminal-psi-integer-certificates-009` — Direct multiplication checks four mathematical corner products and never uses its future result equation to prove safety.
- `terminal-psi-integer-certificates-010` — Cast words are nonempty, contiguous and exact; carrier intersection transports bounds without licensing partial casts.
- `terminal-psi-integer-certificates-011` — Correlated forbidden roots require disjoint affine branches, exact parameter/bounds and an empty reconstructed exceptional-root set.
- `terminal-psi-integer-certificates-012` — Producer witnesses acquire authority only through the specified checked rule and premises; search strategy creates no proof rule.

**Remaining:** Several source arithmetic rows exist; corruption and exact canonical-goal rejection need kernel/verifier tests independently of positive source cases.

<a id="terminal-psi-loans"></a>

#### loans

**Source:** [wiki/spec/terminal-psi/loans.md](../../spec/terminal-psi/loans.md) · 4 headings · SHA-256 `012b10e6faa52a8e07efe555391aac4b5195185d7ac8d88bc70d66c8b54082a0`.

**Evidence needed:** Loan formation, reborrow and compatibility controls.

**Fact area candidates:** [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff), [indexed_element_mut_loan](../../../samples/cli/facts/indexed_element_mut_loan), [guarded_line_loan](../../../samples/cli/facts/guarded_line_loan), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan), [returned_write_destination](../../../samples/cli/facts/returned_write_destination), [disjoint_windows](../../../samples/cli/facts/disjoint_windows).

**Other candidate areas:** [pass/borrows (287 roots)](../../../tests/omega/pass/borrows), [fail/borrows (286 roots)](../../../tests/omega/fail/borrows), [run/borrows (30 roots)](../../../tests/omega/run/borrows), [pending/borrows (1 roots)](../../../tests/omega/pending/borrows), [pass/borrow (51 roots)](../../../tests/omega/pass/borrow), [fail/borrow (71 roots)](../../../tests/omega/fail/borrow).

**Required families:**

- `terminal-psi-loans-001` — Logical compatibility cannot form, extend, widen or duplicate a loan or authority; capture is exact.
- `terminal-psi-loans-002` — Logical, content and physical footprints remain separate with explicit bridges.
- `terminal-psi-loans-003` — Successful comparisons are not serialized and replayed inside the same compile; removed PCC is not a requirement.
- `terminal-psi-loans-004` — Reborrows preserve immediate parent and full cohorts, end deepest-first and restore original authority; lexical expiry alone is insufficient.
- `terminal-psi-loans-005` — Dynamic selectors retain original capture separately from projected suffix.
- `terminal-psi-loans-006` — State-exit source dispositions are not serialized proof certificates; migrations preserve semantics.

**Remaining:** Several narrow loan rows exist; parent/cohort restoration and logical-to-physical footprint correspondence are not established by their presence.

<a id="terminal-psi-mathematical-values"></a>

#### mathematical values

**Source:** [wiki/spec/terminal-psi/mathematical_values.md](../../spec/terminal-psi/mathematical_values.md) · 6 headings · SHA-256 `c6293785d5362f8e0c3c514480945056f5f1ce0d00a08f743df8737511e665fb`.

**Evidence needed:** Proof denotation, carrier identity, correspondence and erasure controls.

**Fact area candidates:** [float_range_delivery](../../../samples/cli/facts/float_range_delivery).

**Other candidate areas:** [pass/float (124 roots)](../../../tests/omega/pass/float), [fail/float (101 roots)](../../../tests/omega/fail/float), [run/float (1 roots)](../../../tests/omega/run/float), [pass/proofs (115 roots)](../../../tests/omega/pass/proofs), [fail/proofs (191 roots)](../../../tests/omega/fail/proofs), [run/proofs (2 roots)](../../../tests/omega/run/proofs), [pending/proofs (1 roots)](../../../tests/omega/pending/proofs).

**Required families:**

- `terminal-psi-mathematical-values-001` — Unbounded integer terms retain exact source carriers/identities and address models without creating runtime arithmetic or authority.
- `terminal-psi-mathematical-values-002` — Representable expands schema-owned lower/upper bounds in canonical order; evidence strength does not change the question.
- `terminal-psi-mathematical-values-003` — Partial executable operations remain independently safe before their equations can be used.
- `terminal-psi-mathematical-values-004` — FloatMeaning classes retain disjoint exact literal, owner, parameter, result, operation, block, call and structural-leaf identities.
- `terminal-psi-mathematical-values-005` — Semantic applications rejoin exact sealed catalog rows, complete ordered operands and earlier meaning dependencies.
- `terminal-psi-mathematical-values-006` — Projection uses exact sealed declarations and a domain-separated contract; same-shaped local declarations or invented coordinates reject.
- `terminal-psi-mathematical-values-007` — Machine result and imported ensures retain owner/use substitution; nested states do not invent result carriers.
- `terminal-psi-mathematical-values-008` — FloatMeaning equality is structural: NaN reflexive and signed zeros distinct, unlike IEEE comparison.
- `terminal-psi-mathematical-values-009` — Proof meanings/evidence erase with no runtime conversion, comparison, check or ABI value.

**Remaining:** Float range delivery is a narrow candidate, not complete sealed meaning/catalog correspondence or proof-erasure evidence.

<a id="terminal-psi-observations"></a>

#### observations

**Source:** [wiki/spec/terminal-psi/observations.md](../../spec/terminal-psi/observations.md) · 6 headings · SHA-256 `359b6f0c22a93e8c2c694bda3a9133bb317cac1f54f95e461e315a5a0475137d`.

**Evidence needed:** Observation schema, trace equality and physical correspondence controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host), [pass/terminal_psi (11 roots)](../../../tests/omega/pass/terminal_psi).

**Required families:**

- `terminal-psi-observations-001` — Static observer profiles differ from execution traces; traces distinguish return, crash, external termination and divergence.
- `terminal-psi-observations-002` — Complete canonical root/site rosters retain operation-owned crash causes, including BoundaryCall, without fabricated edges.
- `terminal-psi-observations-003` — Malformed tags, order, types, duplicates, missing fields and trailing bytes reject; profile equality alone is not refinement.
- `terminal-psi-observations-004` — Trace equality preserves IEEE bits including signed zero and NaN payload, exact integers/booleans and whole structural values, without digest substitution.
- `terminal-psi-observations-005` — ProcessExit uses exact canonical i32 and its authored domain authority, not host interpretation.
- `terminal-psi-observations-006` — Fuel/timeout are consumer outcomes rather than semantics; physical correspondence is separately admitted.

**Remaining:** No dedicated observer row; source-to-Terminal implementation-status wording differs from process_exit status and remains unverified, not a normative decision.

<a id="terminal-psi-ownership"></a>

#### ownership

**Source:** [wiki/spec/terminal-psi/ownership.md](../../spec/terminal-psi/ownership.md) · 7 headings · SHA-256 `11a3820e40b1a27f4a497a40c6f009b64633e04d867465844ce9b69780779422`.

**Evidence needed:** Exact ownership partitions, restoration, cleanup and physical preservation.

**Fact area candidates:** [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff), [returned_write_destination](../../../samples/cli/facts/returned_write_destination), [indexed_element_mut_loan](../../../samples/cli/facts/indexed_element_mut_loan).

**Other candidate areas:** [pass/ownership (140 roots)](../../../tests/omega/pass/ownership), [fail/ownership (105 roots)](../../../tests/omega/fail/ownership), [run/ownership (7 roots)](../../../tests/omega/run/ownership), [pass/drops (90 roots)](../../../tests/omega/pass/drops), [fail/drops (56 roots)](../../../tests/omega/fail/drops).

**Required families:**

- `terminal-psi-ownership-001` — Claim paths are typed, canonical and case-guarded; missing membership never means inactive, and borrowed/dynamic/scalar paths cannot fabricate claims.
- `terminal-psi-ownership-002` — Calls/results bind complete active claim sets by path, transfer once and establish outputs only on successful completion.
- `terminal-psi-ownership-003` — State bindings continue incoming obligations after capture/cleanup, preserve simultaneous swaps and reject collisions or missing/extra paths.
- `terminal-psi-ownership-004` — Partial moves make ancestors unavailable; residual complements are maximal live subtrees in recursive reverse establishment order.
- `terminal-psi-ownership-005` — Huge array dimensions are size-checked before complement enumeration; no runtime bitmap replaces static cleanup.
- `terminal-psi-ownership-006` — Exclusive extraction opens exact captured-place restoration debt; wrong-place repair, overlapping observations and missing repair on return reject.
- `terminal-psi-ownership-007` — Whole-root and runtime-view-element windows preserve displaced referent multiplicity, loan authority and runtime selector bounds independently.
- `terminal-psi-ownership-008` — Normal exclusive-call completion refreshes only the restored caller region and actual active claim paths, preserving disjoint moved siblings.
- `terminal-psi-ownership-009` — Root identity, current location, cleanup region and home retirement remain distinct; every obligation belongs exactly once in an edge partition.
- `terminal-psi-ownership-010` — Fuel charges before cleanup/transfer and suspension preserves uncommitted frontiers; crash and ProcessExit have separate abandonment rules.
- `terminal-psi-ownership-011` — Nominal cleanup targets must return normally, have locally established prerequisites and preserve exact action order, subjects and costs.
- `terminal-psi-ownership-012` — Erased owners retain hidden storage and eligible cleanup; borrowed erased views never dispose referents.
- `terminal-psi-ownership-013` — Physical planning consumes checked transfers, preserves promised address stability, and retains semantic edge/action attribution through sharing or cloning.

**Remaining:** Loan/write rows cover small custody facts; partial residual partitions, dynamic windows and cleanup scheduling need separate source/native/corruption fixtures.

<a id="terminal-psi-product"></a>

#### product

**Source:** [wiki/spec/terminal-psi/product.md](../../spec/terminal-psi/product.md) · 6 headings · SHA-256 `4e90d896d6d19b1a367496be2f427173de0392fbffb7d2d6076d25ded716605d`.

**Evidence needed:** Source-free product roundtrip, identity and admission controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/terminal_psi (11 roots)](../../../tests/omega/pass/terminal_psi), [pass/targets (47 roots)](../../../tests/omega/pass/targets), [fail/targets (39 roots)](../../../tests/omega/fail/targets).

**Required families:**

- `terminal-psi-product-001` — Terminal Psi is immutable, concrete, post-instantiation and source-free, with exact executable choices retained.
- `terminal-psi-product-002` — A target family retains all checked bodies; root contract differs from runtime target selection.
- `terminal-psi-product-003` — Demands preserve exact operation, owner requirement and closed tuple before optimization; provider candidates are not selections.
- `terminal-psi-product-004` — Choosing conformance identity remains distinct from actual body; retain template owner and specialization roster despite equal runtime signatures.
- `terminal-psi-product-005` — Callback identity preserves registrar, binder, requirement, body and placement.
- `terminal-psi-product-006` — Assembly is typed semantic instructions with exact environment coverage, not opaque text.
- `terminal-psi-product-007` — Fixed reach contributions are distinct from installation bound rows.
- `terminal-psi-product-008` — Current product carries no proof evidence: consumers validate structural, contract and authority boundaries and rely on Psi logical checking.
- `terminal-psi-product-009` — Canonical declaration ownership survives reload/cache; stale or unknown encodings reject.
- `terminal-psi-product-010` — Pending build choices confer no authority; lowering, selected optimization and publication retain phase ordering, including empty selections.
- `terminal-psi-product-011` — Semantic, fuel, native realization and installation identities remain separate.

**Remaining:** No dedicated product row; source-free roundtrip and corruption tests must qualify identities without reinstating removed proof-carrying envelopes.

<a id="terminal-psi-structural-access"></a>

#### structural access

**Source:** [wiki/spec/terminal-psi/structural_access.md](../../spec/terminal-psi/structural_access.md) · 9 headings · SHA-256 `f428f73dec8415ec2c683eafbc5df781a6aa24a024986492cbcbdb4651b9f7dd`.

**Evidence needed:** Reference identity, construction, access, invalidation and native caller observation.

**Fact area candidates:** [data_where_standing](../../../samples/cli/facts/data_where_standing), [field_where_index](../../../samples/cli/facts/field_where_index), [indexed_element_mut_loan](../../../samples/cli/facts/indexed_element_mut_loan), [reference_arrival_handoff](../../../samples/cli/facts/reference_arrival_handoff), [returned_write_destination](../../../samples/cli/facts/returned_write_destination), [guarded_record_line_loan](../../../samples/cli/facts/guarded_record_line_loan).

**Other candidate areas:** [pass/borrows (287 roots)](../../../tests/omega/pass/borrows), [fail/borrows (286 roots)](../../../tests/omega/fail/borrows), [run/borrows (30 roots)](../../../tests/omega/run/borrows), [pending/borrows (1 roots)](../../../tests/omega/pending/borrows), [pass/structural (25 roots)](../../../tests/omega/pass/structural), [fail/structural (4 roots)](../../../tests/omega/fail/structural), [pass/write_only (2 roots)](../../../tests/omega/pass/write_only).

**Required families:**

- `terminal-psi-structural-access-001` — All borrowed access denotes original storage; register ABI, equal pointers or a callee observing staged writes cannot justify copying referents.
- `terminal-psi-structural-access-002` — Atomic cell identity/orderings are distinct from scalar layout; shared atomic mutation expires affected observations without expiring captured snapshots.
- `terminal-psi-structural-access-003` — Reference carriers own permission rather than referents; explicit Referent crossings cannot transfer owned claims/cleanup.
- `terminal-psi-structural-access-004` — Representation views require admitted total validity or bidirectional mutable compatibility, preserve ingress identity and grant no disjointness from changed field names.
- `terminal-psi-structural-access-005` — Returned references reconstruct exact public origins; release is deepest-first and successful completion preserves ancestry and complete leaf rosters.
- `terminal-psi-structural-access-006` — Stored-reference crossings require live unsuspended custody at every possible origin; native code loads actual pointer words before offsets.
- `terminal-psi-structural-access-007` — Write-only loans require valid existing storage and permit no content read, readable reborrow, take, swap or read-modify-write.
- `terminal-psi-structural-access-008` — Discardable and descriptor-cleanup replacement authorities are exact, with complete child closure and old/new rosters; copy permission does not follow.
- `terminal-psi-structural-access-009` — Case observations require current readable, established exact sums; runtime selection owes bounds and supplies no canonical mutable-case fact.
- `terminal-psi-structural-access-010` — Record/structural-array construction uses complete exact operands in authored order, transfers claims/descriptors once and publishes atomically after charge.
- `terminal-psi-structural-access-011` — Restricted scalar initializers and stores prove the declaration predicate on the new SSA value; reads transport the invariant onto fresh snapshots.
- `terminal-psi-structural-access-012` — Primitive locals require dominating initialization on every arrival; repeated reads need exact reaching definitions and no aliasing mutation.
- `terminal-psi-structural-access-013` — Runtime-index stores own all selector obligations and invalidate the conservative selected-array prefix without publishing a canonical field equation.
- `terminal-psi-structural-access-014` — Concatenation preserves source predicates/current lengths and proves capacity on the mathematical sum; result live length is not capacity.
- `terminal-psi-structural-access-015` — Whole byte replacement writes bytes plus live length; shared descriptor replacement instead installs backing plus extent and preserves detached loans.
- `terminal-psi-structural-access-016` — Stored descriptor lifetime, incoming-backing compatibility and source predicates are separate; unresolved actuals cannot skip call checks.
- `terminal-psi-structural-access-017` — Indexed UTF-8 writes need an ASCII-established current carrier plus per-byte obligations, not merely ASCII replacement bytes or in-bounds indices.
- `terminal-psi-structural-access-018` — Length metadata can be read through write-only access but grants no content read; overlapping extent changes invalidate it.
- `terminal-psi-structural-access-019` — Native controls must observe original caller writes across forwarded/projected and register/stack references, with copied-argument and stale-plan rejects on both Linux architectures.

**Remaining:** Existing fact rows address bounded reads and narrow writes; reference-origin, descriptor replacement, UTF-8 preservation and full native architecture coverage remain independently unqualified.

<a id="terminal-psi-structural-predicates"></a>

#### structural predicates

**Source:** [wiki/spec/terminal-psi/structural_predicates.md](../../spec/terminal-psi/structural_predicates.md) · 4 headings · SHA-256 `cb18337d5b6047f6575af486a0ed691a470e6703063db8427dea54cc1f5367fc`.

**Evidence needed:** Predicate path substitution and equality/operation denotation controls.

**Fact area candidates:** [case_where_equality](../../../samples/cli/facts/case_where_equality), [dependent_subtract](../../../samples/cli/facts/dependent_subtract), [alias_fact_transfer](../../../samples/cli/facts/alias_fact_transfer).

**Other candidate areas:** [pass/contracts (49 roots)](../../../tests/omega/pass/contracts), [fail/contracts (86 roots)](../../../tests/omega/fail/contracts), [run/contracts (3 roots)](../../../tests/omega/run/contracts), [pass/cases (27 roots)](../../../tests/omega/pass/cases), [pass/arithmetic (103 roots)](../../../tests/omega/pass/arithmetic), [fail/arithmetic (128 roots)](../../../tests/omega/fail/arithmetic), [run/arithmetic (6 roots)](../../../tests/omega/run/arithmetic), [pending/arithmetic (1 roots)](../../../tests/omega/pending/arithmetic).

**Required families:**

- `terminal-psi-structural-predicates-001` — Paths retain every exact typed field, index and case; projected calls prepend actual paths and substitute both roots independently.
- `terminal-psi-structural-predicates-002` — Structural equality expands canonical relevant fields/cases; all-erased records differ from truly empty records, and inequality negates the whole equality.
- `terminal-psi-structural-predicates-003` — IEEE leaves retain NaN/signed-zero comparison semantics; byte equality uses live prefixes rather than capacity or pointers.
- `terminal-psi-structural-predicates-004` — Integer terms retain carrier and Exact/Wrapping/Saturating denotation, including Euclidean wrapping shift counts.
- `terminal-psi-structural-predicates-005` — Every division/remainder needs nonzero and Exact signed operations exclude MIN/-1; calls retain complete applicable requirements.
- `terminal-psi-structural-predicates-006` — Trapping arithmetic does not form predicate terms; mathematical embed and executable crashes have distinct evidence.
- `terminal-psi-structural-predicates-007` — Predicate vocabulary grants no ownership or content transfer.

**Remaining:** Source equality/arithmetic rows are narrow candidates; adversarial typed-path, canonical expansion and stale-substitution tests require separate mapping.

<a id="area-build"></a>

### build

<a id="build-assumed-binding-contracts"></a>

#### assumed binding contracts

**Source:** [wiki/spec/build/assumed_binding_contracts.md](../../spec/build/assumed_binding_contracts.md) · 7 headings · SHA-256 `ab5234879f12818fc25ca60880b0f3499b75be248f0e9d1f7eec2cf4050b7ebb`.

**Evidence needed:** Root trust admission, contract direction, provenance and build-time refusal controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build).

**Required families:**

- `build-assumed-binding-contracts-001` — Assumed links retain distinct required/supplied contracts and an honestly checked selected implementation, not synthesized conformance.
- `build-assumed-binding-contracts-002` — Runtime shape, operational and custody compatibility remain independent of the assumed semantic relationship with exact positional subjects.
- `build-assumed-binding-contracts-003` — Entry and exit assumptions are directional and authorize only exact crossing subjects/routes.
- `build-assumed-binding-contracts-004` — Explicit accepted refutable relationships remain auditable extremely-high-risk assumptions; ordinary refutable-grant veto still applies elsewhere.
- `build-assumed-binding-contracts-005` — Every member has one supply; competing checked/assumed supply and cycles reject.
- `build-assumed-binding-contracts-006` — Inlining, wrappers and erasure preserve complete transitive assumption and actual-provider identity.
- `build-assumed-binding-contracts-007` — Only runtime roots may authorize links, with project acceptance where applicable; Build, constants, proofs, generators and automatic tests cannot rely on them.
- `build-assumed-binding-contracts-008` — Direct supplier calls acquire no caller-facing qualification; false trust promises no recoverable or predictable runtime outcome.

**Remaining:** No dedicated assumed-binding row; acceptance needs directional, contradictory, transitive and build-refusal controls rather than an unsafe runtime oracle.

<a id="build-behavior-exclusions"></a>

#### behavior exclusions

**Source:** [wiki/spec/build/behavior_exclusions.md](../../spec/build/behavior_exclusions.md) · 10 headings · SHA-256 `1bb4d96ab4576292cae186b61a5395035e237bc1771869601dea432dd4cf7a72`.

**Evidence needed:** Selected composition absence proof, provider and replacement controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [pass/targets (47 roots)](../../../tests/omega/pass/targets), [fail/targets (39 roots)](../../../tests/omega/fail/targets).

**Required families:**

- `build-behavior-exclusions-001` — Ordinary callable checking and product behavior exclusion are independent; stronger composition facts do not rewrite public contracts.
- `build-behavior-exclusions-002` — Exclusions use exact crash/service/physical-class identities and executed original-Build operations; duplicates union idempotently and cannot remove restrictions.
- `build-behavior-exclusions-003` — Scope includes all possible entries, generated adapters, arguments, callbacks, cleanup and selected dependencies of the exact child.
- `build-behavior-exclusions-004` — Absence is reconstructed conservatively from semantics and admitted realization, not a test run, empty summary or physical instruction scan.
- `build-behavior-exclusions-005` — No-op assertion selection must establish admissibility without optional optimization; eager argument effects remain.
- `build-behavior-exclusions-006` — Abstract service invocation persists despite a silent provider; physical authority classes are separate.
- `build-behavior-exclusions-007` — Dynamic/replacement alternatives all preserve bans or retain conservative contracts; broad opaque evidence cannot establish absence.
- `build-behavior-exclusions-008` — Failure distinguishes prohibited possible behavior from missing absence evidence and publishes no partial successful product.
- `build-behavior-exclusions-009` — Complete source/provider/entry/target/policy identities invalidate stale receipts; replacement cannot widen its own envelope.
- `build-behavior-exclusions-010` — Trap exclusion promises neither Abort/ProcessExit exclusion nor recovery; automatic test roots remain separate.

**Remaining:** No dedicated exclusion row; source selection plus complete-closure and stale-consumer controls are needed.

<a id="build-boundary-shapes"></a>

#### boundary shapes

**Source:** [wiki/spec/build/boundary_shapes.md](../../spec/build/boundary_shapes.md) · 3 headings · SHA-256 `21ee6c2ffe8e948d1837d5c96d6d85755e4803028e6a75939c71743e0c861be8`.

**Evidence needed:** Public normalized ABI shape and marshaling controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/calling (1 roots)](../../../tests/omega/pass/calling), [fail/calling (2 roots)](../../../tests/omega/fail/calling), [pass/calls (289 roots)](../../../tests/omega/pass/calls), [fail/calls (117 roots)](../../../tests/omega/fail/calls), [run/calls (28 roots)](../../../tests/omega/run/calls), [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host).

**Required families:**

- `build-boundary-shapes-001` — Policies classify only publicly determined normalized shapes recursively; equal size or private compiler layout cannot stand in for ABI class.
- `build-boundary-shapes-002` — Erased members contribute no physical fragments; all-erased records have no by-value carrier.
- `build-boundary-shapes-003` — BoundarySignature flat graph and ordered native telescope retain every semantic/native-only callback origin.
- `build-boundary-shapes-004` — Opaque by-value shapes require exact closed representation applications.
- `build-boundary-shapes-005` — No implicit C array decay or slice/text descriptor ABI; foreign pointer/count/terminator shapes are explicit.
- `build-boundary-shapes-006` — Marshaling establishes access, lifetime and inbound validation independently of calling placement; proof objects do not cross.
- `build-boundary-shapes-007` — Selected entry requirement owns physical result mapping, not a machine named main.

**Remaining:** No dedicated ABI-shape row; aggregate, erased-member, callback and foreign marshaling fixtures must be independently qualified.

<a id="build-calling-plans"></a>

#### calling plans

**Source:** [wiki/spec/build/calling_plans.md](../../spec/build/calling_plans.md) · 7 headings · SHA-256 `5e3b12626ae9bbdee918ea6d96363f65d6641de674cdc3b9d420d792c5000897`.

**Evidence needed:** Policy evaluation, normalized application identity and native replay.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/calling (1 roots)](../../../tests/omega/pass/calling), [fail/calling (2 roots)](../../../tests/omega/fail/calling), [pass/calls (289 roots)](../../../tests/omega/pass/calls), [fail/calls (117 roots)](../../../tests/omega/fail/calls), [run/calls (28 roots)](../../../tests/omega/run/calls), [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host).

**Required families:**

- `build-calling-plans-001` — Exact requirement selects named CallingPolicy; deterministic terminating Build evaluation returns Accepted/Rejected before compiler validation.
- `build-calling-plans-002` — Policies use closed primitives/u64 geometry and cannot emit instructions or confer provider authority.
- `build-calling-plans-003` — CallPlan and StatePlan retain distinct promises; BoundaryPlanApplication binds complete nominal parameter/callback origins.
- `build-calling-plans-004` — Every native argument/result is placed exactly once with valid stack, alignment, registers, clobbers, saved state and control regimes.
- `build-calling-plans-005` — One validated plan governs inbound and outbound realization; borrowed parameters retain original referents.
- `build-calling-plans-006` — Normalized complete promises define strong identity; equivalent source refactors preserve it, parameter swaps change application identity.
- `build-calling-plans-007` — All imports/vtables/service tables retain selected plans rather than reselecting by target or locator.
- `build-calling-plans-008` — Candidate final machine-state footprint is independent of public calling promise.

**Remaining:** No dedicated plan row. Status reports final footprint production remains open; passing plan evaluators cannot establish final-image footprint replay.

<a id="build-compiler-request"></a>

#### compiler request

**Source:** [wiki/spec/build/compiler_request.md](../../spec/build/compiler_request.md) · 10 headings · SHA-256 `c8194b824ebdf71ea6b8c574d95f297b53b28abf4965fa748b5d8cf99c787fa1`.

**Evidence needed:** Shared D/C request vectors, outcome and malformed-wire controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [tests/bootstrap/omega-request/README.md](../../../tests/bootstrap/omega-request/README.md), [tests/product/request/test_request.py](../../../tests/product/request/test_request.py).

**Required families:**

- `build-compiler-request-001` — Standalone request contains resolved complete source and explicit invocation, not ambient discovery, host defaults or compiled-instance acceptance.
- `build-compiler-request-002` — Canonical package lineage/key, resolution, root role and complete virtual snapshots remain distinct and recomputed without origin access.
- `build-compiler-request-003` — Anonymous/default-build logical inputs remain valid but currently need an explicit shared physical extension; no fake named owner.
- `build-compiler-request-004` — OCREQ framing uses bounded subtractive extent checks, exact canonical order and subject-byte commitment before semantic interpretation.
- `build-compiler-request-005` — Closed subject/invocation field/tag tables and unassigned admission kinds reject unknown variants.
- `build-compiler-request-006` — Complete, Reject, Incomplete and InternalFailure retain distinct meaning, diagnostic phases and coordinate spaces.
- `build-compiler-request-007` — Every lexical/syntax/checking refusal and named scalar-resource provision uses its assigned table/code and exact coordinate rules.
- `build-compiler-request-008` — Private capacities and unsupported valid constructs report Incomplete, not source invalidity; proof exhaustion is not disproof.
- `build-compiler-request-009` — Optional proof searches share scoped budgets/memos and cannot publish checked artifacts after exhausted obligations.
- `build-compiler-request-010` — Failure emits only canonical OCOUT with no artifact prefix; Complete-only publication and adjacent-boundary vectors are required.
- `build-compiler-request-011` — Shared differential vectors support, but do not replace, refinement proofs; incomplete lineage and physical provisions preclude claiming interoperable V1 complete.

**Remaining:** Needs request/diagnostic/code-table vectors across D and C; contract explicitly lists incomplete physical fields, so no whole-boundary completion claim is available.

<a id="build-component-publication"></a>

#### component publication

**Source:** [wiki/spec/build/component_publication.md](../../spec/build/component_publication.md) · 16 headings · SHA-256 `cdd62d5196070d797007fc5721dc756162dbf04ae111084ff216ee9b17780058`.

**Evidence needed:** Component closure, installation/era custody and publication transactions.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers).

**Required families:**

- `build-component-publication-001` — Fused is default; Independent requires explicit root selection and a closed graph, never silent fallback or provider-selected independence.
- `build-component-publication-002` — Closure follows concrete implementation, conformance, layout, cleanup, state and custody edges to a least fixed point; enlarged cohorts need owner acceptance.
- `build-component-publication-003` — VerifiedComponent consumes exact subject/profile and verifier-established complete inventory, with unknown outgoing authority reported incomplete.
- `build-component-publication-004` — Initial composition freezes imports, authority, compatibility, resources and replacement envelope; candidates cannot widen it.
- `build-component-publication-005` — Static requirement applications are provider slots, not source Binding values; exact era entry/leave pins selected transitive dependencies.
- `build-component-publication-006` — Returned sessions, descriptors, callbacks and cleanup retain accounted era pins until unavoidable valid disposition; copy cannot mint pins.
- `build-component-publication-007` — Replacement accounts for every old activation/state/registration/claim and separates new routing from old drain and reclamation.
- `build-component-publication-008` — Supervisor linear authority owns publication; opaque retained roots require unregistration/quiescence, containment or quarantine without falsely discharging debts.
- `build-component-publication-009` — Deployment journals and recovery policy are ordinary runtime responsibilities; stored records cannot recreate installation authority.
- `build-component-publication-010` — Terminal, native artifact, candidate, installed runnable and publication receipt establish different claims.
- `build-component-publication-011` — Selected provider/progress/external-root closures bind exact installed occurrences and strong complete-set identities.
- `build-component-publication-012` — Every failed deployment transition returns exact custody; claims remain one-shot across downstream failure.
- `build-component-publication-013` — Visible publication stages sealed bytes/mode, validates before rename and replays visible output; package receipts cover their complete scope.
- `build-component-publication-014` — Reports retain nonclonable custody, explicit output category and exact entry bridge; execution uses verified paths without object/check fallback.

**Remaining:** No dedicated component row; model/source/physical lifecycle evidence is separate and source Independent status remains explicitly incomplete.

<a id="build-configuration"></a>

#### configuration

**Source:** [wiki/spec/build/configuration.md](../../spec/build/configuration.md) · 10 headings · SHA-256 `3b001c155966790b298a70659fbf06d96c9b20254dddd0812daa5a0c097a7110`.

**Evidence needed:** Evaluated configuration, target fanout and confined filesystem controls.

**Fact area candidates:** [field_where_index](../../../samples/cli/facts/field_where_index), [range_fact_delivery](../../../samples/cli/facts/range_fact_delivery).

**Other candidate areas:** [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [pass/targets (47 roots)](../../../tests/omega/pass/targets), [fail/targets (39 roots)](../../../tests/omega/fail/targets), [pass/wire (20 roots)](../../../tests/omega/pass/wire), [fail/wire (11 roots)](../../../tests/omega/fail/wire).

**Required families:**

- `build-configuration-001` — Activation roots/handles are occurrence-scoped and never durable result authority; path resolution rejects traversal and symlink escape before access.
- `build-configuration-002` — Normalized selections retain target/dependency/root/provider/tests/exclusions/optimization/output, not host capabilities.
- `build-configuration-003` — Optimizations are exact empty-by-default root selections, independent of targets/debug/assertions.
- `build-configuration-004` — Dynamic ELF output retains exact absolute non-NUL interpreter path without host probing; static output emits no PT_INTERP.
- `build-configuration-005` — Test groups and behavior exclusions are executed product selections with attenuated authority and no implicit groups/dependencies.
- `build-configuration-006` — Explicit targets are nonempty exact profiles; omission selects deployable toolchain closure, while recognition differs from implementation availability.
- `build-configuration-007` — Psi and Build run once, checking every target body; target selection narrows realization only and gives independent outcomes per target.
- `build-configuration-008` — Provider-folded constants agree across target rows and family contracts join every member rather than importing one member's private guarantee.
- `build-configuration-009` — Artifact-only applications require completed outputs and no executable roots/providers.
- `build-configuration-010` — Wire compatibility requests retain exact edge/lineage/local/peer and directional facts; omitted facts report but do not reject.

**Remaining:** Other-target-body fact candidates do not establish multi-target artifact fanout, target-default effects or directional wire admission.

<a id="build-declarations"></a>

#### declarations

**Source:** [wiki/spec/build/declarations.md](../../spec/build/declarations.md) · 7 headings · SHA-256 `43c3fd5278d77b83ac7cbc636409392f2459b06d5896d27d390fe56bc52ea935`.

**Evidence needed:** Build evaluation, dependency scheduling and nominal owner controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [pass/modules (66 roots)](../../../tests/omega/pass/modules), [fail/modules (47 roots)](../../../tests/omega/fail/modules), [pending/modules (1 roots)](../../../tests/omega/pending/modules).

**Required families:**

- `build-declarations-001` — One ordinary free build(builder:&mut Build) activation produces exactly one role; no privileged call-name parser or synthesized receiver.
- `build-declarations-002` — Dependency requests are purpose-qualified nonblocking mutations; await_provider suspends/resumes the same continuation and Build return joins all jobs before freeze.
- `build-declarations-003` — Product dependencies are not callable Build authority; aliases and product/build scopes are separate with no fallback.
- `build-declarations-004` — Default Build uses the same route and preserves supplied owner/role without discovering main or undeclared imports.
- `build-declarations-005` — Artifact-only is application output mode, requiring completed artifacts and excluding executable selections.
- `build-declarations-006` — Named identity uses declared name and canonical lineage; revision/content are resolution, root role is separate but retained.
- `build-declarations-007` — Anonymous owners derive from exact captured source/dependency/core/semantic inputs after freeze, excluding host paths, caches and random activation IDs.
- `build-declarations-008` — Consumers preserve exact declaration owners through specialization/publication; identity grants no acceptance or execution.
- `build-declarations-009` — Build facets expire with activation and require real confined sponsor operations; runtime providers and assumed runtime links give no Build grants.
- `build-declarations-010` — Workspaces catalog independent roots without merging graphs, lock acceptance or undeclared member discovery.

**Remaining:** No dedicated build/owner fact row; default versus authored malformed builds and relocation/cache-independent owner vectors need package/Build fixtures.

<a id="build-embedding"></a>

#### embedding

**Source:** [wiki/spec/build/embedding.md](../../spec/build/embedding.md) · 14 headings · SHA-256 `779b59d09e232a0ab07b9c040f19fd244ac75b65ecf61ba5c28fc602b852504c`.

**Evidence needed:** Interpreter library lifecycle, host crossings and replacement integration.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/terminal_psi (11 roots)](../../../tests/omega/pass/terminal_psi), [pass/asm (19 roots)](../../../tests/omega/pass/asm), [fail/asm (55 roots)](../../../tests/omega/fail/asm).

**Required families:**

- `build-embedding-001` — Loading Terminal does not run Build, fetch, compile or gain host authority; acquisition, compilation, environment and engine are separate.
- `build-embedding-002` — Install all demanded exact slots before execution with no native fallback or ambient provider; shared interface adapters rejoin exact declarations/contracts.
- `build-embedding-003` — Interpreter address observations are numeric identities rather than pointers, trampolines or native layout.
- `build-embedding-004` — Instances own independent guest state and retain exact program lifetime; crossings preserve exact scalar types, loans, owned resources and erasure.
- `build-embedding-005` — Start/resume preserves original arguments/handlers and once-only host effects; fuel pause is not source suspension or invariant-restoring access.
- `build-embedding-006` — Management/re-entry uses scoped authority while long-lived guest work continues; dropping handles does not discard unfinished obligations.
- `build-embedding-007` — Queued events mean acceptance rather than completion; debugging/reflection descriptions grant no arbitrary live mutation.
- `build-embedding-008` — Close/Busy/failure accounts for all activations, handles, callbacks, loans and cleanup before releasing host state.
- `build-embedding-009` — Interpreted components satisfy the same era/update/retirement contracts as native; loading creates only a candidate.
- `build-embedding-010` — Checked assembly executes admitted modeled semantics with persistent environment state, not native fallback or host hardware authority.
- `build-embedding-011` — Cathedral acceptance requires actual guest startup/device/event/replacement algorithms, including old activation retirement and assembly effects across fuel pauses.

**Remaining:** No dedicated embedding row; unit interpreter behavior does not close long-running guest-driven replacement or Cathedral integration acceptance.

<a id="build-entry-roots"></a>

#### entry roots

**Source:** [wiki/spec/build/entry_roots.md](../../spec/build/entry_roots.md) · 5 headings · SHA-256 `f995e3afb4e9bbea4acd829d8fe405ac3c9cc1ca6ce8cc7cea817ba3577ec2fc`.

**Evidence needed:** Exact slot binding, generated bridges, storage and authority controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/entry (2 roots)](../../../tests/omega/pass/entry), [fail/entry (1 roots)](../../../tests/omega/fail/entry), [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build).

**Required families:**

- `build-entry-roots-001` — Target-owned slot identity fixes direction, lifecycle, cardinality, indexing and entry shape independently.
- `build-entry-roots-002` — Every required active slot has exactly one selection; inactive rows still resolve exact profile/schema ownership without demanding unavailable backends.
- `build-entry-roots-003` — Build binds a declaration rather than invokes it; no main-name, unique export or hidden-context discovery.
- `build-entry-roots-004` — Only designated product-reference operands select product declarations with exact visibility and no generated-source cycles.
- `build-entry-roots-005` — Arrival bridge separates physical/semantic contracts and supplies only schema-declared arguments under exact target calling plan.
- `build-entry-roots-006` — Provisioned context is occurrence-local, ZII-valid and explicitly loaned from root backing; erased constructor terms confer no provider authority.
- `build-entry-roots-007` — Normal return cleans up context while crash/ProcessExit retain separate abandonment; outstanding tasks need real disposition.
- `build-entry-roots-008` — Provider demands retain exact call/application/schema/target/plan and never fall back to erased fields or late lookup.
- `build-entry-roots-009` — Complete authority ceiling, source/package/schema custody and strong physical contract identities remain independent checks.
- `build-entry-roots-010` — WCSU fits target supply and exact partition; context/stack backing cannot also be forwarded whole.

**Remaining:** No dedicated entry row; harness entry names are not evidence of exact slot/schema, generated bridge or root-backed storage qualification.

<a id="build-executable-installation"></a>

#### executable installation

**Source:** [wiki/spec/build/executable_installation.md](../../spec/build/executable_installation.md) · 6 headings · SHA-256 `62b345359003668727ab55de777fe082346560a2c2e2ce367ba1cb0bdcf26016`.

**Evidence needed:** Linear code installation, final bytes, W^X and retirement controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [omega-rust/omega/compilation/backend/runtime/executable-installation/src/tests/installed_realization_custody_fields.rs](../../../omega-rust/omega/compilation/backend/runtime/executable-installation/src/tests/installed_realization_custody_fields.rs).

**Required families:**

- `build-executable-installation-001` — Only sealed admitted immutable executable artifacts enter installation; bytes or packages cannot self-establish admission.
- `build-executable-installation-002` — Placement follows linear materialize/freeze/validate/install/retire states, with exact bytes/relocation/footprint and every failed transition returning inputs.
- `build-executable-installation-003` — Artifact identity is reusable while installed occurrence is linear; validation cannot transplant to different placement or be spent twice.
- `build-executable-installation-004` — Visibility and quiescence differ; installation reports hardware-enforced or convention-only W^X and rejects unsupported providers.
- `build-executable-installation-005` — Retirement proves exact quiescence, execute removal, write restoration and provider completion before returning placement.
- `build-executable-installation-006` — CFI needs compatible sealed entry references and protected control state, independently of code injection prevention.
- `build-executable-installation-007` — Container decode yields only a candidate; closed relocations, nonoverlap and final-byte checks remain separate from trusted-producer acceptance.
- `build-executable-installation-008` — Boot measurement is not admission; device uploads and checked trampolines retain their own provider contracts.

**Remaining:** Lifecycle model status does not establish actual target cache/execute operations or complete component byte format; physical tests required.

<a id="build-execution"></a>

#### execution

**Source:** [wiki/spec/build/execution.md](../../spec/build/execution.md) · 5 headings · SHA-256 `eacbe58a1b3d117c1ab3dcf656b890757cb1c415fc96391fe8d2f40e91614c4d`.

**Evidence needed:** Build admission, nonreplaying continuation and generated-source extension controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build).

**Required families:**

- `build-execution-001` — Check/admit each Build or awaited dependency implementation before first execution and install only exact admitted scoped slots.
- `build-execution-002` — Coordinator/package/Psi/interpreter responsibilities preserve one prepared root and mandatory dependency join before result freeze.
- `build-execution-003` — Baseline confined work needs no restricted approval; extra host requests require project acceptance and real attenuated grants before execution.
- `build-execution-004` — Runtime assumed links remain forbidden through the full Build/generator/test closure even under wider host permissions.
- `build-execution-005` — Root-selected virtual backing cannot be replaced by dependency imports or runtime services; unaccepted actions cannot run for discovery.
- `build-execution-006` — include_source publishes exact retained UTF-8 staged bytes under generated logical paths, without mutable-path rereads.
- `build-execution-007` — Generated stratum extends the admitted base without changing authored lookup, rerunning Build or rebuilding the retained frontend.
- `build-execution-008` — Final admission covers both strata and direct dependency/public visibility; generated code inherits no generator authority.
- `build-execution-009` — Each dependency hands off one exact compiler-issued bundle, including empty, binding purpose/profile/target/closure/bytes and rejecting mismatched custody.

**Remaining:** No dedicated Build execution row; scheduling, restricted requests, prefix preservation and bundle substitution need integration controls.

<a id="build-external-roots"></a>

#### external roots

**Source:** [wiki/spec/build/external_roots.md](../../spec/build/external_roots.md) · 6 headings · SHA-256 `a4ab9725c0cff7956c67d7f34a7d32344fe0d85a19d6eba6abc94f7e307e8917`.

**Evidence needed:** External-entry ledger, processor startup and installed resource admission.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/entry (2 roots)](../../../tests/omega/pass/entry), [fail/entry (1 roots)](../../../tests/omega/fail/entry), [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host).

**Required families:**

- `build-external-roots-001` — External roots bind exact entry, boundary/provider, artifact, resources, liveness and era pins; installation consumes independent publication authority.
- `build-external-roots-002` — Removal returns authority only after exact unreachability and quiescence, not merely stub emission.
- `build-external-roots-003` — Startup dispatch, confirmed arrival and safe release remain distinct, bound to exact outstanding invocation and installed entry.
- `build-external-roots-004` — Unconfirmed settlement proves no future dispatch and no stack/state execution; delayed receipts cannot resolve another attempt.
- `build-external-roots-005` — Each processor owns disjoint stack/private-state backing and ledger holds entry custody until all attempts retire.
- `build-external-roots-006` — Stack, logical work and machine-state columns require independent exact evidence; work is not WCET or execution permission.
- `build-external-roots-007` — Installation-bound reach substitutes selected operation through complete closure and rejects unresolved rows.
- `build-external-roots-008` — Interruption-fence facts originate in installed-root/provider evidence, never the operation's assertion.

**Remaining:** No dedicated external-root row; startup cancellation/replay, resource separation and physical installation require consumer-ledger fixtures.

<a id="build-foreign-bindings"></a>

#### foreign bindings

**Source:** [wiki/spec/build/foreign_bindings.md](../../spec/build/foreign_bindings.md) · 6 headings · SHA-256 `d12a1f7de9dde70e1497d65cb45d2b15aa26354228f0f070dca1e57b7e9803c7`.

**Evidence needed:** Native locator, satisfaction identity and foreign call realization controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers), [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host).

**Required families:**

- `build-foreign-bindings-001` — ForeignBinding is closed compile-time locator data, not runtime slot/selection/authority; DllImport and Syscall are the evaluated sum.
- `build-foreign-bindings-002` — via admits only irreducible mechanism payload; table identities and intrinsics rejoin exact typed declarations/catalog rather than arbitrary ordinals or text.
- `build-foreign-bindings-003` — Exact target-trait lifetime application supplies every active binder; satisfaction-edge equality partition differs from full signature substitution.
- `build-foreign-bindings-004` — Variant/target validation checks bytes, lengths, versioning, ranges and unused zeros before provider planning.
- `build-foreign-bindings-005` — Normalized locator/evaluation/target identity survives relocations and audit; changed coordinates require relink and fresh admission.
- `build-foreign-bindings-006` — Opaque realizations refine reach, suspension, blocking, crash, invocation and termination ceilings with visible admitted premises.
- `build-foreign-bindings-007` — Detached native calls retain worker/storage/era until actual return; in-process cancellation cannot safely kill a hang.
- `build-foreign-bindings-008` — Returning native imports preserve canonical floating control with correctly ordered save/restore, independently of callback envelopes.
- `build-foreign-bindings-009` — Physical nonreturning calls do not establish canonical semantic ProcessExit.

**Remaining:** No dedicated foreign-locator row; exact lifetime partitions, locator variants, operational contracts and native floating-state envelopes need separate evidence.

<a id="build-foreign-storage"></a>

#### foreign storage

**Source:** [wiki/spec/build/foreign_storage.md](../../spec/build/foreign_storage.md) · 6 headings · SHA-256 `46f069be0ed2ddbffa72f79ce9693e08bf2e069251682af7a855d1f633d09e05`.

**Evidence needed:** Foreign marshaling, retention, provenance and completion controls.

**Fact area candidates:** [returned_write_destination](../../../samples/cli/facts/returned_write_destination).

**Other candidate areas:** [pass/borrows (287 roots)](../../../tests/omega/pass/borrows), [fail/borrows (286 roots)](../../../tests/omega/fail/borrows), [run/borrows (30 roots)](../../../tests/omega/run/borrows), [pending/borrows (1 roots)](../../../tests/omega/pending/borrows), [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host).

**Required families:**

- `build-foreign-storage-001` — addr/Ptr are inert; exact authorized claims plus marshaling establish pointers/views, not ABI policy alone.
- `build-foreign-storage-002` — Call-scoped borrow, retained loan and ownership transfer keep distinct stable-root/range/access/lifetime custody.
- `build-foreign-storage-003` — Retention cannot arise from a borrowed signature; ambiguous consumed-input/result mappings need exact ordinary postconditions.
- `build-foreign-storage-004` — Pointer graphs retain explicit arena/extent roots; unknown provenance rejects without admitted provider establishment.
- `build-foreign-storage-005` — Snapshots need explicit copy semantics and per-occurrence persistent capacity; identity preservation and unchecked writeback are separate.
- `build-foreign-storage-006` — Provider-owned views retain exact invocation provenance and invalidation/stability contracts without inventing input spatial containment.
- `build-foreign-storage-007` — Completion correlates exact live claim/argument/path and commits only after accepted successful return; progress releases only exact subclaims.
- `build-foreign-storage-008` — Provider handles pin their producing era, whereas static slots alone do not.
- `build-foreign-storage-009` — Write-only access requires existing valid contents, no observation and exact unchanged complement, and creates no durable retention.

**Remaining:** Write-only row is narrow. Spec status explicitly leaves retained-argument native backing, installed occurrence settlement and opaque by-value ABI joins open.

<a id="build-hardware-materialization"></a>

#### hardware materialization

**Source:** [wiki/spec/build/hardware_materialization.md](../../spec/build/hardware_materialization.md) · 5 headings · SHA-256 `d9213a652a520cd5ea12cf1866ff500ccc7bf968ed5677bbc61f3e9537bb6ee0`.

**Evidence needed:** Symbolic layout writer, byte validation and publication controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/layouts (90 roots)](../../../tests/omega/pass/layouts), [fail/layouts (57 roots)](../../../tests/omega/fail/layouts), [run/layouts (7 roots)](../../../tests/omega/run/layouts), [pass/asm (19 roots)](../../../tests/omega/pass/asm), [fail/asm (55 roots)](../../../tests/omega/fail/asm).

**Required families:**

- `build-hardware-materialization-001` — Symbolic sources retain consumption phase; loader-consumed fields fit relocation vocabulary while runtime addresses still require range authority.
- `build-hardware-materialization-002` — Materialization binds complete range, alignment, regime and installation scope before consumption.
- `build-hardware-materialization-003` — Writer uses exact mapped pinned writable unpublished placement and restricted sealed resolver, with each target resolved once.
- `build-hardware-materialization-004` — Failed partial bytes establish no consumer value; complete current bytes match exact writer output before atomic visibility.
- `build-hardware-materialization-005` — Consumer semantic validation, materialization authority and installation/publication authority remain distinct.
- `build-hardware-materialization-006` — Post-firmware fault-free claims require backed WCSU, bounded work, valid fragments and no unsupported dynamic/suspend/allocate path, retaining physical-failure assumptions.
- `build-hardware-materialization-007` — Checked privileged instructions require exact consumer authority and full state/clobber/footprint contracts.

**Remaining:** No dedicated hardware row; requires writer corruption, placement and real consumer-publication evidence.

<a id="build-interrupt-obligations"></a>

#### interrupt obligations

**Source:** [wiki/spec/build/interrupt_obligations.md](../../spec/build/interrupt_obligations.md) · 3 headings · SHA-256 `219548f8fabcfef0252a8e928efceb62f00afb5910b41f152270a37bf1ed76e7`.

**Evidence needed:** Installed interrupt token, replay and completion controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/asm (19 roots)](../../../tests/omega/pass/asm), [fail/asm (55 roots)](../../../tests/omega/fail/asm), [pass/domains (81 roots)](../../../tests/omega/pass/domains), [fail/domains (161 roots)](../../../tests/omega/fail/domains), [run/domains (55 roots)](../../../tests/omega/run/domains), [pending/domains (1 roots)](../../../tests/omega/pending/domains).

**Required families:**

- `build-interrupt-obligations-001` — Mask guards and acknowledgements are independent opaque linear data with exact target-closed representations.
- `build-interrupt-obligations-002` — Provider evidence binds installed root/invocation/state/token lineage; compact matches and replay cannot substitute occurrences.
- `build-interrupt-obligations-003` — Nested mask restoration consumes only newest state; deriver exit requires exact restored mask and completed acknowledgement.
- `build-interrupt-obligations-004` — Pending qualification enters through exact authorized semantic/ABI parameter occurrence, not an authored entry marker.
- `build-interrupt-obligations-005` — Entry and completion reach remain distinct and installation resolves every bounded dependency.
- `build-interrupt-obligations-006` — Deferred acknowledgement retains root/controller lease until actual completion without revocable pins.

**Remaining:** No dedicated interrupt row; semantic token checks and actual installed occurrence/completion evidence are separate.

<a id="build-machine-state-evidence"></a>

#### machine state evidence

**Source:** [wiki/spec/build/machine_state_evidence.md](../../spec/build/machine_state_evidence.md) · 4 headings · SHA-256 `29204046ce2d666d6e6bcebeb757ae5a2ce1ab8e21711adacb690f3399886d72`.

**Evidence needed:** Complete final instruction/region footprint replay.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/asm (19 roots)](../../../tests/omega/pass/asm), [fail/asm (55 roots)](../../../tests/omega/fail/asm), [pass/register_allocation (2 roots)](../../../tests/omega/pass/register_allocation).

**Required families:**

- `build-machine-state-evidence-001` — Final artifact after allocation, inlining, thunks and stubs stays within published StatePlan ceiling.
- `build-machine-state-evidence-002` — One exact certificate binds final bytes, placements and complete executable-region inventory with full byte coverage.
- `build-machine-state-evidence-003` — Each byte-bearing instruction has exactly one compiler-spec or assembly-catalog replay authority and footprint row.
- `build-machine-state-evidence-004` — Raw leaves retain explicit admitted provider claims distinct from derived Omega evidence.
- `build-machine-state-evidence-005` — Counts, normalized state union and certificate identity agree; unsupported instructions cannot disappear from the union.
- `build-machine-state-evidence-006` — Return control and restored state match exact call/state plans before provider execution forms.

**Remaining:** No source row can establish complete final-byte inventory; contract explicitly does not claim current native implementation complete.

<a id="build-macos-application"></a>

#### macos application

**Source:** [wiki/spec/build/macos_application.md](../../spec/build/macos_application.md) · 6 headings · SHA-256 `7967f8c34c2764598aaa4679e593e70a61121bdf14a5b10d165d8e45392da341`.

**Evidence needed:** Whole application package publication and tamper controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build).

**Required families:**

- `build-macos-application-001` — Selected macOS GUI intent requires one complete bundle, console remains flat and other targets have independent outcomes.
- `build-macos-application-002` — One validated name supplies bundle/executable names; exact authored nonempty ASCII identifier is required before GUI signing.
- `build-macos-application-003` — Identifier affects signed realization, while display/destination metadata affects publication only.
- `build-macos-application-004` — Deterministic v1 package contains exact executable/plist tree and supports cross-invocation publication without frontend recovery.
- `build-macos-application-005` — Stage and replay complete shape/bytes; missing/extra/tampered/partial contents reject.
- `build-macos-application-006` — Checked package-root and inner-executable paths have one verified relationship and are unavailable on failed publication.
- `build-macos-application-007` — Ad-hoc executable signing is not distribution authenticity; resource delivery and Finder behavior need actual runtime evidence.

**Remaining:** No dedicated package row; spec labels publication implementation as a task and source inspection as insufficient runtime evidence.

<a id="build-observations"></a>

#### observations

**Source:** [wiki/spec/build/observations.md](../../spec/build/observations.md) · 4 headings · SHA-256 `db44513c826d18a0ef68c6de309e58fd9bf441623177e9fab502f95fb41c68dd`.

**Evidence needed:** Single Build execution and complete output tree custody.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [omega-rust/omega/packages/build-output/src/captured_source/tests.rs](../../../omega-rust/omega/packages/build-output/src/captured_source/tests.rs).

**Required families:**

- `build-observations-001` — Build uses exact accepted lock inputs and attenuated root-selected backing without silent renewal or real-host fallback.
- `build-observations-002` — Each admitted activation executes once; operation playback is not an admission/reproducibility mechanism.
- `build-observations-003` — Retained observations preserve attempted results, exact immutable inputs, source handoffs and completed outputs without inventing missing success.
- `build-observations-004` — After teardown retain complete canonical output tree, modes, empty directories, links, content and unique-content accounting.
- `build-observations-005` — Materialization into empty destination re-inspects exact tree and bytes; generated handoffs are explicit closed-file subsets with valid attempt order.
- `build-observations-006` — Disk backing requires real race-safe isolation or an explicit sufficient executor premise; canonical path checks alone do not suffice.

**Remaining:** No dedicated Build-observation row; complete-tree and race/isolation evidence differ from source fact delivery.

<a id="build-opaque-representations"></a>

#### opaque representations

**Source:** [wiki/spec/build/opaque_representations.md](../../spec/build/opaque_representations.md) · 4 headings · SHA-256 `2c87ddf46aefaaa43172249aaef1752d03f0a4caab805ca12adbdf4a4fc18db3`.

**Evidence needed:** Exact by-value representation, lifecycle and composition controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [pass/calling (1 roots)](../../../tests/omega/pass/calling), [fail/calling (2 roots)](../../../tests/omega/fail/calling).

**Required families:**

- `build-opaque-representations-001` — Original Build selects one exact named compiler-owned representation conformance; descriptor geometry is derived rather than authored.
- `build-opaque-representations-002` — By-value demand closes before calling-policy evaluation; unbound reference/proof-only uses create no demand.
- `build-opaque-representations-003` — Invalid or conflicting explicit choices reject even unused; historical dependency selections do not become current root authority.
- `build-opaque-representations-004` — Inert v1 lifecycle checks complete closed carrier graph including inactive cases; copyable opaque requires copyable inert representation.
- `build-opaque-representations-005` — Physical byte moves preserve one affine/linear occurrence and cannot create semantic copies.
- `build-opaque-representations-006` — Composition compares strong exact application identity at actual edges, retaining declaration/source/target/shape/movement/lifecycle/evidence origin.
- `build-opaque-representations-007` — Source occurrence is audit provenance rather than ABI identity; cleanup-owning carriers require a separate accepted lifecycle relationship.

**Remaining:** No dedicated opaque row; inert-graph, copy and cross-artifact mismatch controls remain separate from ABI geometry tests.

<a id="build-optimizations"></a>

#### optimizations

**Source:** [wiki/spec/build/optimizations.md](../../spec/build/optimizations.md) · 8 headings · SHA-256 `04651d6ea32f6ee8e66eed911af63c0da0152a79cc62780880158f7a448de7d6`.

**Evidence needed:** Transformation preconditions, independent validation, rollback and differential controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/optimizer (30 roots)](../../../tests/omega/pass/optimizer), [fail/optimizer (2 roots)](../../../tests/omega/fail/optimizer).

**Required families:**

- `build-optimizations-001` — Exact root selection is empty by default, phase-owned and canonical; unknown rules/versions/targets/compositions reject.
- `build-optimizations-002` — Every phase validates input/output and each applied transformation; empty selection executes the same identity phase.
- `build-optimizations-003` — All authored source checks before pruning; published Terminal retains exact selection/input/output and consumers do not rerun Psi passes.
- `build-optimizations-004` — Preserve values, traps/order, services, atomics, cleanup, ABI, loans, address stability and required provenance without policy/fast-math substitution.
- `build-optimizations-005` — Logical fuel differs from native cost; final spill-inclusive frame/state evidence binds exact optimized realization.
- `build-optimizations-006` — Facts/qualifications retain exact identity, lineage and every-arrival availability; CSE cannot invent or silently narrow membership while claiming equivalence.
- `build-optimizations-007` — Cyclic/rank/custody analyses retain exact topology and invalidate or recompute stale dependencies.
- `build-optimizations-008` — Candidates are immutable; independent validator reconstructs preconditions on fresh input before commit.
- `build-optimizations-009` — Decision policy chooses only listed validated candidates, with exact context/manifest/budget and deterministic bounded fallback.
- `build-optimizations-010` — Disable overlay subtracts exact unique names before work, preserves authored identity and records applied/effective/rollback provenance.
- `build-optimizations-011` — Promotion needs separate owner decision and semantic/corruption/differential/determinism/target/cost/rollback evidence; no learned optimizer is currently required.

**Remaining:** No dedicated optimization row; fact sources can supply inputs, but each rule needs exact transformation/corruption and realization evidence.

<a id="build-permissions"></a>

#### permissions

**Source:** [wiki/spec/build/permissions.md](../../spec/build/permissions.md) · 8 headings · SHA-256 `69641b7db1216c0fd329dd706d4eca1c635db51f73a820e5c7a7a21562aac92e`.

**Evidence needed:** Mechanism classification, receiver-policy replay and bounded lifecycle proofs.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host), [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers), [pass/effects (14 roots)](../../../tests/omega/pass/effects), [fail/effects (13 roots)](../../../tests/omega/fail/effects).

**Required families:**

- `build-permissions-001` — Abstract reach, invocation authority and executable trust are separate; ordinary artifact production requires no invented receiver policy.
- `build-permissions-002` — Every demanded terminal mechanism has one exact explicit classification, including empty, with unknown/missing/duplicate/cyclic/substituted leaves rejected.
- `build-permissions-003` — Role-tagged intrinsic/syscall/foreign/table/instruction identities include full checked argument contracts and strong plan identity where required.
- `build-permissions-004` — Classification covers every permitted runtime value; narrower classes require occurrence-specific checked constraints.
- `build-permissions-005` — Consumer permissions are independent inputs; project acceptance, risk labels and classification cannot grant receiver execution authority.
- `build-permissions-006` — Filesystem facets classify operations rather than objects or confinement, and deferred deletion/durability attribution preserves actual protocols.
- `build-permissions-007` — Narrow ordinary-release proofs require successful constrained acquisition, preservation through every call, one actual release and no alias escape/reuse.
- `build-permissions-008` — Changed arguments/flow/contracts invalidate narrowed claims; unsupported unbounded cases cannot fabricate a broad union.
- `build-permissions-009` — MachineControl and PortIo are exact services; independent hosted grants and machine-owner freestanding authority retain instruction-specific obligations.
- `build-permissions-010` — Native inventory/containment remains explicit; lowering safe Terminal does not admit supplied native bytes.

**Remaining:** No dedicated permission row; classification/schema and flow-proof corruption controls need separate qualification, not only service-name fixtures.

<a id="build-platform-services"></a>

#### platform services

**Source:** [wiki/spec/build/platform_services.md](../../spec/build/platform_services.md) · 5 headings · SHA-256 `08c17de81865bd0616b4df609ccd8b261a0941c66f4c13971a3917d07f02eff4`.

**Evidence needed:** Portable/raw service layering and provider closure controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/filesystem (33 roots)](../../../tests/omega/pass/filesystem), [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers).

**Required families:**

- `build-platform-services-001` — Portable API stays identical across targets; each raw family has complete own schema/handles without cross-family sentinel rows.
- `build-platform-services-002` — Selected portable provider pins exact transitive raw-family requirement/provider closure and re-admits changes.
- `build-platform-services-003` — Closure adds no source routing fields, private raw nameability or ambient authority.
- `build-platform-services-004` — Missing/unsupported target closure rejects rather than selecting all families or altering portable carriers.
- `build-platform-services-005` — Raw service reach is explicitly declared and physical classification stays independent of interface grouping.
- `build-platform-services-006` — Portable zero shapes mean absent handle/default options/failure result; unsupported operations return portable Unsupported rather than raw stubs.

**Remaining:** No dedicated portable-service row; exact target closure, missing family and portable zero-result controls are needed.

<a id="build-private-callbacks"></a>

#### private callbacks

**Source:** [wiki/spec/build/private_callbacks.md](../../spec/build/private_callbacks.md) · 6 headings · SHA-256 `ff2c2c0a9b3ca8bb2f7f44a87175d530997c13a4d0d87d0e5b2bd30f81904fc9`.

**Evidence needed:** Callback selection, materialization, registration and lifetime controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/calls (289 roots)](../../../tests/omega/pass/calls), [fail/calls (117 roots)](../../../tests/omega/fail/calls), [run/calls (28 roots)](../../../tests/omega/run/calls), [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host).

**Required families:**

- `build-private-callbacks-001` — Static binder selects exact requirement overload and nominal machine; signature coincidence or address observation cannot select callbacks.
- `build-private-callbacks-002` — Native-only direct callback entries retain actual interleaved positions without semantic runtime operands or implicit environments.
- `build-private-callbacks-003` — Private field slots belong to selected exact layout conformance and are each supplied once with complete plan identity.
- `build-private-callbacks-004` — Inbound callback and outbound registrar plans remain distinct and rejoin exact binder/destination/private function in order.
- `build-private-callbacks-005` — Receiving validation needs authenticated application plus authored-use correspondence, not a producer digest alone.
- `build-private-callbacks-006` — Code emission proves neither registration nor capacity/lifetime authority; success transfers exact registration capacity and failure preserves it.
- `build-private-callbacks-007` — Unregister plus quiescence precedes code/era release; call-scoped borrows create no durable roots.
- `build-private-callbacks-008` — Synchronous invokes graph is acyclic and independent of future roots; omitted invokes is no synchronous entry.
- `build-private-callbacks-009` — Opaque re-entry assumptions and selected actual envelopes remain visible without becoming ambient caller facts.

**Remaining:** No dedicated callback row; source identity, physical materialization, actual registration and quiescence need distinct fixtures/receipts.

<a id="build-provider-selection"></a>

#### provider selection

**Source:** [wiki/spec/build/provider_selection.md](../../spec/build/provider_selection.md) · 6 headings · SHA-256 `c31891e9040ab6671507ef68c50ad84e80925f3d059584279536308ad4685244`.

**Evidence needed:** Exact provider conformance, complete-set selection and executable trust controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/providers (133 roots)](../../../tests/omega/pass/providers), [fail/providers (94 roots)](../../../tests/omega/fail/providers), [run/providers (2 roots)](../../../tests/omega/run/providers), [pass/traits (92 roots)](../../../tests/omega/pass/traits), [fail/traits (117 roots)](../../../tests/omega/fail/traits), [run/traits (3 roots)](../../../tests/omega/run/traits).

**Required families:**

- `build-provider-selection-001` — Build chooses declared exact named candidates/defaults; selection cannot author plans, source conformance evidence, token binders or runtime authority.
- `build-provider-selection-002` — Original activation Build identity is required for select/accept operations; lookalike names and receiver syntax grant no special action.
- `build-provider-selection-003` — Provider application binds exact package/slot/requirement/schema/realizer/target and complete immutable canonical selected set.
- `build-provider-selection-004` — Checked ordinary bodies may implement toolchain-owned constant answers without a special constant provider kind.
- `build-provider-selection-005` — Candidate derivation covers full schema and inherited receiver application, preserving phantom/region arguments and selecting owner.
- `build-provider-selection-006` — Transitive raw/provider closure rejects missing, cyclic, ambiguous, cross-target or unadmitted dependencies.
- `build-provider-selection-007` — Operator-family override covers every canonical overload atomically; closed specialization and target realization remain separate from symbolic coverage.
- `build-provider-selection-008` — Universal generic coverage is explicitly unimplemented/fail-closed; no concrete case or external claim supplies it.
- `build-provider-selection-009` — Routed claims establish only through exact invocation receipts, not schema/type displays.
- `build-provider-selection-010` — Executable trust reports derived versus admitted per fact, exact selected code and attributed completeness; containment guarantees remain independent.
- `build-provider-selection-011` — Runtime executable ledger rejects replay and cannot erase unrelated incompleteness; isolated child manifests bind exact parent endpoint/scope.

**Remaining:** No dedicated provider row; complete conformance/selection, generic-demand and dynamic trust-manifest evidence needs owner-specific suites.

<a id="build-scoped-execution"></a>

#### scoped execution

**Source:** [wiki/spec/build/scoped_execution.md](../../spec/build/scoped_execution.md) · 13 headings · SHA-256 `4470ee13d3d53e1cd5a61551dc86c43c9ee0f5368c8a4351d33aa047bb8a264b`.

**Evidence needed:** Purpose-scoped Build integration, snapshot protocol and output transactions.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build).

**Required families:**

- `build-scoped-execution-001` — Product/build dependencies have separate aliases and checked instances; acquisition reuse never grants cross-context nameability or authority.
- `build-scoped-execution-002` — Awaited pre-known requirements install admitted static slots and resume exact continuation without retroactive imports or replay.
- `build-scoped-execution-003` — Scheduling keys include package/purpose/execution profile/product target and detect newly introduced cycles before binding and final join.
- `build-scoped-execution-004` — Designated product operands resolve exact authored frontier under lexical owner scope without executing product code or exposing private siblings.
- `build-scoped-execution-005` — Role-specific product descriptions retain exact target/live activation and cannot become callables, grants, raw handles or unfinished layout queries.
- `build-scoped-execution-006` — Every activation receives explicit immutable captured inventory, empty private staged output and bounded diagnostics, with no ambient host services.
- `build-scoped-execution-007` — Logical paths reject traversal/normalization/host ambiguity; symlinks are inert and special host files cannot become captured source.
- `build-scoped-execution-008` — Reads have deterministic EOF/range semantics; staging is append-and-seal with one writer, explicit errors and no fake successful I/O.
- `build-scoped-execution-009` — Source capture is coherent and pre-execution; exact optional absence and independent control-state exclusion are retained.
- `build-scoped-execution-010` — Per-dependency immutable input maps are fixed before execution and must agree completely for shared nodes; union would widen grants.
- `build-scoped-execution-011` — Cache identity includes complete inventories, purposes, policy and executor semantics; hits do not bypass fresh eligibility.
- `build-scoped-execution-012` — Restricted host work requires independent acceptance plus actual scoped grants; transitional live filesystem modes cannot claim snapshot isolation.
- `build-scoped-execution-013` — RequiredOutput is activation-bound linear completion, with exact sealed current bytes, collision rejection and nonforgable executor records. Candidate: `build-seal`.
- `build-scoped-execution-014` — Failure/incomplete obligations publish no new successful set; complete immutable manifests commit only after source/tests/product checks, with per-target outcomes. Candidate: `build-seal`.
- `build-scoped-execution-015` — Artifact-only application requires explicit target, completed file and no executable selections; package artifact formats/meaning remain ordinary code.
- `build-scoped-execution-016` — Verified descriptions from completed components may feed separate composition builds; current Build cannot inspect or rewrite its own unfinished product.
- `build-scoped-execution-017` — Acceptance requires actual multi-file acquisition/generation/isolation/failure tests on Windows and macOS; deferred caches/plugins/generated-root queries are not required features.

**Remaining:** No dedicated scoped-build row; 27-case source acceptance table and platform isolation/output lifecycle need integration evidence, not dependency-parser tests alone.

<a id="build-task-runtime"></a>

#### task runtime

**Source:** [wiki/spec/build/task_runtime.md](../../spec/build/task_runtime.md) · 8 headings · SHA-256 `9702e1fecb18b737fec843b444328a49a2abda54b44acb2a61ebb956f43e5577`.

**Evidence needed:** Task description, transactional start, carry and linear runtime lifecycle.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/tasks (2 roots)](../../../tests/omega/pass/tasks), [fail/tasks (3 roots)](../../../tests/omega/fail/tasks), [pass/concurrency (13 roots)](../../../tests/omega/pass/concurrency), [fail/concurrency (7 roots)](../../../tests/omega/fail/concurrency).

**Required families:**

- `build-task-runtime-001` — Named machine starts are exact static selections under a held runtime capability, not capture inference or ambient spawn.
- `build-task-runtime-002` — Activation description binds entry/layout/calling/WCSU/carry/cancellation and exact selected start requirement/strong specialization identity.
- `build-task-runtime-003` — Each local activation gets backed fixed nonmoving stack before execution and retains it across park; reports are not invocation receipts.
- `build-task-runtime-004` — Start rejection returns every moved argument/reservation, and successful pending task requires actual compatible provider custody.
- `build-task-runtime-005` — Task is linear through records/cases; cancel requests retain claim and cleanup cannot implicitly join/detach/discard it.
- `build-task-runtime-006` — Runtime-instance ledger correlates exact activation and lease era; cross-instance or replay settlement preserves failed claim.
- `build-task-runtime-007` — Suspension continues same invocation with no result/cleanup; cancellation is cooperative and hardware preemption/blocking are not semantic safe points.
- `build-task-runtime-008` — Live loans/carry require exact pinning, alias, outcome and CPU/thread/address preservation evidence.
- `build-task-runtime-009` — Foreign same-stack demand versus separately provisioned stacks and inbound callback plans retain independent obligations.

**Remaining:** No dedicated task row; static task-plan accounting does not prove transactional execution, cancellation or complete runtime custody.

<a id="build-testing"></a>

#### testing

**Source:** [wiki/spec/build/testing.md](../../spec/build/testing.md) · 10 headings · SHA-256 `c59e1809c9a75ae3ee208a103d1ee766314cd8e18a82a414965f8c18a4a5485e`.

**Evidence needed:** Requirement discovery, test admission, dual-engine execution and publication integration.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build).

**Required families:**

- `build-testing-001` — Executed registration selects one exact ordinary requirement application and package/target group, without privileged names or duplicate groups.
- `build-testing-002` — Discover every concrete local exact satisfier including generated bodies, but no open generics or imported tests; zero tests is not pass evidence.
- `build-testing-003` — Entry requirements/preconditions, provisioned context and exact provider closure obey ordinary contracts and authority.
- `build-testing-004` — Groups default enabled; disabled/unavailable states remain distinct from passes and cannot escape delegated virtual backing.
- `build-testing-005` — Default schedules every applicable permitted available engine; explicit requirements and at-least-one execution gate successful publication.
- `build-testing-006` — Publish exact deterministic plan before effects and report separate per-engine outcomes with fresh local state and matching observations.
- `build-testing-007` — Failure/exhaustion cannot be relabeled optional unavailability; native execution adds no authority and fuel proves no termination.
- `build-testing-008` — Automatic tests cannot use assumed runtime links through any helper/provider/evidence path.
- `build-testing-009` — Mandatory verdict checks cannot be replaced by no-op assertions or rely on the disputed guarantee.
- `build-testing-010` — Test-only roots/images remain separate from application products while ordinary called test code remains in application closure.
- `build-testing-011` — Corpus pass/fail/run semantics and matching native/interpreted results retain exact diagnostics/oracles.
- `build-testing-012` — Acceptance runs real pure and virtual-filesystem groups on both engines, including unavailable engines, escape attempts and application-root exclusion.

**Remaining:** No dedicated group row; existing corpus execution is not evidence that automatic Build registration/admission/publication orchestration is complete.

<a id="build-uefi-entry"></a>

#### uefi entry

**Source:** [wiki/spec/build/uefi_entry.md](../../spec/build/uefi_entry.md) · 6 headings · SHA-256 `7be905edcfcbb74dcd8102aaf050ac90c88132b9cc30c03dd27d09e3d9db0ae9`.

**Evidence needed:** Source-owned firmware adapter, exact storage roots and handoff lifecycle.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/host (54 roots)](../../../tests/omega/pass/host), [fail/host (5 roots)](../../../tests/omega/fail/host), [pass/entry (2 roots)](../../../tests/omega/pass/entry), [fail/entry (1 roots)](../../../tests/omega/fail/entry).

**Required families:**

- `build-uefi-entry-001` — Firmware definitions/policies are source-owned checked plans; replay establishes actual source/plan/realization correspondence rather than copied Rust policy.
- `build-uefi-entry-002` — Physical arrival and semantic continuation have distinct exact schemas; visible source receives only declared image/storage roots.
- `build-uefi-entry-003` — Both geometry obligations hold before either root introduction; failure preserves moved bootstrap custody and calls no continuation.
- `build-uefi-entry-004` — Initial storage is disjoint, actual stack/context partitions cannot be forwarded whole and image sections mint no fresh roots.
- `build-uefi-entry-005` — Canonical 128KiB/16-byte profile stack minimum remains separate from actual invocation backing and full same-stack demand.
- `build-uefi-entry-006` — Returning firmware application and nonreturning OS handoff have distinct lifecycle contracts.
- `build-uefi-entry-007` — Source GetMemoryMap/ExitBootServices retry retains exact snapshot/key/stride/attempts and transitions Boot Services/allocations only on actual success.
- `build-uefi-entry-008` — Post-exit memory claims require final-map and policy evidence; surviving handoff stack is explicitly accounted.
- `build-uefi-entry-009` — Critical hardware preparation/publication retains bounded software-fault-free evidence and explicit NMI/physical assumptions.
- `build-uefi-entry-010` — Reports preserve exact providers, environment, root scopes, installation, footprints and remaining debts without guessing installed ledgers.

**Remaining:** No dedicated UEFI row; bounded migration entrances and compiler-specific retry realization do not close actual source-owned boot/handoff acceptance.

<a id="area-packages"></a>

### packages

<a id="packages-acceptance"></a>

#### acceptance

**Source:** [wiki/spec/packages/acceptance.md](../../spec/packages/acceptance.md) · 8 headings · SHA-256 `b120acae8b97cc0fa0433da66f4f12b3875e46df83281e127212cd2601955d42`.

**Evidence needed:** Exact project acceptance, review transaction and locked compilation controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/packages (2 roots)](../../../tests/omega/pass/packages), [fail/packages (3 roots)](../../../tests/omega/fail/packages), [pass/modules (66 roots)](../../../tests/omega/pass/modules), [fail/modules (47 roots)](../../../tests/omega/fail/modules), [pending/modules (1 roots)](../../../tests/omega/pending/modules), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [omega-rust/omega/packages/source-acquisition/src/identity/tests.rs](../../../omega-rust/omega/packages/source-acquisition/src/identity/tests.rs), [omega-rust/omega/packages/build-declarations/src/tests/mod.rs](../../../omega-rust/omega/packages/build-declarations/src/tests/mod.rs).

**Required families:**

- `packages-acceptance-001` — Ordinary compilation matches every occurrence's source/compiler identity and exact Product/Build context, rejecting stale locks without inline review or renewal.
- `packages-acceptance-002` — Absent lock permits development through ordinary package route without inventing grants; malformed or missing sections never fall back to unlocked mode.
- `packages-acceptance-003` — Explicit review derives complete candidate findings and compares readable risk meanings; fully checked advisory-only changes need no new approval.
- `packages-acceptance-004` — Review resolver/compiler rows join bijectively by exact source/purpose/profile/target and bound hostile source packets independently.
- `packages-acceptance-005` — Assumed links retain extremely-high-risk exact contradictions/subjects/provider/dependencies and require root authorization plus project acceptance.
- `packages-acceptance-006` — Restricted Build/test requests are reviewed before effects, separately from runtime permission and actual scoped executor grants.
- `packages-acceptance-007` — Unknown/missing authority or invalid proofs cannot become empty risk rows or be approved away.
- `packages-acceptance-008` — Resolution-only changes preserve exact meaning without adding grants; compiler/schema changes require applicable fresh reconstruction.
- `packages-acceptance-009` — Decisions bind complete comparison and permit only pending-to-accept/reject; blanket accept is explicit invocation-only through same checks.
- `packages-acceptance-010` — Recoverable declaration/lock publication preserves original lineage/base, checks all targets, stops on unrelated edits and retains pending state.

**Remaining:** No dedicated acceptance row; transaction, stale-decision, restricted-resume and exact permission joins need package integration controls.

<a id="packages-boundaries"></a>

#### boundaries

**Source:** [wiki/spec/packages/boundaries.md](../../spec/packages/boundaries.md) · 4 headings · SHA-256 `10cff77088890f39b570ef62f73322d3bf7b995fe82153a80e97f65c2b8d4bb2`.

**Evidence needed:** Authored selection, carried semantic dependencies and pre-execution admission.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/packages (2 roots)](../../../tests/omega/pass/packages), [fail/packages (3 roots)](../../../tests/omega/fail/packages), [pass/modules (66 roots)](../../../tests/omega/pass/modules), [fail/modules (47 roots)](../../../tests/omega/fail/modules), [pending/modules (1 roots)](../../../tests/omega/pending/modules), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [omega-rust/omega/packages/source-acquisition/src/identity/tests.rs](../../../omega-rust/omega/packages/source-acquisition/src/identity/tests.rs), [omega-rust/omega/packages/build-declarations/src/tests/mod.rs](../../../omega-rust/omega/packages/build-declarations/src/tests/mod.rs).

**Required families:**

- `packages-boundaries-001` — Direct dependency authorizes authored declaration selection; merely carrying a foreign type/qualification/cleanup descriptor does not expose owner members.
- `packages-boundaries-002` — Reflection and generic helpers use lexical author's dependency/visibility scope rather than caller privilege.
- `packages-boundaries-003` — Public interfaces cannot expose private selections except exact evidence-only authorized issuer references.
- `packages-boundaries-004` — Conformance/satisfaction/domain routes retain exact complete owner/application and each authored occurrence before supply admission.
- `packages-boundaries-005` — Calls, operators, fields, constants and static arguments preserve independent source selections through normalization.
- `packages-boundaries-006` — Admit exact targets or complete candidate-owner sets before effect-free execution, never after executing.
- `packages-boundaries-007` — Carried semantic dependency and authored lookup remain separate facts; packages are statically composable rather than implicit replacement units.

**Remaining:** No dedicated package-boundary row; transitive carried-type versus authored lookup and evaluation-before-admission need positive/reject pairs.

<a id="packages-locks"></a>

#### locks

**Source:** [wiki/spec/packages/locks.md](../../spec/packages/locks.md) · 4 headings · SHA-256 `9a0c13195ec2adf8fc1287a966e8b4923d75840f52eb08a6e1d520f9cc2438f7`.

**Evidence needed:** Canonical lock meaning, invalidation and source/control-state controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/packages (2 roots)](../../../tests/omega/pass/packages), [fail/packages (3 roots)](../../../tests/omega/fail/packages), [pass/modules (66 roots)](../../../tests/omega/pass/modules), [fail/modules (47 roots)](../../../tests/omega/fail/modules), [pending/modules (1 roots)](../../../tests/omega/pending/modules), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [omega-rust/omega/packages/source-acquisition/src/identity/tests.rs](../../../omega-rust/omega/packages/source-acquisition/src/identity/tests.rs), [omega-rust/omega/packages/build-declarations/src/tests/mod.rs](../../../omega-rust/omega/packages/build-declarations/src/tests/mod.rs).

**Required families:**

- `packages-locks-001` — Lock retains exact graph/resolutions/root role/purposes plus readable normalized risk/acceptance and compiler-bound semantic inputs, not a safety certificate.
- `packages-locks-002` — Assumed/restricted/critical-policy rows retain exact contextual meanings without secrets, host capabilities or duplicate admission databases.
- `packages-locks-003` — Full API/provider/representation/source graphs remain fresh review data rather than copied lock payload.
- `packages-locks-004` — Version 4 outer and version 3 acceptance schemas reject unknown/old formats without guessed migration or empty acceptance.
- `packages-locks-005` — Mutable local root lock/control state is excluded from source identity with case variants and smuggling links rejected; materialized trees retain exact files.
- `packages-locks-006` — Exact target section is selected before acquisition and locked selectors never refresh; another target's acceptance cannot authorize current use.
- `packages-locks-007` — Every accepted source/compiler identity, including edited local root, must match; mismatch directs explicit update.
- `packages-locks-008` — Absent lock grants nothing but permits development; unreadable/stale/obsolete locks cannot become absent.
- `packages-locks-009` — Compiler/toolchain input identity is an invalidation key rather than attestation, and fresh review compares complete row meaning.

**Remaining:** No dedicated lock row; codec, target-before-network, local edits, purpose migration and acceptance invalidation require package tests.

<a id="packages-resolver-security"></a>

#### resolver security

**Source:** [wiki/spec/packages/resolver_security.md](../../spec/packages/resolver_security.md) · 12 headings · SHA-256 `f01de7004483803c36b9a1f126b8e475193b57527b5b6f2fd55d3b3ff625e070`.

**Evidence needed:** Source acquisition adversarial trees, process limits and immutable custody.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/packages (2 roots)](../../../tests/omega/pass/packages), [fail/packages (3 roots)](../../../tests/omega/fail/packages), [pass/modules (66 roots)](../../../tests/omega/pass/modules), [fail/modules (47 roots)](../../../tests/omega/fail/modules), [pending/modules (1 roots)](../../../tests/omega/pending/modules), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [omega-rust/omega/packages/source-acquisition/src/git/tests/validation.rs](../../../omega-rust/omega/packages/source-acquisition/src/git/tests/validation.rs), [omega-rust/omega/packages/source-acquisition/src/local/tests/capture.rs](../../../omega-rust/omega/packages/source-acquisition/src/local/tests/capture.rs), [omega-rust/omega/packages/source-acquisition/src/identity/tests.rs](../../../omega-rust/omega/packages/source-acquisition/src/identity/tests.rs), [omega-rust/omega/packages/build-declarations/src/tests/mod.rs](../../../omega-rust/omega/packages/build-declarations/src/tests/mod.rs).

**Required families:**

- `packages-resolver-security-001` — Downloaded source never executes during resolution and receives no resolver credentials/environment/transport authority.
- `packages-resolver-security-002` — Closed HTTPS/SSH locator policy rejects secrets and package-controlled protocol/redirect/hook/filter/submodule substitution.
- `packages-resolver-security-003` — Operator-selected primary Git path freezes once, rejects package-controlled/relative lookup and unsupported wrapper decoding.
- `packages-resolver-security-004` — Host Git/SSH authentication/routing remains host policy and does not alter authored canonical lineage or mint source authority.
- `packages-resolver-security-005` — Fresh quarantine verifies exact commit/tree/object graph and materializes without checkout; parent independently validates success, bounds and snapshot before publication.
- `packages-resolver-security-006` — Named members may leave unrelated inert gitlinks outside selection, but selected source/declaration traversal cannot acquire submodules.
- `packages-resolver-security-007` — Recorded pins never resolve mutable selectors; offline has no transport, corruption is not absence and failed fetch is not automatic cache corruption.
- `packages-resolver-security-008` — Missing source.identity cannot manufacture live custody; exact stored bytes require full object/snapshot/bound revalidation.
- `packages-resolver-security-009` — Local capture retains coherent immutable bytes under quiescent-tree premise, detects observed drift and excludes exact mutable control reservations.
- `packages-resolver-security-010` — Replacement snapshots retain before/proposed identities and enforce expected digest/modes/ceilings without pretending proposed bytes are already live.
- `packages-resolver-security-011` — Compiler limits cover entries/bytes/depth/commands/output/cache and honest platform process-tree lifecycle, without claiming hostile same-user confinement.
- `packages-resolver-security-012` — Source identity contains resolved facts, not self-issued Git process receipts or host executable hashes.

**Remaining:** No dedicated resolver row; malformed object/tree, drift, warm/offline cache and process cleanup controls are appropriate evidence.

<a id="packages-review"></a>

#### review

**Source:** [wiki/spec/packages/review.md](../../spec/packages/review.md) · 8 headings · SHA-256 `810be19ff7c651b885327f79699c91d0b438f58605c9d88b81bdbd45f32c9ea6`.

**Evidence needed:** Complete compiler findings, provenance, critical policy and trust projection.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/packages (2 roots)](../../../tests/omega/pass/packages), [fail/packages (3 roots)](../../../tests/omega/fail/packages), [pass/modules (66 roots)](../../../tests/omega/pass/modules), [fail/modules (47 roots)](../../../tests/omega/fail/modules), [pending/modules (1 roots)](../../../tests/omega/pending/modules), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [omega-rust/omega/packages/source-acquisition/src/identity/tests.rs](../../../omega-rust/omega/packages/source-acquisition/src/identity/tests.rs), [omega-rust/omega/packages/build-declarations/src/tests/mod.rs](../../../omega-rust/omega/packages/build-declarations/src/tests/mod.rs).

**Required families:**

- `packages-review-001` — Full review runs only in explicit workflows; ordinary compile uses exact accepted source/compiler inputs.
- `packages-review-002` — Complete checked findings preserve assumptions, all reachable authority and build/test requests; unknowns reject instead of becoming empty rows.
- `packages-review-003` — Use earliest coherent owner representation for each fact and join after successful checking without introducing a second program stage.
- `packages-review-004` — Core-critical identities and root-added custom services affect highlighting/acceptance, not effect existence or runtime authority.
- `packages-review-005` — Root audit additions union monotonically under original Build identity; policy changes invalidate prior reuse.
- `packages-review-006` — Semantic identity preserves exact package owner, canonical constants, complete telescopes/contracts/qualifications and no legacy conformance subject.
- `packages-review-007` — Exact byte spans explain source custody but do not define semantic identity; generated/derived origins cannot be invented.
- `packages-review-008` — Published ceilings, actual body reach, preselection base, bodyless claims and unused external executable supplies are distinct findings.
- `packages-review-009` — Terminal/native evidence is required only for claims about those products; generic completeness flags cannot replace per-fact evidence.
- `packages-review-010` — Unproved contracts reject before review and accepted axioms remain assumptions; engine-local search stand-down is not final proof failure.
- `packages-review-011` — Embedded compiler input plus exact selected library content invalidates stale review/cache, without attesting loaded image or audit quality.
- `packages-review-012` — Public target-dependent applications retain exact dependency; private changes instead force content rebuild/relink.

**Remaining:** No dedicated review row. Its unlocked-native refusal wording needs reconciliation with acceptance/locks allowing absent-lock native development; do not resolve via implementation inference.

<a id="packages-sources"></a>

#### sources

**Source:** [wiki/spec/packages/sources.md](../../spec/packages/sources.md) · 5 headings · SHA-256 `10b62550e4ad2cc7123d9d3a65dfc9461cfb932bff5b58f2facc2dde602bf686`.

**Evidence needed:** Requester graph, immutable resolution, source capture and selective update.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/packages (2 roots)](../../../tests/omega/pass/packages), [fail/packages (3 roots)](../../../tests/omega/fail/packages), [pass/modules (66 roots)](../../../tests/omega/pass/modules), [fail/modules (47 roots)](../../../tests/omega/fail/modules), [pending/modules (1 roots)](../../../tests/omega/pending/modules), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [omega-rust/omega/packages/source-acquisition/src/git/tests/root_pin.rs](../../../omega-rust/omega/packages/source-acquisition/src/git/tests/root_pin.rs), [omega-rust/omega/packages/source-acquisition/src/git/tests/workspace.rs](../../../omega-rust/omega/packages/source-acquisition/src/git/tests/workspace.rs), [omega-rust/omega/packages/source-acquisition/src/identity/tests.rs](../../../omega-rust/omega/packages/source-acquisition/src/identity/tests.rs), [omega-rust/omega/packages/build-declarations/src/tests/mod.rs](../../../omega-rust/omega/packages/build-declarations/src/tests/mod.rs).

**Required families:**

- `packages-sources-001` — Interpreted purpose-qualified requests create graph edges; no import scan or call-name parser does, and graph closes only at mandatory joins.
- `packages-sources-002` — Requester-local aliases remain separate from canonical keys and resolutions; diamonds coalesce exact owners while conflicts/cycles reject.
- `packages-sources-003` — Anonymous root identity derives dependency-first after closure without historical catalog or compiled-output hashing.
- `packages-sources-004` — Build/product checked instances remain separate even when source acquisition shares bytes.
- `packages-sources-005` — Git repository/revision and Root/Named member selection are separate, with one pinned commit/tree per closure and exact verified declaration custody.
- `packages-sources-006` — Mutable local capture excludes Git internals/root output/control state and rejects links smuggling them back; resolver-owned snapshots retain exact published rows.
- `packages-sources-007` — Ordinary local compile retains captured bytes in memory, independent of persistent transaction/review snapshots.
- `packages-sources-008` — Selective update preserves unchanged pins, moves same-lineage members/path edges together and never refreshes unrelated repositories.
- `packages-sources-009` — Missing pins fail offline or fetch exact commit only; whole-invocation offline overrides all new/transitive fetches without changing acceptance.

**Remaining:** No dedicated source graph row; alias diamonds, member pinning, offline/selective updates and captured-control exclusions need acquisition/package fixtures.

<a id="packages-toolchain"></a>

#### toolchain

**Source:** [wiki/spec/packages/toolchain.md](../../spec/packages/toolchain.md) · 2 headings · SHA-256 `ccbb027934da207b74fd697ea323263e4fbc6c16bbd85decf4ea0d0ff7f5ab34`.

**Evidence needed:** Exact core/ordinary library provenance and semantic-binding controls.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/packages (2 roots)](../../../tests/omega/pass/packages), [fail/packages (3 roots)](../../../tests/omega/fail/packages), [pass/modules (66 roots)](../../../tests/omega/pass/modules), [fail/modules (47 roots)](../../../tests/omega/fail/modules), [pending/modules (1 roots)](../../../tests/omega/pending/modules), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [omega-rust/omega/packages/source-acquisition/src/identity/tests.rs](../../../omega-rust/omega/packages/source-acquisition/src/identity/tests.rs), [omega-rust/omega/packages/build-declarations/src/tests/mod.rs](../../../omega-rust/omega/packages/build-declarations/src/tests/mod.rs).

**Required families:**

- `packages-toolchain-001` — One exact core version is language-coupled; ordinary std names, lineage, paths and shapes grant no compiler privilege.
- `packages-toolchain-002` — Build/product dependency scopes apply equally to std while core mount remains explicit toolchain identity.
- `packages-toolchain-003` — Default builds preserve resolved core/std owners without collapsing onto anonymous root.
- `packages-toolchain-004` — Consumer semantic bindings rejoin exact package/declaration/schema/required plan and settle every row once, independently of candidate designation.
- `packages-toolchain-005` — Bindings grant no provider, capability or invocation; downstream consumers use resolved identities rather than another name lookup.
- `packages-toolchain-006` — Sealed proof projections and canonical primitive operator owners require exact admitted toolchain identities, not lookalike declarations.
- `packages-toolchain-007` — Standalone compatibility limits do not become package-aware semantic authority.

**Remaining:** No dedicated toolchain provenance row; same-spelled std/core and stale/unmatched semantic-binding controls need package compilation fixtures.

<a id="packages-topology"></a>

#### topology

**Source:** [wiki/spec/packages/topology.md](../../spec/packages/topology.md) · 20 headings · SHA-256 `fde81ce5ff47555b2c2768ac8911dd998c4c1c62962cb73773cdb2c4cef2cfa6`.

**Evidence needed:** Package graph correctness, independent plan verification and installed endpoint confinement.

**Fact area candidates:** no dedicated row identified; consult the other candidate areas.

**Other candidate areas:** [pass/packages (2 roots)](../../../tests/omega/pass/packages), [fail/packages (3 roots)](../../../tests/omega/fail/packages), [pass/modules (66 roots)](../../../tests/omega/pass/modules), [fail/modules (47 roots)](../../../tests/omega/fail/modules), [pending/modules (1 roots)](../../../tests/omega/pending/modules), [pass/build (56 roots)](../../../tests/omega/pass/build), [fail/build (92 roots)](../../../tests/omega/fail/build), [pending/build (1 roots)](../../../tests/omega/pending/build), [omega-rust/omega/packages/source-acquisition/src/identity/tests.rs](../../../omega-rust/omega/packages/source-acquisition/src/identity/tests.rs), [omega-rust/omega/packages/build-declarations/src/tests/mod.rs](../../../omega-rust/omega/packages/build-declarations/src/tests/mod.rs).

**Required families:**

- `packages-topology-001` — Topology is ordinary build-only package logic over generic verified complete component descriptions, not compiler graph syntax or service reach ordering.
- `packages-topology-002` — Finite exact instances/endpoints bind complete subjects, schemas, entries, authority and assumptions; unknown communication coverage rejects.
- `packages-topology-003` — Every demanded import has exactly one explicit compatible binding; no implicit equal-name, default, wildcard or dynamic-delegation edges.
- `packages-topology-004` — Conservative graph grants every outgoing endpoint from every component entry/state, with callbacks explicit and synchronous replies not extra invocation grants.
- `packages-topology-005` — no_route and only_via use exact nonempty disjoint selector sets, cycles and bounded traversals; only_via requires existential connectivity and excludes bypasses.
- `packages-topology-006` — Algorithm correctness needs ordinary proof/admission evidence; successful BFS or stored verdict alone is not a proof.
- `packages-topology-007` — Independent immutable owner request fixes roster, policies, verifier, confinement and assumptions; plan writer cannot omit policy or choose replacement intent.
- `packages-topology-008` — Consumer reconstructs graph and reruns exact admitted reference policies, with exact versioned codec/schema and malformed/forged/stale input rejection.
- `packages-topology-009` — Complete endpoint confinement covers raw networking, shared memory, inherited handles, brokers and opaque code under checked narrowing or admitted enforcement.
- `packages-topology-010` — Wire bindings retain exact codec/operation, ownership, failure/retry/lifetime semantics; graph routing proves neither payment authorization nor information-flow secrecy.
- `packages-topology-011` — Build publishes only exact completed artifact bytes; installation separately joins all binding receipts, code and confinement before any application activation.
- `packages-topology-012` — Preparation failure returns/cleans exact pending custody; replacement validates new plan then quiesces old before new activation, with no assumed safe union.
- `packages-topology-013` — First customer is three local processes/private request-response pipes with bounded frames, one outstanding request, exact mappings and denied endpoint attempts on Windows/macOS.
- `packages-topology-014` — Pending authored Build customer and deleted disconnected Rust helpers close no current product acceptance; missing producer APIs and unavailable platforms stay explicit.

**Remaining:** No dedicated topology row; spec status explicitly says authored Build/source-free consumer, demand/supply schema join and Windows/macOS confinement remain unclosed. Package algorithms and installed confinement require different evidence.

## Refresh and verification

This is a replaceable source-bound audit, not a second task board or a new normative spec. Refresh the snapshot and its evidence links together after source changes. Retain unknowns until the actual candidate has been inspected and qualified; remove this static audit if a generated requirement/evidence inventory supersedes it.

Validation compares the complete tracked `wiki/spec/**/*.md` roster and every SHA-256 against the JSON; checks all heading titles/levels/lines and family IDs; verifies all fact places, control/diagnostic hashes and candidate paths; and validates generated local Markdown links. No compiler suite was run for this documentation audit.

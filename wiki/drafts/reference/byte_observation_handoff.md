# Checked byte observations: unfinished handoff

Draft [#3746](https://github.com/CathedralOS/Omega/pull/3746), branch
`work/psi-declared-byte-observations`, retains the original qualified-buffer
customers. The owner explicitly authorized stopping with a draft handoff on
2026-11-16. Neither customer is complete, and no receiving session has accepted
ownership. Keep their acceptance and follow their next failures when resuming.

## Original acceptance

- `samples/cli/probes/bounded_concat`: preserve both copies (`"Room "` at offset
  zero and the live `"A1"` label at offset five), label capacity four/live length
  two, text capacity sixteen/live length seven, and the `"Room A1"`/length-seven
  success check. Complete ordinary Build/Product, native/Psi execution and
  independent retained-artifact reopening. Source maintenance makes genuine Core
  imports and the original Ascii/Wrapping literal casts explicit; it does not
  change bytes, operations, bounds or the oracle.
- `samples/cli/basics/multiplication_table`: retain its Std dependency, initial
  whole assignment of twenty-eight spaces, every clear/digit write, all numeric
  operations and bounds, and the nine formatted multiplication rows. Complete
  ordinary Build/Product, native/Psi execution and independent artifact reopening.
  Verify all nine ordered rows externally, including their original two-digit
  fields and twenty-eight-byte width. Its entry establishes stronger concrete
  contents before the loop; do not infer ASCII from UTF-8 or replace the source
  buffer contract merely to bypass restoration.

## Current code

The final draft's Rust/library input manifest is
`782c24b2152edf5478d919e3389f1792464f6f0e920b0d5a0407f228ba420c16`.
The exact submitted commit is recorded in the PR body. The current tree includes:

- Authenticated Core observers and complete typed-header admission. Runtime and
  logical calls reuse the existing canonical byte-predicate operation. Ordinary
  wrappers retain exact Boolean-equivalence obligations; a function's spelling
  grants no primitive meaning. General ordinary helper-domain normalization is
  still unserved.
- Named/default storage requirements, restoration windows and structural-arrival
  correspondence from the earlier checkpoints. UTF-8 no longer incorrectly
  supplies an ASCII bound on each byte.
- Reference disposition through one shared actual-source query. Owned literal
  descriptors and owned block arrivals use the same existing custody rules as
  other homes. Borrowed storage, referent crossings, inline-descriptor identity,
  restoration and child-closure guards remain.
- Plain subslice formation followed by the ordinary Requalify/Establish proof
  edge for its requested domain. A result annotation alone no longer asserts
  qualification. The receiving exact-plain-descriptor guard is unchanged.
- Literal byte-domain additions now use ordinary predicate establishment rather
  than omitting the destination's logical fact.
- Whole-window predicate transport. The shared helper authenticates a unique
  u64-zero start, the exact own immediately preceding u64 length measurement and
  `end == length`. Only an authenticated whole window retains all source
  predicates; partial UTF-8 windows still drop UTF-8. Source-checker premises
  remain conditional on the real source predicate.

Required integration with fixed main `a22f0b3fbeb1c8dbac98f201e99eaec1f232f411`
is committed in `3b47e7dd005db3aaf5a7584703df18f27447296d`. It uses main's
captured byte-element owner and nominal-Exact cast dependency check, preserves
one canonical default-requirement roster, and retains main's shared operand
resolver. One new governed-membership scan was mechanically ported to the
existing `(place, carrier, memberships)` tuple; its policy check remains.

## Evidence and limits

On the final manifest above, strict all-target checks pass in
`terminal-semantics`, `typed-trees`, `typed-trees-to-terminal-psi` and
`terminal-psi-to-terminal-psi` (39.69 seconds). The following combined unit build
was interrupted by the environment: it has no completed exit receipt or current
execution verdict. The new whole-window semantic mutation test and latest
literal/whole-window changes have **not been executed**. No new compiler,
Product, native, full Std or saved-artifact success is claimed.

Earlier results remain tied to their actual inputs:

- `01e7f5f8525`, manifest `f828581d7a91`: Source06 strict passes (15.11 seconds),
  runner build passes (830.94 seconds), and borrowed-restoration positive/negative,
  missing-disposition tamper and UTF-8 preservation controls pass. Original Room
  A1 gets past reference release and next refuses a qualified raw subslice.
- Pre-final manifest `5d6e2114167918c46b0fabdfb5818a200a46fcd48bce6207d9cd62ec92af1df4`:
  affected-three-crate strict passes (94.63 seconds); three runner builds pass
  (1038.68 seconds). Five of seven focused functions pass, covering five subslice
  measurement/access/ownership mutations, nonbyte input under a false premise,
  borrowed restoration, missing disposition and the UTF-8 write pair.
- On that pre-final manifest, original Room A1 refuses in `copying`, machine 4,
  operation 9: `reference use without exact live backing correspondence`.
  `Storage::backing` cannot reconstruct an exact projection at that use. Do not
  replace missing or ambiguous correspondence with the spelled root.
- The pre-final whole-UTF-8 control also lacks a call-requirement fact and a cast
  arrival proof, and receiving rejects its primitive-domain transfer. The final
  whole-window and literal-establishment repairs address those mechanisms, but
  their execution is unmeasured.

Exact receipts and verified executable/archive hashes are retained locally in
worker 3's `build/qualified-byte-customers/resumed-release-*`, `resumed-range-*`
and `resumed-whole-*`. `resumed-whole-strict-exit.json` records the final strict
pass. There is no `resumed-whole-unit-exit.json`; do not promote the interrupted
build into a pass. Earlier b056 receipts retain fifteen completed controls and
an interrupted Room attempt; the later bounded retry has its actual failure.

## Resume here

1. Run the new semantic control with its actual full name:
   `bytes::byte_sequence_predicates::tests::whole_byte_windows_require_zero_and_their_own_immediate_measurement`.
   A zero-test run is not evidence. The control covers whole versus nonzero
   UTF-8 cuts, foreign same-length measurements, wrong width, duplicate zero
   producers and shortened ends.
2. Rebuild the affected source runners with mbx and run
   `tests::compiler_primitive_headers::a_qualified_window_proves_its_domain_without_qualifying_the_raw_subslice`
   and the existing original Room A1 execution test. Preserve the nearby
   nonbyte-formation, subslice-tamper, ownership and UTF-8 negatives.
3. Two **test-only temporary diagnostics** remain intentionally for the next
   trace: Source06 `places.rs::backing` prints the unresolved root/cut/origins and
   actual block, and the existing `OMEGA_TEST_TRACE_DEFAULTS` failure trace now
   includes call requirements and arrival invariants. Set that environment
   variable for exact predicate goals/hypotheses. Remove temporary diagnostics
   after recording the needed correspondence and before proposing a stable merge.
4. Follow the actual descriptor and predicate failure without weakening source,
   bounds, ownership, source custody or proof requirements. Then complete the
   original customer paths above. Reuse the ordinary checked-wrapper control for
   live native/Psi ASCII true (`"Room A1"`) and false (`"é"`) observations, and
   refuse a constant wrapper's false equivalence guarantee. The generated local
   controls under `build/qualified-byte-customers/observer-*-control` have not run.
5. Review, run required admission, and squash only a tested coherent candidate.
   A checkpoint does not close either original outcome. No task acceptance was
   removed and no language-design question was introduced by this handoff.

Governing contracts: [byte views](../../spec/terminal-psi/byte_views.md),
[domains](../../spec/language/domains.md),
[invariant windows](../../spec/language/dependent_values.md#invariant-windows),
and [structural access](../../spec/terminal-psi/structural_access.md).

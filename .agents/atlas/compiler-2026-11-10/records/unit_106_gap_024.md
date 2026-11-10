Second pass for unit 106

Ranges covered (paths relative to `<checkout>/`):

1. omega-rust/psi/foundation/access-plans/src/tests.rs 6701-7168 (468 lines)

Shorthand: `T` = omega-rust/psi/foundation/access-plans/src/tests.rs. I read lines 6660-7169 (a few lines above the range to find the enclosing test); only 6701-7168 counts toward coverage.

---

## T 6701-7168

**Enclosing module / function.** The range starts in the middle of the test `placed_field_authorization_replays_projection_authority_and_allows_retry` (T:6664-6708, `#[test]` at 6664); lines 6701-6708 are its tail. The remaining 11 tests (T:6710, 6749, 6806, 6837, 6892, 6921, 7007, 7046, 7070, 7114, 7144) lie wholly inside the range. All sit in the top-level `mod stable_specialization` (header T:6176, imports T:6177-6194), whose closing brace is T:7168 (read). The file's `mod stable_specialization` is a sibling module of earlier test modules, not nested in another module (read: `mod` is at column 0 at T:6176).

**Purpose.** Tests that every conversion step in the Stable access path (field projection -> authorization -> `PrimitiveAccessRequest` -> `StablePrimitiveAccess` / Stable compound mutation) replays the retained authority instead of trusting copied fields. Each test copies a valid request/projection/view, corrupts one field at a time, asserts a specific rejection text, asserts the failed conversion hands back the identical request, restores the field and retries successfully. They also assert that receipts and custody are untouched after a rejected request is dropped.

**Shape (tests, with `file:line`).**
- `placed_field_authorization_replays_projection_authority_and_allows_retry` T:6665-6708: tampers `projection.plan.0` (T:6671), `supply.offset` (T:6679), `primitive_address` (T:6686); each makes `projection.read()` fail ("access field authorization" + "retained authority", "replayed resource row", "reproduce the projected primitive address"); repairs; final request must specialize to Stable with address 0xad20 and admission id 167 (T:6701-6707).
- `stable_primitive_specialization_replays_exact_supply_row_and_returns_custody` T:6711-6747: mutates `request.effective_supply` fields (key slot, field name, width_bits, offset, alignment_bytes) and `primitive_address`; expected fragments "supply key and width", "field identity", "supply offset", "supply alignment". After drop, `validity_receipt` = 169 and `custody_receipt` = 170 (T:6745-6746).
- `placed_authorization_and_specialization_replay_resident_content_grant` T:6750-6804: builds a second owned resident (`replacement_*`) and an unrelated content grant (T:6753-6772), swaps `corrupt.content` (T:6772), points projection `_authority`, `resident_claim`, `access_occurrence` at the corrupt view (T:6777-6779); read must fail with "resident content grant" (T:6783). Repeats on a `PrimitiveAccessRequest` (T:6792-6795), then restores and specializes (T:6797-6803, address 0xad50).
- `stable_primitive_specialization_replays_descriptor_geometry_and_authorization` T:6807-6835: corrupts `logical_extent.fragments[0].source_bit_offset` ("field descriptor"), `effect_footprint.address` and `length_bytes` ("effect footprint"), and `operation = Write` ("does not permit Write").
- `stable_primitive_specialization_replays_exact_placement_authority` T:6838-6890: builds a borrowed (not owned) Stable view from `extent.loan(0, 4)` (T:6842-6846). Corrupts `plan`, `profile_receipt`, `admission`, `reach`, `source_loan`, `resident_claim` (spurious Some), and the field name in three places at once (descriptor, request, effective_supply); expected fragments "access authority" (x4), "source-loan", "resident identities", "resource row".
- `stable_primitive_specialization_rejects_coherent_authorization_rewrite` T:6893-6919: exclusive source loan (`loan_mut`, T:6897); sets `current_borrow = Exclusive` and `operation = Write` on a request authorized for shared Read; expects "field authorization"; asserts the retained `request.authorization` still reports `Shared` and `Read` (T:6914-6918).
- `borrowed_view_establishment_replays_profile_and_returns_admission_for_retry` T:6922-7005: destructures `AccessAdmission` field by field (T:6934-6941, 6963-6970) to swap in `profile` from a different extent whose receipt is equal (asserted T:6933); `place` must fail with "could not replay the admitted resource profile" (T:6956) and return the admission (T:6960-6962). After repair, `place` succeeds; then swaps `view.profile` for a different-lineage profile (T:6981-6982) and `view.project` must fail with "field projection" + "retained access authority" (T:6986-6987); restores and walks through to Stable specialization (T:6992-7004).
- `borrowed_view_retirement_replays_authority_and_returns_exact_loan` T:7008-7044: UART plan, 12-byte shared loan; corrupts `view.resources.fields[0].offset ^= 4` (T:7022); `view.retire()` must fail with "resource compatibility differs" (T:7030); repaired view retires and returns a loan equal in origin, lineage root, base 0xad88, length 12, polarity Shared (T:7039-7043).
- `established_primitive_specialization_replays_resident_identities` T:7047-7068: drifts `resident_claim` to id 184 and `access_occurrence` to id 185; both expect "resident identities"; receipts 181/182 intact after.
- `stable_compound_specialization_fails_closed_and_returns_exact_request` T:7071-7112: exclusive projection `project_mut` then `compound_mutation()` (T:7074-7079); corrupts `observation` to External ("Stable observation"), `effective_supply.kind` to External ("Stable supply"), `key.slot` and `effective_supply.width_bits` ("supply key and width"), `current_borrow` Shared and `source_loan` Shared (both "exclusive current and source"), `operation = Write` ("sealed CompoundMutation event"). Receipts 165/166.
- `external_request_rejects_stable_specialization_and_returns_exact_request` T:7115-7142: no corruption; an External-observation UART `status` read cannot enter `into_stable_primitive_access` ("Stable observation"); returned request keeps plan, admission 132, address 0xb000, observation External, op Read, source loan Shared.
- `compound_request_rejects_stable_specialization_and_returns_custody` T:7145-7167: a valid compound-mutation request is refused by `into_stable_primitive_access` ("Read or Write"); returned request has operation `CompoundMutation`; receipts 134/135.

**Inputs and outputs.** Uses in-file helpers defined earlier (not part of this range): `established_stable_word`, `stable_word_placement`, `stable_word_profile`, `uart_extent_with_lineage`, `uart_extent`, `uart_access_plan`, `uart_reach`, `uart_resource_profile`, `admit_uart`, `provider_existing_content`, `field_key`, `primitive_request_snapshot` (imports T:6177-6182). `expect_exact_stable_primitive_rejection` and `expect_exact_stable_compound_rejection` are generated by `expect_exact_rejections!` (T:615-646, read for this pass): each takes the request, snapshots it with `primitive_request_snapshot` (T:622), calls `into_stable_primitive_access` or `into_stable_compound_mutation_access` (T:644-645), asserts the error text contains the given fragment (T:628-634), takes the request back via `into_parts()` and asserts the snapshot is identical (T:635). Outputs: assertions only; nothing is produced.

**How it decides.** The tests are keyed on substring match of the diagnostic text (`.0.contains("...")`, e.g. T:6675, 6683, 6783, 6956) and on identity numbers (`normalized_identity()` 164-265, e.g. admission 167, receipts 165/166). They do not pin the error type, only the message fragment. Specific values used as the "right" ones: addresses 0xad20, 0xad40, 0xad50, 0xad60, 0xad70, 0xad74, 0xad78, 0xad7c, 0xad80, 0xad88, 0xb000, 0xb100; admission ids 132-266.

**Cases by shape.** Every test is the Stable word fixture (4-byte `word` field) or the UART fixture (`status`, 12 bytes); no Atomic or External specialization is exercised in this range, except T:7115-7142 which uses the External `status` field only to show it cannot become Stable. One borrowed-view test uses `extent.loan` for a shared view (T:6842, 6927, 7013); one uses `loan_mut` (T:6897); the others use owned-established views via `established_stable_word`.

**Same job elsewhere.** The corrupt-one-field/expect-fragment/repair sequence at T:6717-6745, 6812-6834, 6850-6889, 7076-7111 is the same pattern repeated; the same pattern for Atomic is at T:643 (`expect_exact_atomic_rejection`, earlier range, not in my unit). Several tests reuse identical identity numbers: `established_stable_word(0xad80, 164, 165, 167)` at T:7072 and `established_stable_word(0xad20, 164, 165, 167)` at T:6666 both use 164/165/167 (read).

**Unused or one-caller code.** The range defines no `pub` items. It calls these crate items that earlier notes said have no outside-crate callers: `admit_access` (T:6844), `admit_owned_access` (T:6756), `adopt_owned_stable` (T:6763), `place` (T:6846), `into_stable_primitive_access`, `into_stable_compound_mutation_access`, `AccessAuthorityRef::EstablishedOwned` (T:6777, 6785, 6792, 6797), `AccessAdmission` destructuring (T:6934-6941, 6963-6970). So these tests are the only consumers of the Stable owned and borrowed lifecycle seen in this crate. This confirms the earlier notes' suspicion (unit_106.md "AP/tests.rs" and lib.rs sections) for the Stable half: the tests exercise it. (Earlier notes found 0 outside-crate users for these names via grep; I did not re-run that grep.)

**Stated limits.** None. Comments in the range are `expect("...")` strings; none says anything is unsupported. T:7157 states "compound mutation needs its distinct bounded lowering" as an expectation message, i.e. Stable ordinary specialization refuses compound mutation.

**Fail-open risks.** None seen in the range; it is test code that asserts rejections. Two things a reader should know about what the tests prove:
- `stable_primitive_specialization_replays_exact_placement_authority` (T:6838) corrupts the field name in three places at once (T:6885-6887) so the descriptor, request and supply row agree with each other; the rejection fragment is "resource row", i.e. the check is against the replayed resources, not internal consistency. (read; status: stated by the test's own name "replays exact placement authority".)
- The assertions only check message substrings, so a rejection for a different reason with the same fragment would pass. Not a fail-open of the compiler; a limit of what the tests pin. (inferred from T:6675-6694, 6956.)

**Contradictions with unit_106.md.** None found. The earlier notes said tests.rs lines 1-700 contain no `#[test]`; my range does contain 12 (one partial). Earlier notes at unit_106.md section 8 said "nothing is claimed about 701-7168"; consistent. The earlier "Five things" item 1 says only `tests.rs` may exercise the placement lifecycle: confirmed for the Stable owned/borrowed lifecycle by T:6756-6763, 6844-6846.

---

## Coverage

- Read in full: omega-rust/psi/foundation/access-plans/src/tests.rs 6701-7168 (468 lines). Context lines read but not counted: 6660-6700, 612-647, 6165-6194.
- Skimmed: none.
- Not read: none.
- Total: 468 = 468 read + 0 not read.
- Counts: `fn name() {` at 4-space indent in the whole of tests.rs = 109 (Grep, count mode, pattern `    fn [a-z_0-9]+\(\) \{`); not broken down by range.

## Five things a maintainer most needs to know about this range

1. All 12 tests are in `mod stable_specialization` (T:6176-7168) and cover the Stable path only: project -> read/compound -> primitive request -> `into_stable_primitive_access` / compound specialization, plus borrowed `place` and `retire`.
2. The shared pattern is corrupt one copied field, expect a named diagnostic fragment, assert the request comes back byte-identical (`primitive_request_snapshot`, T:622-635), restore, and retry; "returns custody" tests assert receipts stay at fixed ids after the rejected request is dropped (e.g. T:6745-6746, 7110-7111, 7165-7166).
3. The checks are string-keyed on diagnostic fragments such as "supply key and width", "resident identities", "access authority", "resource row", "sealed CompoundMutation event" (T:6722-6888, 7082-7107), not on error variants.
4. Two tests re-create the same authority from different carriers: owned resident (`established_stable_word`) and borrowed view (`extent.loan`/`loan_mut` + `admit_access` + `place`, T:6844-6846, 6897-6901, 6927-6929).
5. `placed_authorization_and_specialization_replay_resident_content_grant` (T:6750-6804) builds a hand-corrupted resident view by assigning `corrupt.content` directly (T:6772), so it relies on that field being reachable from the test module; the other tests use similar direct field writes (`request.effective_supply.*`, `projection.plan.0`, `view.profile`, `view.resources.fields[0]`).

## What you are unsure about

- I did not read the helper bodies (`established_stable_word`, `provider_existing_content`, etc.) beyond what the earlier notes say; so I only know their names and the numbers the tests assert about them.
- I did not re-run the outside-crate greps for `admit_access` etc.; I rely on the earlier notes for the "0 outside-crate users" claim.
- I did not check that each corrupted field named in a test is actually checked at the code site that produces the asserted fragment (that would need the primitive_access.rs validators, which belong to another range).

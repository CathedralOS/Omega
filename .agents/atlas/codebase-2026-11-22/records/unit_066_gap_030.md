# Record: unit_066_gap_030 (area 04-source-stages-06-07)

P06 = `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src`.
This is the second pass over unit 066. It covers `P06/tests.rs` lines 4951-7575 and seven files under
`P06/tests/`. Everything here is test code for the stage 06 lowered-form checker. No file in this unit
defines production code; there are no `pub` items to grep. All claims are from reading, except where a
grep or count is named. The files under `P06/tests/` are declared by `mod` lines in `tests.rs`
(`tests.rs:28-34`, grepped).

Shared vocabulary of the tests (read): `run`/`judge`/`discharge`/`lower` are helpers defined earlier in
`tests.rs` (outside my ranges); `Shaped`, `Machine`, `Lending` are small program-builder structs; a
"verdict" is a per-obligation `Result<Evidence/Discharged, Refusal>`; `RouteVerdict` is
`Accepted | Rejected{..} | Incomplete | FallBack`. A "twin" is a pair of programs differing in one input.

## 1. `P06/tests.rs` lines 4951-7575 (2,625 lines), read in full

**Purpose.** Unit tests for how stage 06 discharges proof obligations: entry clauses over record fields,
default-domain invariant windows, saved-proof reuse, generic-body checking and specialization, branch
facts at join parameters, crash-route coverage, callee guarantees, and erased formals. 45 `#[test]`
functions in the range (`sed -n '4951,7575p' tests.rs | grep -c '#\[test\]'`), one `#[ignore]` timing test
(`tests.rs:6232`, relative line 1282 of range).

**Shape (groups, with `file:line`).**
- Entry clause over a record formal: `formal_field` (`:4964`), `peek` (`:4974`), tests `:4998`, `:5075`.
  Clause `p.x == 7` is decided from the actual's leaf value; a call result leaf is unproved because the
  callee promises nothing (`:5030-5073`, expected detail "the bounds do not decide the comparison").
- Invariant windows (`:5116-6050`): `WindowUse` imported at `:5121`; `assert_accepted` (`:5124`);
  `span_consumers` (`:5203`) builds three consumers of a `Span` (call argument, whole read, return);
  tests: restored window closes before consumption (`:5235`), consumption inside an open window rejects
  naming write and use (`:5272`), guard supplies the missing fact at the consumption point (`:5338`),
  nested inner/outer `Wrap`/`Span` windows (`:5435`), sibling-field store does not close another clause's
  window (`:5522`), a field read that takes a clause as a premise consumes an open window (`:5635`),
  gated local established field by field (`:5704`), window open on one way into a join stays open (`:5799`),
  exactly one verdict per consumption point (`:5902`), a non-restoring store opens no second window (`:5999`).
  Record types come from `windowed()` (`:5590`: `Both`, `Half`, `Limits`, ids 7-9).
- `mod reuse` (`:6051-6332`): `prepare_replay`, `reverify`, `reverify_at_call` with `ProofSearchActivity`.
  Tests: same obligation reverifies with no prover started (`:6076`), stale proof refused (`:6103`),
  proof whose assumption is gone refused (`:6120`), proof for another obligation refused (`:6131`), proof
  from another machine accepted only where goal and hypotheses agree (`:6161`), per-call substitution
  of actuals into a guarantee (`:6187`). The ignored timing test (`:6233`) compares `discharge` to
  `reverify` over 200 rounds and only prints.
- `mod generic_bodies` (`:6334-6919`): builds a generic form with `lower_generic(id(1), .., &[id(2)])`
  (`:6457`), `checked_once` (`:6463`), `prepared_refinement` (`:6469`, builds a `TerminalCompilation::Prepared`
  with `GenericForms`, `Selected`, `witnesses`, `selection_witnesses`), `carried` (`:6571`, calls
  `reverify_specialization`), `closure` (`:6724`, calls `discharge_carrying` with `Carried`/`CarriedForm`/`Selection`).
  Tests: one witness discharge reused for every selection (`:6522`, asserts search count equal for 1 and 2
  selections, `:6541-6544`), failed verdict of a reused witness kept (`:6548`), generic body checked against
  the requirement alone (`:6606`), certificates reverify in two implementations (`:6635`), public call
  certificates do not admit a crossed witness (`:6666`), closure takes generic proofs with three fewer searches
  (`:6755`, `searched - started == 3` at `:6765`), nothing-to-carry falls back to closed check (`:6796`),
  body valid only for one implementation is rejected (`:6819`), non-refining implementation refused (`:6857`),
  one implementation's refinement proof cannot supply another's guarantee (`:6880`).
- Join-parameter branch facts (`:6921-7021`): `either` (`:6921`), `arrival` (`:6932`), `past_a_disjunction`
  (`:6939`), tests `:6957` and `:7002`. Checks both arrivals at a `||` join discharge
  `ArrivalInvariant`, `weakened_joins` is empty, and the later add cites a `BlockInvariant`.
- Crash coverage (`:7023-7254`): `crashing_below_ten` (`:7028`), `crashing_at_zero` (`:7121`),
  `calling_a_zero_route` (`:7182`), tests `:7084`, `:7099`, `:7157`, `:7172`, `:7242`, `:7307`. Each builds
  `Body` and `Signature` tables by hand and calls `lower_bodies` then `discharge`.
- Guarantee tests (`:7256-7304`): `result_at_most` (`:7258`), `:7266`, `:7280`.
- Erased formals (`:7384-7573`): `keep` (`:7387`), `keeps_seventy_under` (`:7403`), tests `:7415`, `:7431`,
  `:7439`, `:7469`, `:7505`, `:7542`. `mod table_faults;` is at `:5114` and `mod loops;` at `:7575`.

**Inputs and outputs.** Inputs are hand-built `Machine`/`Shaped`/`Body`/`Signature` values and
`IdTable<StructuralTypeId, RecordType>` record tables; they go through `crate::lowering::{lower_bodies,
lower_generic, elaborate_over(helper)}` and `crate::checking::{discharge, judge, judge_compilation,
prepare_replay, reverify*, discharge_carrying}`. Outputs are assertions only; nothing is written.

**How it decides (what the assertions pin).** Keyed on identity: obligations by `ObligationId`, operations
by `OperationId`, values by `ValueId`, constants by position in `constants(&elaboration)` (e.g. `[2]`,
`[3]` at `:5248`), and kernel re-checking by `assert_kernel_checked`. Window rows are compared by
`row.at: Construct` and `row.opened_by: ObligationId`. Some assertions key on message text:
`entry.message().starts_with("cannot prove requires clause 1 of a data type ... open default-domain
invariant window")` (`:5326`), `refusal.detail.contains("the bounds do not decide the comparison")`
(`:5068`), and `refusal.detail == "refinement unavailable"` (`:6565`). The `.detail`-string match is
spelling-keyed. `UnprovedCause` and `RefusalReason` variants are matched structurally elsewhere
(`:6227`, `:6710-6716`).

**Cases by shape.** The window tests use only the fixed record shapes in `constrained()` (defined outside
the range) and `windowed()` (`:5590`): `Span`, `Wrap`, `Paired`, `Both`, `Half`, `Limits`. The comment at
`:5806` says "Every arm of this subset completes the machine", and the join test
at `:5799` adds a restoring store "by hand" via `store_by_hand` (`:5862`) plus a hand-pushed
`DataDomain` obligation row (`:5877-5885`), because no source store can sit on that way. The generic-body
tests use only one trait shape (`Step::apply`, one `u32` formal) and implementations `double`, `shift`,
`wide` (`:6372-6393`).

**Same job elsewhere.** The `Signature { .. }` struct literal with the same twelve fields appears as a
copy at `:6441-6455`, `:7044-7058`, `:7135-7149`, `:7189-7203`, `:7215-7234`, `:7318-7332`,
`:7344-7358` and `tests/arrays.rs:331-345`: a repeated hand-built signature (read). The `Body`/`Signature`
build plus `lower_bodies` plus `discharge` triple is repeated in `crashing_below_ten`, `crashing_at_zero`,
`calling_a_zero_route` and the closure in `:7311-7364`. The `assert_kernel_checked` plus `certificate` pair
recurs across the file. Whole-program scaffolds `array_subject` (`tests/array_lengths.rs:142`), `execute`
(`tests/atomic_wake.rs:116`), `execute_restoration` (`tests/borrowed_restoration.rs:628`) and
`assert_array_invocation_value` (`tests/array_lengths.rs:203`) each repeat the same
`publish_invocation` then `TerminalProgram::borrow` then `instantiate` then `invoke` sequence.

**Unused or one-caller code.** None applicable: all helpers are file-private `fn`. Helpers inside the range
that have a single caller within it (read): `witness`/`wide` etc. have several. `result_at_most` is defined
twice: in `mod generic_bodies` (`:6352`, `at_most(2, value)`) and at file level (`:7258`,
`formal(2, ..)` form). The two bodies differ.

**Stated limits.** `:5806` "Every arm of this subset completes the machine" (test subset limit);
`:5896-5898` comment "with different contents on the two arrivals the clause cannot even be stated";
`:6232` `#[ignore = "timing: run with --ignored --nocapture on a quiet host"]`.

**Fail-open risks.** None seen in production code (this is test code). Two tests assert acceptance that
a reader might misread as fail-open; both state why:
- `:6666-6683` `public_call_certificates_do_not_admit_a_crossed_implementation_witness`: stated at `:6667-6669`,
  "That evidence is not implementation admission": crossed witnesses still yield kernel-checked certificates
  for goals that follow from `Step`'s published `<=20` contract, and the real `Wide` refinement is
  judged separately and rejected (`:6685-6718`). Classification: stated.
- `:6880-6918`: the generic `<=10` input proof does match Wide's goal and is accepted, comment "that arithmetic
  fact authenticates no selected implementation" (`:6884`). Classification: stated.

## 2. `P06/tests/address_bounds.rs` (1-319), read in full

**Purpose.** Pins that `addr::Bound` in a contract clause is read as one past the largest address in the
`addr` carrier, and that address arithmetic obeys the count/address signature. Header cites
counts_and_addresses.md (`:1-8`). 10 tests (`grep -c '#\[test\]'`).

**Shape.** `verdict(build: &str) -> String` (`:14`) returns `"accepted"`, `"rejected"`, or the refusal detail
text; it calls `super::build_purpose::judge_build` and, on `Accepted`, calls `publish_invocation().unwrap()`
(`:19`). Tests: `:31`, `:45`, `:62`, `:88`, `:106`, `:124`, `:160`, `:206`, `:252`, `:290`.

**Inputs and outputs.** Omega source strings in, a verdict string out.

**How it decides.** Compared by exact refusal text, so keyed on message spelling: `"contract runtime value in
a mathematical term without embed while reading the signature of `sized`"` (`:51`), `"expression arithmetic on
an address while reading the body of `probe`"` (`:77`), `"expression cast to or from an address while reading
the body of `probe`"` (`:81`). Others use `contains("arithmetic on an address")` (`:237`, `:276`, `:282`) or
`ObligationKind::OperationSafety` structure (`:152`, `:246`). The statement text `failure.statement.contains(
"local `endpoint`")` (`:200`) and `unproved[0].statement.contains("no_wrap(base, length)")` (`:316`) key on
rendered statement text.

**Cases by shape.** The accepted forms are fixed: `addr + u64` and `u64 + addr` (both orders, anonymous
and typed literal), `addr - u64`, `addr - addr -> u64` (`:88-121`). Refused forms are listed at `:207-227`:
`addr + u32`, `addr + addr`, wrapping `+%`, `u64 in Trapping` count. Subtraction refused: `u32` count,
underflow `0 - 1` (rejected, not refused, `:261-269`), `-%`, qualified count (`:252-285`).

**Same job elsewhere.** `verdict` here is a near-copy of `verdict` in `array_lengths.rs:14` (that one adds the
`unestablished` detail) (read).

**Unused or one-caller code.** None (private test helpers).

**Stated limits.** `:61-64` "Address addition to another address and arbitrary value/address casts remain
refused."

**Fail-open risks.** None seen. Note `:305-306` says "The compiler need not prove the proposition false to
refuse the call", and the test accepts either `Incomplete` or `Rejected` (`:308-310`); this is test tolerance
on the refusal kind, not an acceptance. Classification: stated.

## 3. `P06/tests/applications.rs` (1-329), read in full

**Purpose.** Pins that applied generic data (`Optional<u32>`, `Optional<u64>`, nested) arrive through
ordinary parsing and typing and that substituted cases are prepared before checked construction. 3 tests
(`:93`, `:229`, `:271`).

**Shape.** `source_program` (`:9`) and `source_program_in_scope(payload, bundled_optional)` (`:13`) read the
real `source/library/core/option.omg` by `include_str!` (`:19-22`), build a `SourceMap`, lex and parse by
`frontend_fixture`, `resolution::resolve`, then `typing::lower_symbol_resolved_trees`. The tests then call
`data_applications::prepare` (`:153`, `:158`), `crate::lowering::prepare_compilation`,
`crate::checking::judge_compilation`.

**Inputs and outputs.** Reads the bundled `core/option.omg`; produces typed trees and a verdict list.

**How it decides.** Data definitions are found by `name.as_str() == "Applied"`/`"Optional"` (`:99`, `:104`,
`:279`) and machine by `name.as_str() == "entry"` (`:181`, `:236`, `:289`): display-name keyed. Rejection is
detected by `refused.detail.contains("record literal field type")` (`:261`, `:319`) and
`"generic data"` (`:325`).

**Cases by shape.** `:311-315` comment: the first verdict is the closed bodies; "one of those is refused for a
contract this route does not read; none is refused for the data declaration any more." The test does not
assert which other verdict is refused, only that none is refused for "generic data" (`:323-327`).

**Same job elsewhere.** The `Compilation { .. }` literal with `declaration_metadata: &[]`, `purpose:
Product` etc. is repeated at `:188-198`, `:241-253`, `:295-305`, and in `array_lengths.rs:149-159`, `:254-264`,
and `borrowed_restoration.rs:93-103`, `:420-430`, `:861-871` (read). A `build_purpose::roster` helper exists
(`array_lengths.rs:657`) for the same job.

**Unused or one-caller code.** None (private).

**Stated limits.** `:1-2` "A manually supplied closed-row marker does not establish this producer path."

**Fail-open risks.** `:316-322` `closed` is computed with `_ => false`, so an unexpected first verdict fails
the test; no acceptance on unknown. The `declaration_refused` check only looks for the one message
"generic data" in any verdict; a different refusal of the template passes (`:323-327`). Classification:
stated (comment `:311-315` explains one other refusal exists).

## 4. `P06/tests/arithmetic_policies.rs` (1-340), read in full

**Purpose.** Pins that `Wrapping`/`Saturating` integer operations are lowered and judged as their policy
operation, never as the Exact one (header `:1-13`). 5 tests (`:93`, `:135`, `:209`, `:257`, `:305`).

**Shape.** Helpers `qualified` (`:21`), `plain` (`:28`), `of` (`:32`), `operate` (`:42`), `requalified` (`:60`),
`binary` (`:66`, returns `Lending`), `operations` (`:81`), `divided_by_a_sum` (`:232`). Imports
`unsafe_operations` from `mutable_formals` and `accepted`, `lower_lending`, `named`, `unit` from `references`
(`:15-16`).

**How it decides.** Checks the lowered `OperationKind` by variant: wrapping variants `WrappingInteger{Add,
Subtract,Multiply}` and saturating variants (`:106-119`); exact `ExactIntegerDivide`, `ExactIntegerShiftRight`
(`:297`, `:329`). Verdicts keyed on `UnprovedCause::NoRuleForGoalForm` (`:281`). The "membership edge" test
(`:135`) mutates `result.qualifications = forged` on the operation result and expects
`ScalarQualificationError::MalformedMachine { detail: "ordinary scalar operation cannot establish a declared
domain" }` (`:196-202`), a string-literal field match.

**Cases by shape.** Fixed lists: policies `Wrapping`/`Saturating` x ops `Add, Subtract, Multiply` (`:94-101`);
division ops `Divide, Remainder` (`:214`); shift test covers only `ShiftLeft` (`:312`).

**Same job elsewhere.** The Exact-vs-policy distinction is also decided in `terminal_semantics` (not read);
suspected.

**Unused or one-caller code.** None.

**Stated limits.** `:265-270` the wrapped-sum division is `Incomplete`, not `Rejected`, because "an operation
the integer rules have no bound for (`prove::has_rules_for`)". This records that a goal depending on a
Wrapping add is undecidable by the route, so is reported as a missing verdict.

**Fail-open risks.** None seen: the test at `:257-302` asserts that a division by a wrapped sum is NOT
accepted (`Incomplete`). The route returns `Incomplete` rather than `Accepted` for it; classification: stated
(`:265-270`).

## 5. `P06/tests/array_lengths.rs` (1-871), read in full

**Purpose.** Pins `array.length`, rigid (generic) array extents named by a `const` binder, nominal call
const-actual checks, array views, and size-generic boundary requirements. 29 tests (`grep -c`).

**Shape.** Helpers: `verdict` (`:14`, accepted / `rejected: <first unestablished detail>` / refusal text),
`rigid_array_form` (`:77`, calls `prepare_typed_trees` then `lowering::elaborate_generic`), `array_subject`
(`:142`, full `prepare_compilation` plus `judge_compilation`), `assert_array_invocation_value` (`:203`, runs the
published artifact in `terminal_interpreter` with a `WorkMeter` budget of 1,000 and compares an unsigned
result), `concrete_array_form` (`:249`, `elaborate_for`), `sized_boundary_program` (`:635`, builds a `boundary
trait Sink` with a `Count`-sized formal), `accepted` (`:669`).

**Inputs and outputs.** Omega source strings; forms or verdict lists.

**How it decides.** Several tests mutate the lowered module directly and require `discharge` to fail:
- `:121-140`: sets `ArrayExtent::Parameter { owner, value }` to a foreign owner or an undeclared value id and
  expects `discharge(..).is_err()` (identity keyed: `owner: id(1|2)`, `value: ValueId 1|99`).
- `:384-404`: swaps call argument 0 for `ValueId 2` and expects `discharge` error.
- `:407-480`: the nominal-call tests expect exact error text: "nominal call input changed its complete type or
  const actuals" (`:446`, `:477`), "nominal call input has no complete type correspondence" (`:455`),
  "duplicated or unordered" (`:466`). The data under test is `Elaboration.nominal_calls` (cleared at `:451`,
  pushed duplicate `:462`, stale `actual_constructor: StructuralTypeId::new(u64::MAX)` at `:471`).
- `:538-601`: `terminal_semantics::calls::const_array_argument_matches` is called with the module; the test
  pins that an unknown structural type id is not an opaque leaf (`:555-557`), a referent row with
  `abstract_opaque: false` grants no opacity (`:581-584`), and that with `abstract_opaque = true` an
  argument still must match a concrete extent equation (`:597-600`).
The `verdict` helper maps text: `"expression length of a fixed array that is not a field path while reading the
body of `build_entry`"` (`:72`), `"fixed array length through an unestablished case payload"` (`:626`).

**Cases by shape.** Array length is only supported as a field path or local array: comment `:65-66` "A call
result is no place: the call would have to run for its length to be read, and nothing is lowered for a
length." The extent carriers tested are `u32` and `u64` (`:96`, `:785`); array element widths `u8`, `u16`,
`u32`. `:1-7` header: "Literal extents stay literal; symbolic extents use their own constant input."

**Same job elsewhere.** `verdict` is a near-duplicate of `address_bounds.rs:14`. `rigid_array_form` and
`concrete_array_form` are used also by `super::boundary_calls` helpers (`typed`, `entry`, `try_typed`,
`compilation_results_for_subjects`); `super::entryless::judged` is called at `:497`.

**Unused or one-caller code.** None (private helpers). Both `array_lengths.rs:accepted` (`:669`) and
`borrowed_restoration.rs:accepted` (`:41`) and `arithmetic_policies.rs` `accepted` (imported from `references`)
are distinct helpers sharing a name in different files.

**Stated limits.** `:809-810` "Specializing at four cannot discharge a subtraction that underflows in the
declaration at zero." `:842-845` comment notes the template's clauses "are read only by its own form". Test
`a_closed_array_equation_cannot_prove_unequal_extents` (`:198`) asserts `[u16; 2] == [u16; 3]` is rejected.

**Fail-open risks.** Test-only. Three weaker assertions are worth noting for a maintainer reading the tests:
- `:229` `one_array_instance_cannot_authorize_a_generic_constant_extent_guarantee` passes if ANY verdict is
  `Rejected` with non-empty `unproved`; it does not pin which obligation (open).
- `:702` `missing`: replaces `"invokes Text\ncrashes none\n{"` and expects a rejection mentioning
  "synchronous invocation". If the string replace matches nothing, the source is unchanged, the first
  assertion `valid` has already accepted it, and `missing` would be accepted and the `any(Rejected)` check
  would fail; so the test self-checks (traced).
- `:831` after a `Rejected` first verdict, `verdicts[1]` is `accepted`: the template form accepts while the
  closed application is rejected (`:822-831`). This pins different verdicts for form vs closure, stated by the
  surrounding comment.

## 6. `P06/tests/arrays.rs` (1-415), read in full

**Purpose.** Pins that index bounds are tied to the actual extent reached (per array type and per prefix),
signed selectors need a nonnegative bound, array construction/copy/replacement preserve other elements,
a match on an indexed subject computes its selector once, and an array move vacates only its element.
7 tests (`:67`, `:90`, `:140`, `:162`, `:191`, `:231`, `:353`).

**Shape.** `array_types()` (`:8`) builds a table: type 1 `u32` storage, type 2 `four` (4 x u32), type 3 `two`
(2 x u32), type 4 `two-by-four` (array of type 2, length 2). `array` (`:38`), `index` (`:42`), `caller` (`:52`).

**How it decides.** Identity-keyed: equal selector `slot(2, u32)` reads from different types are decided
separately (first has a certificate, second `assert_unproved`, `:85-86`). A long short-circuit guard of
10 selectors: when `last_bound == 5` the last obligation is unproved, otherwise all kernel-checked
(`:129-135`). Signed selector: `lower` of `0` accepted, `-1` rejected (`:163`). The move test requires
the rejection to be only in `unestablished`, with exact text "owned storage is used after its value was
transferred or consumed" (`:410`), and `formations`, `unproved`, `conflicts` empty (`:407`). The indexed
match test counts `OperationKind::ExactIntegerAdd` operations equal to 1 (`:311-318`) and expects
`lower_bodies` to fail with "outside its match capture" when a capture belongs to another match (`:346-349`).

**Cases by shape.** Fixed array types only: lengths 4 and 2, `Literal` extents (`:30`). Owned-element test
uses a single record `OwnedElement` with one `u32` field and array length 2, accepted read index `1` after
move of index `0` (`:380`).

**Same job elsewhere.** The `Signature { .. }` literal at `:331-345` repeats the pattern from `tests.rs`
(see section 1). The guard-bounds-reach-the-read property is also tested in `tests.rs:6957-7021` for join
parameters (different mechanism).

**Unused or one-caller code.** None.

**Stated limits.** `:1-2` "The same storage operations handle array construction, copies and writes."

**Fail-open risks.** None seen. At `:402-413` the accepted twin for `read_index == 0` is rejected via
unestablished storage (traced).

## 7. `P06/tests/atomic_wake.rs` (1-350), read in full

**Purpose.** Runs Omega programs that use atomic data (`AtomicU64`, `AtomicU32`) through check, publish and
the Terminal Psi interpreter, and checks atomic misuse is refused. Header `:1-3`: "a sequential wake oracle,
not a claim about concurrent interrupt delivery or native retry-loop timing." 10 tests.

**Shape.** `WAKE` const source (`:7-109`): `TimerRootWakeState`, `TimerRootWakeClaim`, `record_and_publish`,
`claim_pending`, then `build_entry`. `execute(source)` (`:116`) calls `judge_build`, requires `Accepted`,
`publish_invocation`, runs with a 10,000-unit meter, expects `Complete(Unit)`. `refused` (`:173`) requires the
verdict is not `Accepted` and `format!("{result:?}")` contains `detail`. `malformed_ordering` (`:225`) calls
`boundary_calls::try_typed` and expects a typing error containing `detail`.

**How it decides.** Refusals are matched by `format!("{result:?}").contains(..)` of a Debug string: `"call
selected by its receiver's type"` (`:189`), `"exact resident type"` (`:202`, `:206`), `"cannot prove ensures
contract"` (`:214`), `"write-only reference parameter"` (`:218`), `"mutable"` (`:221`), `"ObserveWhileExclusive"`
(`:244`, a variant name), `"used after its value was transferred or consumed"` (`:289`), `"call borrow of a value
that is not a place"` (`:348`). Typing errors for illegal orderings: `"atomic store cannot use"` (`:193`),
`"atomic load cannot use"` (`:197`). Spelling-keyed on Debug output.

**Cases by shape.** Illegal orderings listed: store with `Receive`, load with `Publish` (`:191-198`). Width
checks: `u64` value stored into `AtomicU32`; `u64` result of `fetch_or` on `AtomicU32` (`:199-206`).

**Stated limits.** `:333-334` "Borrowing a computed payload currently refuses in the reader before the
cleanup partition. This is fail-closed coverage, not a retirement-loan test."

**Same job elsewhere.** `execute` is the same publish-and-run sequence as `assert_array_invocation_value`
(`array_lengths.rs:203`) and `execute_restoration` (`borrowed_restoration.rs:628`).

**Unused or one-caller code.** None.

**Fail-open risks.** None seen; every `refused` call asserts not-`Accepted`. A bad detail string would only
fail a test.

## 8. `P06/tests/borrowed_restoration.rs` (1-751), read in full

**Purpose.** Tests the borrowed-storage invariant window from ownership.md: moving an owned value out of
storage lent with `@mutable` leaves a window that must be closed by storing a replacement into the exact
vacant place before return, transition or handback (header `:1-12`). 40 tests (`grep -c`).

**Shape.** `DATA` (`:16`) defines `Inner { value: u32 }` and `Holder { inner: Inner; tag: u32 }`. `verdict(body)`
(`:18`) calls `super::entryless::judged(text, false)` and returns `Ok(messages)` for `Rejected`, `Err("accepted")`
for `Accepted`, `Err("unfinished")` for `AcceptedPrefix`, `Err("incomplete")`, `Err("not supported: ..")` for
`FallBack`. `accepted` (`:41`), `rejected(body, rule)` (`:45`, requires some violation text containing `rule`).
Constants `NOT_RESTORED = "the place does not hold a value again where the machine returns the borrow"`
(`:53`) and `USED_VACANT = "owned storage is used after its value was transferred or consumed"` (`:55`).
Fixtures: `include_str!` of `tests/fixtures/ownership/run/{borrowed_root_restoration, owned_borrowed_field_restoration,
owned_borrowed_root_restoration, retained_borrowed_root_restoration}/main.omg` (`:623-626`, `:658-661`,
`:666-669`, `:749-752`); `ls` confirmed those directories exist under
`P06/../tests/fixtures/ownership/run/`.

**Inputs and outputs.** Omega source in; for the run tests, `execute_restoration` (`:628`) replaces
`machine main()` with `machine build_entry()`, accepts, publishes and runs in the interpreter expecting `Unit`.

**How it decides.** The main distinctions are keyed on identity of storage (exact vacant place) and on
rendered message text (`NOT_RESTORED`, `USED_VACANT`, `"conflict:"`, `"TwiceExclusive"`, `"not supported"` text
`"whole-referent restoration requires unqualified nonlinear owned storage without claims"` at `:276`).
Direct-module tests:
- `claim_verification_preserves_the_exact_unavailable_storage` (`:110`): calls
  `plain_affine::verify_in`, `verify_claims_in`, requires the same `site`, `place`, `unrestored`, then
  `discharge` must return `RefusalReason::Unestablished` with `detail == actual.message()` (`:171-172`).
- `malformed_return_claims_are_refused_before_unrestored_storage` (`:177`): pushes a
  `StructuralResultClaimBinding { claim: id(u64::MAX), path: [] }` into `returned_claims` and expects
  `Error::Malformed { detail: "return claims differ from actual returned custody" }` and
  `RefusalReason::ForeignIdentity` from `judge` (`:200-219`).
- `a_last_reference_use_does_not_authorize_parent_use_before_actual_release` (`:399`): moves a
  `ReleaseReference` operation after a parent observation, recaptures `unoptimized_behavior`
  (`:464-469`), and expects `LoanConflict::ObserveWhileExclusive` (`:474`).
- `an_untracked_copyable_carrier_does_not_erase_an_explicit_window` (`:692`): rewrites a
  `StructuralLeafCopy` into `MoveStructuralField` and expects `plain_affine::plan` to return
  `Error::Unsupported { detail: "a borrowed move requires tracked storage contents" }` (`:720-726`).
- `a_dead_borrowed_block_address_still_owes_its_missing_field` (`:840`): calls
  `plain_affine::borrowed_address_liveness` and `is_address_header`/`is_live` (`:879-901`); requires a moved
  field on an address header dead at a return; the invalid twin is accepted as owing restoration if either a
  `Rejected` violation contains `NOT_RESTORED` or a `FallBack` detail contains "borrowed block storage still
  owes restoration at its last use" (`:918-926`).
- `replacing_a_borrowed_sum_returns_only_the_new_tags_claims` (`:930`): three mutations of
  `loan_completions` (clear, clear `path`, reuse old claim) must each fail `verify_claims_in` with a message
  containing "loan completion" (`:980-997`); removing the store gives an `unestablished` detail "a replacement
  must be stored" (`:1018`).

**Cases by shape.** Tests list explicit program shapes: whole local owned home, owned field of a local
holder, field of a separate record, borrowed param field, whole borrowed referent, retained exclusive local,
exclusive descendants, `Loan<'storage>` with a field reference (`:818-836`), sum payload repaired by replacing
the whole sum (`:930`). Comment `:551-552`: "A shared reference lends no window: the move stays outside what
this route reads, and is never accepted." (`assert_ne!(outcome, Err("accepted"))` at `:558`, `:614`, `:778`.)
Comment `:264`: `a_whole_qualified_owned_origin_remains_explicitly_unsupported` pins that a qualified owned
origin is refused with the text at `:276`.

**Same job elsewhere.** Window/restoration logic is also under test in `tests.rs:5116-6050` (data-domain
invariant windows); this file is the ownership counterpart. `plain_affine` is in `terminal_semantics` (not
read; suspected owner of the checking logic).

**Unused or one-caller code.** None (private).

**Stated limits.** `:264` test name "remains explicitly unsupported"; `:551` comment above; `:276` the refusal
text quoted above.

**Fail-open risks.** `an_owned_move_through_a_shared_reference_is_not_accepted` (`:554`),
`an_owned_whole_referent_cannot_be_taken_through_a_shared_loan` (`:613`) and
`a_shared_local_reference_does_not_acquire_an_owned_window` (`:777`) assert only `!= Err("accepted")`, so
any non-accepted outcome (including a lowering refusal, `Incomplete`) passes them; they do not pin which
rule refuses (open). The tests at `:402-413` and elsewhere pin exact text. Production risks: none seen.

## Coverage

Read in full (every line, in order):
- `P06/tests.rs` 4951-7575: 2,625 lines.
- `P06/tests/address_bounds.rs` 1-319: 319 lines.
- `P06/tests/applications.rs` 1-329: 329 lines.
- `P06/tests/arithmetic_policies.rs` 1-340: 340 lines.
- `P06/tests/array_lengths.rs` 1-871: 871 lines.
- `P06/tests/arrays.rs` 1-415: 415 lines.
- `P06/tests/atomic_wake.rs` 1-350: 350 lines.
- `P06/tests/borrowed_restoration.rs` 1-751 (the file holds 1,021 lines; unit range says 751 from 1; see
  Unsure): 751 lines listed in the unit row.

Total read in full: 6,000 lines as listed in `units/gap_030.tsv`. Skimmed: none. Not read: none within the
unit rows. Counts for tests come from `grep -c '#\[test\]'` per file and one `sed | grep -c` for the
`tests.rs` range.

## Five things a maintainer most needs to know about this unit

1. All of it is test code that builds its own programs. The `tests.rs` range uses hand-built `Machine`,
   `Body` and `Signature` values (`tests.rs:6441-6455`, `:7044-7058` and five more copies); the `tests/` files
   use Omega source through `judge_build`, `entryless::judged` or `boundary_calls::typed`. A change to the
   `Signature` struct breaks about eight copies.
2. Many assertions are keyed on rendered message text, not identity: `NOT_RESTORED`/`USED_VACANT`
   (`borrowed_restoration.rs:53-55`), `"expression arithmetic on an address while reading the body of ..."`
   (`address_bounds.rs:77`), `"nominal call input changed its complete type or const actuals"`
   (`array_lengths.rs:446`), Debug-string `contains` in `atomic_wake.rs:179-180`. Rewording a refusal in stage
   06 or the lowering will fail these tests without any behavior change.
3. The window tests (`tests.rs:5116-6050`) hand-inject a store and a `DataDomain` obligation row
   (`tests.rs:5862-5885`) to test a join, because source cannot place a store on one arm. The
   "exactly one verdict per consumption point" test (`tests.rs:5902`) re-derives the expected list from the
   lowered operations with its own loop (`:5949-5967`), so it mirrors the checker's rule for what counts as a
   consumption point (whole copies of the root after a store, every `CallStructuralScalar`, every `Return` edge).
4. The generic-body tests (`tests.rs:6334-6919`) pin that carried certificates are re-decided by the kernel
   with zero searches (`assert_eq!(started, 0)` at `:6644`, `:6675`, `:6844`, `:6874`, `:6887`) and that a
   refinement proof is bound to its own implementation and cannot supply another's guarantee. Search counts
   come from `ProofSearchActivity::started()`.
5. Three tests in `borrowed_restoration.rs` (`:554`, `:613`, `:777`) assert only "not accepted", and
   `array_lengths.rs:229` asserts only "some verdict is Rejected with unproved". They pin that the program is
   not accepted, not why.

## What you are unsure about

- The unit TSV lists `borrowed_restoration.rs` as lines 1-751, but the file runs to line 1,021 (I read the
  whole file through 1,021 when the Read tool returned it). Lines 752-1,021 are beyond the unit row; I record
  them above (sections for `:754-1021` tests such as `:931`, `:840`) because I read them, but the unit's
  stated coverage is 1-751. If the other reader of unit 066 was also assigned lines past 751, there may be
  overlap.
- Helper definitions used here (`run`, `judge`, `discharge`, `lower`, `elaborate_over`, `constrained`,
  `constants`, `verdicts`, `assert_kernel_checked`, `certificate`, `store_by_hand`, `domains`) live in
  `tests.rs` before line 4951 or in sibling `tests/` modules (`build_purpose`, `boundary_calls`, `entryless`,
  `references`, `mutable_formals`); I did not read them, so I describe their behavior only from call sites.
- Whether the `mod table_faults;` (`tests.rs:5114`) and `mod loops;` (`tests.rs:7575`) files are covered by
  another unit is not checked here.

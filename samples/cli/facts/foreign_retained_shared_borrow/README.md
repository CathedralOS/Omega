# Retained shared foreign backing

Parents: **FOREIGN-RETAINED-ARGUMENT-BACKING**,
**BUILD-OWNED-TARGET-SELECTION**, and **BUILD-OWNED-NATIVE-BINDINGS** in
`TASKS.md`.

The migration requirement is to preserve this entire shared-loan row and its
composition controls while ordinary named implementations replace target-prefixed
bodies and direct target-keyed `Build::bind` rows replace native helper forwarders.
Locator byte lengths are inferred from the original fixed-byte values. The same
four hosted locators, public contracts, provider authority, backing and loan
occurrences remain the executable obligation.

A shared foreign registration retains the exact original `Buffer` backing until
completion consumes `Pending`; completion observes the original value 37.
Each place transports that same obligation. Its invalid twin writes 99 while
that registration remains live and must reject for the active borrow.

The provider fixture has two slots. Acquisition issues one generation-checked
linear capacity; registration consumes it, completion returns that same capacity,
and release retires it. A separate registration generation changes on every
registration, including reuse before capacity release. Completion clears the
pointer before returning capacity. Invalid transitions trap under the authored
`crashes Trap` contracts. The C self-test checks simultaneous values 37 and 91,
exact capacity return and both generation lifecycles; nine subprocess controls
check exhaustion, null backing/output, stale capacity/pending generations,
duplicate registration/completion/release and release while registered.

The retained pointer reads one `u64` at the original record home. Its 8-byte
extent comes from the declared layout; the counted content projection is a
semantic amount, not an inferred native pointer/count association. The lifetime
obligation forbids overlapping writes and early disposal. It creates no pointer
inside `Pending`, copying permission or implicit write-back. The completion's
observed output has its own explicit `&mut u64` contract.

## Reproduce on Linux

The fixture declares imports and program-entry bindings for all four hosted
target slots. Its preparation script builds the matching ELF, Mach-O or PE
library and checks the same C lifecycle. Native process controls require the
actual deliberate trap status; Python exception translation is not that oracle.

```sh
python3 samples/cli/facts/foreign_retained_shared_borrow/fixture/prepare.py
export LD_LIBRARY_PATH="$PWD/build/foreign_retained_fixture${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
build/gate-cli/debug/omega run --both samples/cli/facts/foreign_retained_shared_borrow/local/main.omg
```

On macOS ARM64, prepare the same fixture and supply its dylib search directory:

```sh
python3 samples/cli/facts/foreign_retained_shared_borrow/fixture/prepare.py
DYLD_LIBRARY_PATH="$PWD/build/foreign_retained_fixture" omega run --both samples/cli/facts/foreign_retained_shared_borrow/local/main.omg
```

The standalone CLI and corpus sponsor an isolated native provider for Terminal
execution. It loads the exact fixed-frame adapter produced with the selected
foreign calling application. The adapter uses the same mapped original cells
across calls; neither per-call snapshots nor token values establish backing or
completion authority. Embedders opt in with an explicit worker command; absence
provides no native execution. Modeled host services retain their own bindings.

All nine places and the shared-alias, enclosing-allocation and resident-whole-move
controls reach actual native/Terminal zero exits on all four hosted targets.
The pinned corpus and fresh public CLI projects exercise both engines; the nine
intended loan rejects still reject. Matching Windows/Linux x86-64 execution is
recorded in [the hosted gate](https://github.com/CathedralOS/Omega/actions/runs/40254747695),
and Linux ARM64 in [its matching-host gate](https://github.com/CathedralOS/Omega/actions/runs/40254749833).
The macOS ARM64 qualification uses the same source revision and oracles.

`tools/ci/compiler_retained_foreign.py` prepares and self-tests the matching C
library, checks the nine intended rejects and pinned positives through the
corpus, then executes fresh copies of all nine authored places and three
composition controls through the actual CLI. It requires both engine exits;
a successful compile or a declined Terminal invocation fails that gate.

| Place | Positive | Invalid twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [write](local/invalid/main.omg) |
| field | [source](field/main.omg) | [write](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [write](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [write](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [write](match_join/invalid/main.omg) |
| constant name | [source](const_name/main.omg) | [write](const_name/invalid/main.omg) |
| a nested machine | [source](other_state/main.omg) | [write](other_state/invalid/main.omg) |
| another machine | [source](other_machine/main.omg) | [write](other_machine/invalid/main.omg) |
| target body | [source](target_body/main.omg) | [write](target_body/invalid/main.omg) |

The matching nine positives and negatives are pinned under
`tests/omega/{pass,fail}/facts/foreign_retained_shared_borrow_*`.
The [shared-alias control](controls/shared_aliases/main.omg) gives C two shared
loans to the same original pointer in one invocation. The
[enclosing-allocation control](controls/enclosing_allocation/main.omg) first
borrows a nested Buffer while preserving the original parent's geometry. The
[whole-move control](controls/resident_whole_move/main.omg) completes a first
registration, moves the realized Buffer through a call and record field, then
registers it again. It retains the allocation seal through the ordinary owned
storage transaction; a scalar resident alone cannot authorize a new root.

The [wrapper-redemption control](controls/wrapper_redemption/main.omg) keeps
Pending debt in one returned field while attempting to publish owned Buffer
backing in another. Preserving a loan-bearing sibling cannot turn that loan
into ownership. The generic owned-sibling declaration under
`tests/omega/pass/core/content_owned_sibling_in_loan_carrier` pins the converse:
a disjoint borrowed field does not take away an actual owned content source.

The [persistent-slot control](controls/persistent_retention/main.omg) replaces
caller-owned storage through an ordinary helper, completes the old registration,
and then observes 91 through the new registration. Its
[twin](controls/persistent_retention/invalid/main.omg) writes that new backing
while the stored loan is still live. The
[restore control](controls/persistent_restore/main.omg) instead restores the old
slot and completes the temporarily installed registration; its
[twin](controls/persistent_restore/invalid/main.omg) writes the still-retained
original backing. Distinct 37/91 values and opposite permitted writes make the
current slot and loan epoch observable. Both pairs have matching pass/fail pins.

The former receiver-namespace refusal was an implementation limit: matching
explicit lifetimes permit the helper once its complete normal-exit loan relation
is carried to the caller. A historical install that is subsequently consumed or
replaced cannot supply that relation, and an unrelated loan at the same path
cannot cover its source. These controls require actual loan transport rather
than preservation of the obsolete namespace restriction.

The controls additionally require early disposal, capacity reuse, repeated
completion and call-scoped retention to reject for their stated obligations. Two simultaneous registrations remain
independent; completing one leaves the other loan live. A post-completion write
and raw values of the same carrier (including a factory with an unused borrow)
remain valid. The [raw borrowed-input control](controls/raw_same_carrier_borrow/main.omg)
also replaces a raw token in caller storage and checks marker 11; its known empty
loan frontier cannot acquire a loan from the helper's unused Buffer borrow. Returning local backing or redeeming borrowed content as independent
owned custody rejects. A separate phantom control needs no
foreign effects and executes in both engines.

## Requirement coverage and limits

- Establishment: the admitted boundary's owner-unique content projection and
  explicit lifetime establish a retained loan. Its scalar token is not a pointer.
- Identity/lifetime: exact caller places and loan occurrences survive erasure,
  nesting, calls, joins and state transfers. Mutable access and disposal conflict;
  completion ends only the consumed occurrence.
- Composition: the consuming completion receives the retained token through the
  nine places. The match join exercises both inputs and records different effects
  in caller storage before completion. The target-body place selects an ordinary
  `PendingTransferCode` implementation and retains the full applied carrier and
  loan. No caller guard re-establishes the loan or authorizes a write.
- Authority: nonspatial dependencies cannot supply reference-result pointers or
  spatial access exemptions. Phantom lifetime spelling supplies no loan. Unpropagated
  persistent stores remain rejected.
- Psi 05–07: checked boundary custody retains its exact source/result projection;
  ordinary loan flow carries the token's dependency without a fabricated
  reference field. Terminal executes the actual C provider under the same custody joins.
- Omega 00–02: the existing borrowed-record path resolves the original home,
  checks access, declared shape, offset and full extent, and supplies the native
  pointer. Protocol tokens remain ordinary transparent one-word records.
- Omega 03–06: native calls use ordinary register/stack and activation homes;
  suspended callers retain their frame backing while helpers transport tokens.
- Omega 07–09/emission: both Linux ELF profiles, macOS Mach-O and Windows PE
  imports execute the authored C lifecycle with native/Terminal agreement.

The hosted execution links above are retained evidence for the original row;
migration qualification records identify the actual producer and matching host.
This row establishes the shared retained-borrow requirement of the two Build
migrations; unrelated source cohorts, retired-syntax deletion, audio acceptance
and other hosted migration executions remain their parent tasks' acceptance.

This covers the direct shared fixed-record rung. The whole-move control covers
ordinary owned movement between completed registrations. Transferring backing
ownership into a foreign protocol, explicit snapshots,
wider/recursive graphs, more general retained provider installations and wider
resident codecs remain open.

The former shared-only mutable rejection is superseded by the
[retained mutable row](../foreign_retained_mutable_borrow/README.md).

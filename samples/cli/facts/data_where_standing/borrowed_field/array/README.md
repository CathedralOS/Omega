# Indexed borrowed field production

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, recursive/reference
predicate read inventories. This is another production of
[data_where_standing](../../README.md), retaining the bound and exact operation
of the [nested borrowed-field production](../nested/README.md).
`Holder.counters` stores two shared references to `Counter`; each Counter stores
a shared scalar reference. The public consumer requires only `index < 2` to
form the indexed selection, then calls `read(holder.counters[index].value)`.
The checked result equation and Counter's standing predicate must justify
`total = 70 + value` before any runtime canary guard.

| Place | Positive | Predicate-removal twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [twin](local/invalid/main.omg) |
| field | [program](field/main.omg) | [twin](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [twin](parameter/invalid/main.omg) |
| call_result | [program](call_result/main.omg) | [twin](call_result/invalid/main.omg) |
| match_join | [program](match_join/main.omg) | [twin](match_join/invalid/main.omg) |
| const_name | [program](const_name/main.omg) | [twin](const_name/invalid/main.omg) |
| other_state | [program](other_state/main.omg) | [twin](other_state/invalid/main.omg) |
| other_machine | [program](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) |
| target_body | [program](target_body/main.omg) | [twin](target_body/invalid/main.omg) |

Every twin removes only Counter's default predicate. The dynamic selector,
index formation requirement, layouts, transport and runtime oracle are identical.
The const-name production retains the independent HIGH/LOW Backing constants;
those concrete callers cannot establish the arbitrary public consumer's bound.

The local and parameter places retain separate local-copy and incoming-value
routes. Field stores the Holder in Envelope; call_result receives an owned Holder
from ordinary relay. Match joins distinct original/swapped reference arrays and
executes both arms with both selector values. Other_state terminal-transfers
Holder and selector into a genuinely nested machine; other_machine forwards
both to a separate consumer. Target_body selects ordinary named implementations
and independent 17u64/29u64 tag checkers through all four hosted Build bindings.

Each entry checks high=60 and low=-17, exact results 130/53, both Counter
before=17/after=53 fields, distinct markers 19/23, and Holder prefix=31/suffix=47.
Both array orders and both selector positions are observed. The field entry
also checks the transported Envelope's Holder. The consumer checks its actual
transported Holder canaries after the arithmetic, detecting collateral copies
without supplying an arithmetic premise. Wrong selector/array order, omitted
loads, branch substitution or lost descriptor transport must fail the oracle.

Establishment, exact descriptor/value identities, complete possible source
inventories, readable access and lifetimes remain separate obligations. No
reference type or index alone authorizes its pointee.

Exact main/Build copies are pinned under
`tests/omega/{pass,fail}/facts/data_where_standing_borrowed_field_array_<place>`.
Positive pins declare native and interpreter exit zero; each twin pins the
intended exact-i32 overflow diagnostic.

The independent formation and identity controls retain Counter's bound:

- [Wrong selected equality](controls/wrong_selected_equality/invalid/main.omg)
  wrongly promises equality with element zero for an arbitrary legal selector.
- [Missing index bound](controls/missing_index_bound/invalid/main.omg) omits
  the selector requirement. A scalar bound cannot prove the array access.
- [Saved selector](controls/saved_selector/invalid/main.omg) saves a selected
  Counter descriptor, changes the selector, then wrongly promises equality with
  the newly selected element. Equal types do not redirect the saved descriptor.

These controls have matching reject pins with suffixes
`wrong_selected_equality`, `missing_index_bound` and `saved_selector`.

The evidence covers these applicable requirements:

- **Establishment:** construction establishes the full Counter predicate while
  its backing is live. The const-name place uses independently qualified Backing
  constants. Public consumers require only the selector bound; their callers and
  later runtime guards do not supply the arithmetic premise.
- **Identity and lifetime:** input observations retain their storage version and
  complete possible suppliers. The runtime selector chooses the pointer. An
  alternative-value proof does not choose a physical origin or grant a loan.
  The saved-selector control separates a captured descriptor from a later index.
- **Composition:** all nine places transport the same bound and operation.
  `read`'s checked result equation carries the selected scalar into the exact
  addition. Match executes both arms and selectors with distinct results.
- **Soundness:** removing only the predicate rejects in every place. Formation,
  wrong-element and saved-selector controls reject independently. This readonly
  production claims no new write authority, restoration or cleanup behavior.
- **Psi 05–07:** the same source checks and lowers into Terminal Psi, which
  actually executes the authored exit-zero oracle. Reference fields remain
  descriptors; scalar observations carry their own completed-value evidence.
- **Omega 00–02:** owned and borrowed Holder transport, returned aggregates,
  reference-array layout and calling plans retain both selected values and
  neighbor canaries. All four hosted entries are bound. Target bodies retain
  independent X86/Arm provider-tag checks.
- **Omega 03–06:** macOS ARM64 execution observes the two pointer crossings,
  selector-dependent result, signed i32 arithmetic and helper-call behavior.
  The row does not claim register exhaustion or an activation-size boundary.
- **Omega 07–09 and emission:** the emitted macOS ARM64 image exits zero and
  agrees with actual Terminal Psi execution for every positive. Other hosted
  targets are bound and source-checked; their native execution remains unverified.

Retained optimized producer R4 (source `111e70eb2b599aa6c49ffe3ee0e63114ab203956`
plus compiler patch SHA-256
`b28c8fc236dc30898f91e9c964eacee30d24c4f2a3f133209280bd53ff284eab`)
checks all nine positives, rejects all nine predicate-removal twins for overflow,
and runs all nine positives at native/Terminal `0/0` on `macos_arm64`.
The parent migration remains open. Exact producer, source/oracle hashes, raw
observations and independent review are retained in the claim/commit evidence;
local records are under `build/scalar-collection-read-20261102`.

# Predicate handback through a stored reference

A saved observation of an unwritten array element stays equal to 7 across a write to a disjoint element.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, current record-field predicate
handback through stored references. This is another production of the existing
saved-observation fact, using the same `[7,1]`, independent owned copy,
`copied[0]=4`, original index-one store and `agree` requirement as the
[indexed-store production](../predicate_handback/README.md).

## Acceptance

`Holder.items` stores a mutable reference to the caller's actual two-element
array; neighbors start at 11 and 13. `preserve` requires and ensures
`kept(&holder.items)==true`. It copies the referent into independent owned
storage, writes copied element zero, and writes that value to original element
one. The terminal-forwarded helpers take the exact array reference and have no
Requires or Ensures; their actual writes must establish the root guarantee.

After that store, a runtime check reads untouched `copied[1]`, expecting 1 and
crashing Trap otherwise. This observes the original copy contents: omitted or
partial copies, copying descriptor words, and copy-as-alias cannot silently
pass. It is not a guard for `kept[0]` or the store; the invalid index-zero twin
still has a normal path with copied tail 1 and must reject its own guarantee.
The Trap route is published through the helper/caller chain and the target
requirement. Positive programs retain Expected exit0.

`agree(saved,current)` requires `saved==current && saved==7`; it consumes the
carried fact before the caller's runtime canaries. Main checks current original
contents `[7,4]`, neighbors 11/13, and the original backing array directly after
the stored reference's last use. These reads detect detached or redirected
storage. The match place runs both arms and checks distinct marker events
11 and 13. Scalar snapshots supply runtime observations, not new equality guards.

Every twin differs only in the final original destination index `1→0`.
It must reject `cannot prove ensures contract for exit from preserve`.
Unrelated formation or access refusals do not qualify.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [source](../local/stored_reference_handback/main.omg) | [twin](../local/stored_reference_handback/invalid/main.omg) | Saved scalar crosses preserve; current and original backing are observed afterward. |
| field | [source](../field/stored_reference_handback/main.omg) | [twin](../field/stored_reference_handback/invalid/main.omg) | Note.observed transports the immutable scalar. |
| parameter | [source](../parameter/stored_reference_handback/main.omg) | [twin](../parameter/stored_reference_handback/invalid/main.omg) | hold receives the fact explicitly and consumes it after preserve. |
| call_result | [source](../call_result/stored_reference_handback/main.omg) | [twin](../call_result/stored_reference_handback/invalid/main.omg) | read_kept publishes the saved observation through its exact result contract. |
| match_join | [source](../match_join/stored_reference_handback/main.omg) | [twin](../match_join/stored_reference_handback/invalid/main.omg) | Both selected arms capture the fact; main executes both marker paths. |
| const_name | [source](../const_name/stored_reference_handback/main.omg) | [twin](../const_name/stored_reference_handback/invalid/main.omg) | MARK names 7 in construction and the consumer oracle. |
| other_state | [source](../other_state/stored_reference_handback/main.omg) | [twin](../other_state/stored_reference_handback/invalid/main.omg) | preserve terminal-forwards the stored field reference to its nested store_original; check consumes the saved/current scalars. |
| other_machine | [source](../other_machine/stored_reference_handback/main.omg) | [twin](../other_machine/stored_reference_handback/invalid/main.omg) | A top-level store_original receives the stored reference; read_kept consumes the fact before returning. |
| target_body | [source](../target_body/stored_reference_handback/main.omg) | [twin](../target_body/stored_reference_handback/invalid/main.omg) | Build selects HostedObservationBody on all four hosts; an explicit backing lifetime forms the public stored-output relation. |

Pins are `tests/omega/{pass,fail}/facts/saved_observation_snapshot_stored_reference_handback_<place>_{exit,rejected}`.
Source and Build inputs match byte-for-byte. All positives bind ProgramEntry for
windows_x86_64, linux_x86_64, macos_arm64 and linux_arm64; actual execution here
is macos_arm64 only.

## Requirements and evidence

- **Establishment:** `[7,1]` establishes kept. The original checked array copy
  and disjoint write preserve element zero. Neither the copied-tail guard nor
  the desired postcondition establishes that fact.
- **Identity and lifetime:** the exclusive descriptor stays in Holder; copying
  array contents creates independent storage. Readable place presentation
  retains the exact selected carrier and endpoint Referent crossings together.
  Descriptor matches retain zero endpoint crossings and their captured extent.
  See [structural access](../../../../../wiki/spec/terminal-psi/structural_access.md).
- **Composition:** the nine transport places retain the same fact, bound and
  operation; root guarantees follow the actual forwarded bodies. The boundary
  backing region distinguishes its stored leaf from the outer record loan.
  A lifetime relation does not identify a pointer or authorize provider behavior.
- **Soundness:** Copy permission, readable access, immediate lineage, exact
  destination and normal-return custody remain independent. Non-copy arrays,
  write-only reads, invalid bounds and exclusive-descriptor reuse still reject.
  See [ownership](../../../../../wiki/spec/language/ownership.md) and
  [lifetimes](../../../../../wiki/spec/language/lifetimes.md).
- **Psi 05–07:** existing CopiedStructuralPlace carries the full source path;
  source, selectors and reference crossings are not reconstructed by a consumer.
  Both engines execute the copied-tail check and current/backing canaries.
- **Omega 00–02:** ordinary pointer/record/array layouts and call placement keep
  the stored address distinct from its referent. Four authored Build entries
  select ordinary checked providers; no target or provider identity supplies proof.
- **Omega 03–06:** actual copies remain independent across writes and calls;
  unchanged carrier neighbors and backing observations expose wrong homes or offsets.
- **Omega 07–09 and emission:** retained native images and canonical Terminal
  envelopes accompany numerical native/Terminal agreement. The match markers and
  copied tail make wrong branches and missing copies observable.

## Qualification and remaining scope

Fixed base: `52929b02d410af19e925c30218b1b00626734041`. Optimized B0 producer
`70c4d121c341de56c8b3e7fe7b39aa92a92a952e` has identical compiler/library inputs.
The final H4 sources check and all twins reject their own guarantee on B0;
the local native/Terminal command refuses before either engine because preserve
has no executable plan for statement0, `let mut copied: [u64;2] = holder.items`.
The earlier H2 local and H3 target refusal records remain separate.

The initial target-body draft omitted the explicit public backing lifetime and
was correctly refused for ambiguous output elision. The H3 formation correction
does not count as a compiler repair. H4 then strengthened the runtime oracle to
observe copied contents; earlier H3 results do not qualify those new bytes.

Trace-free R1 producer `62a790db60eb699fcfada57ee73ad1b15d58b536` checks all nine
H4 positives and all nine twins retain their own Ensures refusal. All nine CLI
programs execute with native/Terminal exit0/0, including the copied-tail and
original-backing checks. The focused corpus observes 25 source outcomes
(12 checked, 13 rejected) and 12 native/Terminal oracles, including exact stdout
and exit70 controls. Run records under `build/stored-reference-field-handback-20261029/`
bind the actual producer, H4 consumer sources, target and retained artifacts.
Other hosts have authored entries, not execution evidence from this executor.

The parent remains open: whole-home replacement and arbitrary recursive body
handback need their own applicable evidence; proposition migration, mutable
recasts, remaining customers and retired range-suffix deletion keep their
existing TASKS acceptance. This row closes the stored-reference production only.

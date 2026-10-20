# Closed opaque carrier exchange

Parent: [OPAQUE-BY-VALUE-BOUNDARY-ABI](../../../../TASKS.md).
Requirement: preserve the exact selected closed Inert carrier and one owned
occurrence through by-value foreign results, transport and consuming arguments,
including replacement of one field without disturbing its sibling. This is
host-boundary evidence; independently compiled Omega-component agreement remains
open.

**Closed nested Inert carrier transport preserves libc host numbers 66051 and
513 and replacement 1027 while retaining the disjoint sibling.**

Every witness uses the same selected
`AddressCarrier<AddressCarrier<AddressWord>>` representation and actual libc
`inet_makeaddr` / `inet_lnaof` operations. The guest cannot inspect or manufacture
an address representation. `consume` consumes the first address, replaces its
field with a third independently returned address, and then consumes the
untouched sibling and replacement. A comparison observes the foreign results;
it does not introduce a proof fact or permit an otherwise unproved operation.
Success exits zero. A place witness crashes on a mismatch; the original root
retains its `observe` and `observe_replacement` operations and Console route,
which exits 71 on a mismatch.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local](local/main.omg) | [cleanup](local/invalid/main.omg) |
| Field | [field](field/main.omg) | [cleanup](field/invalid/main.omg) |
| Parameter | [parameter](parameter/main.omg) | [cleanup](parameter/invalid/main.omg) |
| Call result | [call result](call_result/main.omg) | [cleanup](call_result/invalid/main.omg) |
| Match join | [both incoming arms](match_join/main.omg) | [cleanup](match_join/invalid/main.omg) |
| Const name | [named construction inputs](const_name/main.omg) | [cleanup](const_name/invalid/main.omg) |
| Another state | [owned arrivals](other_state/main.omg) | [cleanup](other_state/invalid/main.omg) |
| Another machine | [owned forwarding](other_machine/main.omg) | [cleanup](other_machine/invalid/main.omg) |
| Target body | [four checked bodies](target_body/main.omg) | [cleanup](target_body/invalid/main.omg) |

The const-name witness names the two construction inputs. [Constants](../../../../wiki/spec/language/constants.md)
require recursively copyable pure values; [semantic evaluation](../../../../wiki/spec/language/evaluation.md#invocation-admission)
excludes runtime-only assumed bindings. Actual affine host-owned occurrences
therefore cannot be compile-time constants. The named input establishes the same
exact construction input at runtime, as in the existing
[linear-claim const witness](../linear_claim_retirement/const_name/main.omg).
This place claims named construction-input transport, not storage of an affine
occurrence in a constant.
All twins differ only by adding nominal cleanup to `AddressWord`; the consuming
operation and inputs are identical. Expected diagnostics require both
`contains independently invoked nominal cleanup` and
`v1 carriers must be inert storage`. Positive and reject copies are pinned under
`tests/omega/{pass,fail}/facts/opaque_closed_carrier_through_*`.

The applicable [requirements and evidence](../../../../AGENTS.md#requirements-and-evidence)
are established as follows:

- **Establishment:** the selected `OpaqueRepresentation` authorizes the closed
  carrier. Complete-graph Inert admission rejects independently invoked cleanup,
  including the identical-layout cleanup twin. A completed actual foreign call
  supplies each owned value; a token alone supplies no external ownership.
- **Identity and lifetime:** each foreign result has a distinct owned occurrence.
  Moves retain it, consuming `host` retires it, and the replacement is fresh.
  Consuming and replacing `pair.first` preserves `pair.second`. The trusted
  invocation store retains the actual C values and their exact selected carrier
  commitment; a consumed token cannot be replayed or rebound to another selection.
- **Composition:** helpers, results, both match paths, state arrivals, enclosing
  fields and target bodies feed the same operation. The exact foreign inputs
  and result comparisons remain unchanged across the row.
- **Soundness boundaries:** opaque ownership stays affine and inaccessible to
  guest field projection. Physical ABI copies do not duplicate semantic
  ownership. Cleanup-sensitive carriers reject before execution. No erased
  mathematics or termination assumption is introduced.
- **Checked-to-Terminal Psi (`Psi 05–07`):** canonical Terminal execution preserves
  opaque identities across these transports and record transactions. Public
  `omega run --both` uses the admitted runtime provisions, not an injected corpus
  handler or a second source checker. No borrow-restoration, proof-tag, `dyn`,
  atomic or ranking claim is exercised by this row.
- **Provider settlement, geometry and ABI (`Omega 00–02`):** exact normalized
  foreign bindings select the actual library functions. Default native policies
  retain the complete source-free calling application, recursive public C shape,
  selected opaque commitments and validated by-value argument/result placements.
  Equal byte size alone does not admit float, array, shifted or cyclic shapes.
- **Instruction selection and activation (`Omega 03–06`):** native calls carry the
  four-byte address record through ordinary helper/state/field storage and the
  selected Linux x86-64 C register lanes. This row does not establish stack-slot
  boundary crossings, indirect aggregate returns or other ISA placement classes.
- **Encoding and emission (`Omega 07–09`):** the emitted Linux ELF executes its
  actual versioned libc relocations and agrees with Terminal execution on all
  observed values. Other image formats and branch-range boundaries are not
  inferred from that agreement.

```sh
omega run --both samples/cli/facts/opaque_closed_carrier_exchange/main.omg
omega run --both samples/cli/facts/opaque_closed_carrier_exchange/local/main.omg
omega --check samples/cli/facts/opaque_closed_carrier_exchange/local/invalid/main.omg
```

At base `591336bdc9e0b38268cbfa63b17aabb92a5d2389`, the frozen original root
ran natively at its then-authored exit 70, while public Terminal execution
declined because no trusted exact inet implementation was available. After the
repair, the byte-identical root produced native 70 / Terminal 70 before the
success-oracle migration. The zero-oracle root and nine positive places then
produced native 0 / Terminal 0 on Linux x86-64, and every cleanup twin rejected
with the fact-sensitive diagnostics. Producer digests, source hashes, exact
commands and raw outputs are retained in this change's qualification evidence;
these observations are distinct from final candidate review.

Every positive declares the four hosted `ProgramEntry` bindings. Linux x86-64 is
the observed execution target. Linux ARM64 and macOS ARM64 bindings are declared
but have no matching-host runtime observation here. This exact BSD inet API has
no verified Windows provider in this row: a Windows entry declaration does not
establish provider availability. Full crosshost coverage therefore remains open.

The parent also still requires independently compiled producer/consumer exchange
and exact composition-edge rejection of provider, carrier, target, lifecycle,
multiplicity and historical-selection drift. The host adapter and equal layouts
do not establish that independent-component acceptance.

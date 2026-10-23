# Closed opaque carrier exchange

Parent: [OPAQUE-BY-VALUE-BOUNDARY-ABI](../../../../TASKS.md).
Requirement: preserve the exact selected closed Inert carrier and one owned
occurrence through by-value foreign results, transport and consuming arguments,
including replacement of one field without disturbing its sibling. This is
host-boundary evidence and initial independent closed Affine input/result
exchange. Copy, qualified or claim-bearing component inputs, remaining mismatch
admission and replacement lifecycle acceptance remain open.

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

Every positive declares the four hosted `ProgramEntry` bindings. Linux x86-64 and
macOS ARM64 are observed execution targets: all nine places and all nine
independent places run `omega run --both` on macOS ARM64 with native exit 0 and
Terminal agreement, and every cleanup twin rejects at checking. Linux ARM64
bindings are declared but have no matching-host runtime observation here. This
exact BSD inet API has no verified Windows provider in this row: a Windows entry
declaration does not establish provider availability. Full crosshost coverage
therefore remains open.

### Matching-host execution (macos_arm64)

Measured on `macos_arm64` with the wave's shared producer at `9ff26a87`
(`opt_level 1`; its compiler source is identical to this row's base
`c1c6bc8`, the only intervening change being `float_result_equality` sample
content):

```sh
omega run --both samples/cli/facts/opaque_closed_carrier_exchange/local/main.omg
omega --check samples/cli/facts/opaque_closed_carrier_exchange/local/invalid/main.omg
python3 tools/corpus_gate.py --native --record --filter pass/facts/opaque_closed_carrier_through,pass/facts/opaque_independent_closed_carrier_through
python3 tools/corpus_gate.py --both --record --filter pass/facts/opaque_closed_carrier_through,pass/facts/opaque_independent_closed_carrier_through
```

The gate runs used a `corpus_runner` built in this worktree at `98309d77b143`
(`mbx test --no-run -p compiler --test corpus_runner`): the native leg recorded
18 `built ... exit:0` rows and the combined leg 18
`built ... exit:0 interpreter-exit:0 interpreter:agrees` rows, each splice
insertion-only (+18 lines, 0 deletions in its golden).

All nine places and all nine independent places report `native exit 0` and
`interp exit: 0`; the eighteen cleanup twins reject at checking with
`contains independently invoked nominal cleanup` and `v1 carriers must be inert
storage`. Each positive differs from its twin only by the added
`AddressWord::drop` obligation. All eighteen fixtures carry macOS ARM64 native
rows (`built ... exit:0`) in `tests/omega/corpus_native_macos_arm64.txt` and
interpreter-agreement rows (`built ... exit:0 interpreter-exit:0
interpreter:agrees`) in `tests/omega/corpus_both_macos_arm64.txt`, recorded at
`98309d77b143`; the check-leg rows already carry `check-target:macos_arm64`
facts. The root `main.omg` retains its Console/std
route and needs an accepted macOS program-entry binding (`omega update` review)
before it checks on a fresh host.

The parent still requires complete composition-edge rejection of provider,
carrier, target, lifecycle, multiplicity and historical-selection drift. The
initial independent exchange below covers closed Affine occurrences; the host
adapter and equal layouts do not establish the remaining acceptance.


## Independent closed Affine exchange

The [two-package root](independent/main.omg) and the places below keep the same
fact, inputs and consuming operation. [The producing package](independent/producer/main.omg)
returns each address through `AddressBridge::make`; the consumer selects
`CompositionMode::Independent`. Both packages choose their own exact closed
representation. Every `AddressBridge::host` call sends the owned address back
across the component edge, where the producing package invokes the same actual
`AddressParts::host` operation. The receiver still consumes and replaces the
first field while preserving its sibling. Its helper and transport places do
not inspect or reconstruct the opaque carrier.

The producer's root deliberately retains its original direct `AddressParts::host`
call. Selecting the complete `AddressBridgeProvider` application exports the
checked `host` member even though no internal root call happens to use it.
Exports derive from selected satisfaction; outgoing imports derive from actual
calls. The complete retained plan preimage is replayed against its commitment
and exact verified candidate/body identities, without donating a receiver
contract or turning a description into execution authority.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local](independent/local/main.omg) | [cleanup](independent/local/invalid/main.omg) |
| Field | [field](independent/field/main.omg) | [cleanup](independent/field/invalid/main.omg) |
| Parameter | [parameter](independent/parameter/main.omg) | [cleanup](independent/parameter/invalid/main.omg) |
| Call result | [call result](independent/call_result/main.omg) | [cleanup](independent/call_result/invalid/main.omg) |
| Match join | [both incoming arms](independent/match_join/main.omg) | [cleanup](independent/match_join/invalid/main.omg) |
| Const name | [named construction inputs](independent/const_name/main.omg) | [cleanup](independent/const_name/invalid/main.omg) |
| Another state | [owned arrivals](independent/other_state/main.omg) | [cleanup](independent/other_state/invalid/main.omg) |
| Another machine | [owned forwarding](independent/other_machine/main.omg) | [cleanup](independent/other_machine/invalid/main.omg) |
| Target body | [four checked bodies](independent/target_body/main.omg) | [cleanup](independent/target_body/invalid/main.omg) |

Each twin changes only the producing carrier's nominal cleanup obligation in
[the invalid producing package](independent/producer_invalid/main.omg). Its
consumer source is identical to the positive. The four hosted entries remain
in every positive build, while execution observations are Linux x86-64 only.
The paired sources are pinned under
`tests/omega/{pass,fail}/facts/opaque_independent_closed_carrier_through_*`.

The applicable requirement categories above extend through the component edge:

- Establishment uses each package's own checked callable and complete contract
  capsule, selected representation and full calling application. The receiver
  cannot donate a contract or geometry to the producer.
- Identity and lifetime retain the producing artifact, actual child invocation
  namespace and owned result until the receiver commits it. An owned input is
  authenticated against its actual opaque host atom and ordinary affine
  frontier; a real installed child accepts that exact tuple before the parent
  commits its transfer. Pending calls retain the original scalar/structural
  operands and atoms. Sponsor suspension resumes the same child without
  rereading a consumed parent home, replaying committed effects or replenishing
  its original total budget. The focused six-call fuel witness compares every
  effect and resource attribution with ordinary execution, including exhausted
  total budgets. The shared host store consumes the exact nonrecycled token;
  artifact-local type coordinates are compared through their owning identities
  rather than as global numbers.
- Composition joins the exact requirement, provider, target and full ABI/opaque
  applications at each selected export. Every place performs the same field
  replacement while preserving the sibling.
- Soundness keeps provider selection, execution authority and publication
  separate. Descriptions and equal layouts do not mint an installed address,
  an opaque occurrence or a host result. This input lane requires owned Affine,
  closed Inert atoms. It explicitly refuses claims, memberships, loans,
  materialized backing/view sidecars, active subtree metadata and other
  multiplicities rather than silently dropping their custody. Ancestor cases
  and disjoint siblings remain with the residual parent.
- Checked-to-Terminal lowering retains the requirement's own source declaration
  and checked contract. A real independently supplied child executes the
  producer with admitted owned inputs, owned completion and separate effect
  namespaces; the driver supplies no fabricated callback result. Preparation
  executes no child operation, and all fallible input checks precede the
  parent's ordinary whole-root or projected affine retirement.
- Provider settlement and ABI realization retain independent producing inputs
  and materialize the exported calling policy from those inputs. The consumer
  independently checks its complete calling application and selected carrier.
- Instruction and frame realization compile the export as a separate callable
  object and entry bridge. Complete product admission compares their actual
  stack bytes and alignment with the exact full provider-plan contribution;
  smaller or misaligned claims and unrelated plans reject. Publication and
  process admission also rejoin the actual root's imports and full call bindings
  with its retained image/runtime roster, rejecting missing, empty or unrelated
  products. Checked transitive authority remains on the receiving call plan.
- Encoding emits a distinct Linux ELF shared component with a strong
  image/slot-qualified protected export. The root names its content-addressed
  sibling in `DT_NEEDED`; GNU version requirements cover actual versioned
  foreign objects. Unversioned component-only roots omit the GNU version tags
  and their fixups; versioned and mixed imports retain their exact version
  roster. Canonical macOS checking realizes a separate `MH_DYLIB`
  with an exact export trie and `@loader_path` sibling binding, using its own
  segment indices for rebases and eager imports. Ordinary flat publication
  retains distinct root and component receipts and validates their exact bytes.
  The macOS files have been produced on Linux; their macOS execution is now
  observed: the nine independent places run natively at exit 0 with Terminal
  agreement on `macos_arm64` under the shared producer at `9ff26a87`.
  This initial process route does not establish hot replacement, stable-handle
  eras or cross-host loading.

Raw observations remain in the existing swarm qualification record, bound to
actual producer revisions and accepted project locks. Full provider/carrier/
target/lifecycle/multiplicity/historical-selection mismatch controls, Copy,
qualified or linear component inputs, materialized/view-backed inputs, and
matching-host execution on the other hosted targets remain separate acceptance
until demonstrated. The fact row and focused fuel witness do not close those
remaining parent requirements.

## Native component description publication

COMPONENT-SUBSTRATE additionally uses this unchanged independent row to check
native-description metadata. The nine independent corpus positives require two
native component descriptions apiece. The source-free consumer checks the exact
Terminal subject, emitted realization identity and entry-bridge-inclusive stack
obligation. Publication receipt controls reject missing identity, omitted stack,
missing files and sibling-export substitution. These are artifact obligations;
they neither expand the opaque ABI nor establish component replacement support.

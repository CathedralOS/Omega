# Admitted executable installation

There is no general executable-memory capability, arbitrary bytes-to-code
conversion, JIT, or self-modifying-code route. An immutable reusable `Artifact`
is eligible only through the sealed `Artifact::AdmittedExecutable` qualification.
Admission binds exact content, identity, relocation/proof payload, footprint,
and placement plan. Packages cannot self-establish it; mutation destroys it.

## Placement lifecycle

These are semantic states, not prescribed generic source type names:

| Transition | Required result |
| --- | --- |
| Canonical decode and PCC/contract admission | Immutable admitted artifact. |
| Linear `CodePlacement` (write, no execute) plus borrowed artifact | Materialized declared sections/relocations. |
| Freeze | Readable, non-executable immutable placement. |
| Validate exact final bytes and footprint | Validated placement bound to those bytes and authority. |
| Installer provider and visibility completion | Linear installed code, readable and executable. |

The artifact is borrowed, not consumed. Placement is linear: validation cannot
be transplanted to other bytes or spent twice. A certificate may remain
reportable/reusable while installation consumes the validated placement.
Failed linear transitions return every input. Content, proof, and final-byte
commitments are distinct domain-framed strong digests; compact compatibility
values are report keys, and exact bytes/placement evidence remain available for
acceptance comparison.

Materialization resolves only sealed entry/data identities into a private copy.
Installation separately validates W^X, target cache/order and instruction-fetch
visibility, and audience through one contracted provider operation. Scope binds
the admitted artifact, placement, and audience. `CodePlacement` composes physical/
virtual extent authority and constraints, not a parallel authority family or
replaceable dispatch binding. Requirement binding is separate and later.

Every route to execute permission follows this rule. Translation providers need
admitted-artifact provenance; checked assembly retains the same installation
authority and reach obligations. Device firmware/GPU/NIC uploads belong to
device providers, not host executable artifacts. AP startup installs a
compiler-produced low-memory trampoline and invokes a target boot protocol;
it does not generate runtime host code.

## Visibility and retirement

| Audience | Gate |
| --- | --- |
| Dormant/local target | Local installation completion. |
| Future remote fetcher | Visibility completion before entry. |
| Possible current executor | Component replacement and quiescence. |

Visibility enables future entry; quiescence permits retirement of existing code.
They are distinct facts, not interchangeable tokens. Patching live code uses
admitted fragments at declared sites through the replacement path. Installation
reports `HardwareEnforced` or `ConventionOnly` W^X; an `Unsupported` provider
rejects. A synchronous visibility operation may suspend under its declared
contract; nonblocking completion needs an explicit provider protocol.

Retirement binds the exact installed realization and proves quiescence, execute
removal, restored write authority, and required provider-canonical completion
facts before returning placement. Incomplete drain keeps the installed
realization and accepted receipt quarantined. A stale-entry fault presents that
exact opaque context, not a compact report ID. Installed roots borrow installed
code and therefore prevent reclaiming it while entry remains possible.
See [component publication](component_publication.md).

## Control-flow integrity

Preventing code injection does not establish legal forward-edge targets within
already-admitted code. Indirect calls require sealed requirement-compatible
entry references/descriptors retaining satisfier and contract identity; component
boundaries use bindings, not exported local descriptors. Checked return integrity
derives from memory safety, sufficient WCSU, and compiler-owned live/parked
control state that ordinary source cannot address. Assembly retains stack/control
effects; opaque providers need admitted call/state exits or hardware isolation.
Missing evidence rejects. Independent final-byte transfer validation and optional
hardware hardening are separate assurance work, not substitutes for those rules.

## Artifact admission and boot

The canonical native component container has bounded length-delimited tables,
checked arithmetic/ranges, nonoverlap, a closed relocation vocabulary, and
required PCC/contract/footprint sections. It has no constructors, scripts,
ambient imports, recursive metadata, or permissive semantic extensions.
Informational sections grant no authority; meaning/trust-bearing sections are
required. A UEFI PE/COFF envelope is not the component format.

Decode/structural validation yields only an immutable candidate. Admission is
separate. Exact content and normalized semantic promises define identity;
presentation order and informational sections do not. Backend translation takes
the validated carrier and rejects target or relocation mismatch atomically.
This does not imply a completed byte-format specification; the owning artifact
and PCC work remains on [TASKS.md](../../../TASKS.md).

The boot base case keeps the same boundary: trusted build validates and signs an
admitted identity; secure boot authenticates and gates entry; measured boot
records the entered identity; the boot-admitted installer admits later artifacts.
Measurement alone never establishes admission.

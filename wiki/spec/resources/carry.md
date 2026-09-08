# Carry demands and runtime preservation

Carry checking describes retention across execution transitions, independently
of copyability and ownership. The compiler interprets one closed type property:

```omega
data PerCpuLease [linear, carry(
    suspension: allowed,
    cpu: same,
    thread: any,
    address: movable,
)] {
    cpu_key: u64;
}
```

| Axis | Alternatives |
| --- | --- |
| Suspension | Allowed or forbidden. |
| CPU | Same CPU or any CPU. |
| Host thread | Same thread or any thread. |
| Address | Stable or movable. |

The property lowers directly to compiler facts, not ordinary core data, a trait,
or a policy-machine result. Ordinary data composes its live fields and explicit
type-wide policy using the most restrictive demand on each axis. Accepted
resource claims originate strict. Their result contracts may establish
`Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, or
`Carry::MovableAddress`; `Carry::Portable` denotes all four. Checked resource
transformations inherit permissions through their provenance mapping; combined
origins retain the most restrictive demand.

A claim's carry entry belongs to its undischarged permission provenance, not
the current predicate-fact set. A cast cannot erase it; consumption or transfer
changes its ownership. Fresh unqualified data has no claim entry and follows
structural/type-wide carry. Unique provenance mappings may infer through moves,
splits, loans, and aggregates; ambiguous inheritance rejects.

Exclusive cross-activation transfer requires ownership and carry/runtime
compatibility. Shared references additionally require a sanctioned shared-access
contract. Copyability only authorizes duplication; it is not shareability.

## Runtime admission

Each lowered activation has a fixed nonmoving stack provisioned from
[whole-call-graph WCSU](storage.md). Portable activations require no affinity
fact. A machine that may retain CPU/thread-restricted values requires its runtime
to establish the corresponding preservation, commonly through a borrowed or
consumed affinity capability. Address stability for stack residents follows
from that fixed `StackLease`; there is no provider-selectable movable
continuation-storage mode.

Carry checking uses canonical place liveness at semantic transitions. A
suspension-forbidden live value rejects an explicit semantic suspension point.
After installation closes bounded reach rows, a live mask or affinity token may
make a call locally inadmissible but cannot change its published reach. Provider
selection cannot widen or erase the resolved suspension ceiling.

Architectural preemption can pause and restore opaque state at any instruction
without being semantic suspension. A host that migrates execution outside
declared semantic points must establish activation-wide CPU/thread preservation
whenever the machine may retain a restricted value, or admission rejects.
Checked providers derive this; opaque providers supply an admitted receipt.
The receipt does not alter the actual host's behavior.

Interrupt masking and scheduler-switch suppression are distinct linear tokens.
The former defers delivery; the latter prevents an Omega activation switch but
cannot prevent a host kernel from preempting its thread. Architectural scheduling
may remain arbitrary while cancellation, migration, and replacement occur only
at explicit safe points.

WCSU, linear consumption, external-loan permissions, and carry share canonical
place-liveness traversal, not an algebra. Local checks combine liveness and
carry; runtime admission joins accumulated demands to selected providers.
Future composition proofs may add interleavings, protocol state, and liveness
evidence without reinterpreting source attributes or replacing either judgment.

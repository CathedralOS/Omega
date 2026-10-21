# Owned initialization rejection

`attempt` is an embedding entry, not a hosted application. The host supplies an
already admitted `Granted & Vacant` extent and binds its actual allocation before
invocation. The selected `MemorySupply` body runs normally; equal profile data
without that completed call is not initialization authority.

The entry initializes `Word { value: 17 }`, checks the returned value and typed
failure payload, and returns the original vacant extent with its value. The
embedding witness uses these inputs:

| Base | Length | Actual backing access | Expected reason argument |
| --- | --- | --- | --- |
| 4096 | 4 | Stable read/write | 1: required 8, available 4 |
| 4097 | 16 | Stable read/write | 2: modulus 8, residue 0 |
| 4096 | 16 | Stable read | 3: incompatible supply |

The host additionally checks unchanged allocation identity, origin, lineage,
geometry and every backing byte, then rejects a second handback. Reason 99 must
trap. Omitting the actual allocation must reject rather than interpreting the
numeric base as a pointer. The compiler primitive cannot be selected as a host
import.

Export with `omega inspect-terminal --machine attempt --output <artifact>
main.omg`. The current execution witness and its immutable artifacts are recorded
in the resident claim/qualification continuation. This fixture alone does not
claim native rejection, successful initialization, resident access or Cathedral
boot acceptance. Those require their separate execution witnesses.

# Buffered conversion preserves the value

A caller supplies a buffer, encodes a pair and decodes it once. The returned
pair must equal the original for every admitted pair, and the caller keeps the
same output storage. A successful tag alone is insufficient.

Parent: **LIBRARY-REPRESENTATION-FAMILIES**. The exact remaining requirement is
resource-backed semantic preservation under the selected conversion contracts,
with explicit storage, failures and sufficient capacity. The earlier
[capacity row](../resource_representation/README.md) proves success and resource
custody; the [fixed-owned agreement row](../representation_agreement/README.md)
uses pure value maps. This row concerns the actual completed effectful call and
its current byte contents. It does not repeat the calls as mathematical terms.

**A completed buffered round trip preserves every semantic field and returns the caller's output storage.**

The [local witness](local/main.omg) retains the three-byte requirement,
arbitrary two-byte Pair, caller-owned mutable output and explicit failure.
Reading borrows the frame while its owner retains the mutable output resource.
Public encoder and decoder contracts state the completed byte/value relations.
The generic caller uses the selected round-trip promise directly. A separate
runtime path uses weak TryFrom views without taking the preservation law as a
premise. It compares both decoded fields. The path using the law also writes
through the returned view and checks its extent; the caller observes the write
and unchanged neighbors. Runtime checks use unequal 17/42 and endpoint 0/255
pairs, a short buffer, a malformed tag and nonzero neighboring bytes.

The [reject twin](local/invalid/main.omg) changes only the decoder's final byte
load from slot 2 to slot 1. Its capacity, success paths and resource handling
remain identical. It must fail semantic preservation, rather than formation or
an unrelated obligation.

The optimized baseline at `3cfc757f135a4c93fd3bdd4c356c2b55aba10a6d`
([run 41546898552](https://github.com/CathedralOS/Omega/actions/runs/41546898552))
refuses the valid source: it cannot form the successful payload after excluding
the only failure case, and it loses the saved public decoder call when selecting
its implementation. The corrupt twin fails for those same unrelated reasons, so
that refusal is not preservation evidence. No completed place is claimed. All nine places, their reject twins and corpus mirrors are authored; their
compiler and runtime observations remain pending. The full standard Representation interface, reflection adapters,
other hosts and the Omega-written producer remain parent work.

Each place supplies the same semantic pair to the conversion. The output
buffer remains a runtime borrow; a constant contributes a pair rather than
permission to access storage.

| Place | Semantic input | Reject twin |
| --- | --- | --- |
| [Local](local/main.omg) | Local request construction | [Wrong last-byte load](local/invalid/main.omg) |
| [Field](field/main.omg) | Field between nonzero neighbors | [Wrong last-byte load](field/invalid/main.omg) |
| [Parameter](parameter/main.omg) | Forwarded parameter | [Wrong last-byte load](parameter/invalid/main.omg) |
| [Call result](call_result/main.omg) | Completed call, both inputs | [Wrong last-byte load](call_result/invalid/main.omg) |
| [Match join](match_join/main.omg) | Distinct results from both arms | [Wrong last-byte load](match_join/invalid/main.omg) |
| [Constant name](const_name/main.omg) | Two owned constants | [Wrong last-byte load](const_name/invalid/main.omg) |
| [Nested machine](other_state/main.omg) | Explicit nested machine | [Wrong last-byte load](other_state/invalid/main.omg) |
| [Other machine](other_machine/main.omg) | Another named implementation | [Wrong last-byte load](other_machine/invalid/main.omg) |
| [Selected implementation](target_body/main.omg) | Build-selected body, checked marker | [Wrong last-byte load](target_body/invalid/main.omg) |

Governing contracts: [codecs](../../../../wiki/spec/layouts/codecs.md),
[completed returns](../../../../wiki/spec/language/state_contracts.md#returns-and-proof-use),
[proof identity](../../../../wiki/spec/proofs/contracts.md#data-identity-in-facts)
and [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md).

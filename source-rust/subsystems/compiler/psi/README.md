# Psi

[Psi](psi.rs) is the portable program that source stage 07 publishes and both execution engines read. Start with the fields of `struct Psi`. The instruction vocabulary, the entry and the machine bodies appear directly on that record. The other fields name the tables their instructions and declarations use.

| Field | Definition |
| --- | --- |
| `vocabulary_marker` | [Version of the program vocabulary](identities/program/identity.rs) |
| `identities` | [Interned names and declaration identities](identities/table.rs) |
| `entry` | [Optional callable selected as the program entry](../foundation/semantic-records/src/lib.rs#L30) |
| `machines` | [Executable machines](machines/machine.rs) contain [blocks](machines/block.rs), [operations](machines/operation.rs) and [terminators](machines/terminator.rs). |
| `types` | [Structural types, domains, qualifications and quotients](types/mod.rs) |
| `scalars` | [Scalar applications, folded constants and predicates](scalars/declarations.rs) |
| `interfaces` | [Services, program entry contracts, providers and callback declarations](interfaces/declarations.rs) |
| `access` | [Access views, lifecycle plans and reborrows](access/declarations.rs) |
| `execution` | [Callables, dynamic dispatch, suspension and task activation](execution/declarations.rs) |
| `proofs` | [Theorems, evidence, proof outputs and operation obligations](proofs/declarations.rs) |
| `layouts` | [Target layout applications and native layouts](layouts/declarations.rs) |

[Source publication](../source-to-psi/pipeline/07_checked-program-to-psi/src/source/publication.rs) produces a program after source checking. [Psi artifacts](artifact/) define its canonical encoding, publication metadata and receiving trust decision. The [shared semantics](semantics/) give operations their meaning. The [execution inputs](execution-inputs/) bind the program's declarations to a particular target and supplied resources.

## Changes to Psi

Implementers choose the representation, operations, encoding and reconstruction
needed for accepted Omega behavior. Human design review takes place when the
owner requests it. Carry a change through its versioned schema, producers,
consumers and the program that exercises it. Keep source behavior, trust
guarantees and the boundary between Psi and native compilation intact.

Compiler checking and rejection tests continue at the documented
[verification boundaries](../README.md#verification-placement). Profiling and
reducing compile costs also fall within implementation work. Record unresolved
language or trust decisions in [owner questions](../../../../OWNER_QUESTIONS.md).

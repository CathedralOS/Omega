# Build observations and reproducibility

This contract concerns admitted build execution, not runtime filesystem
permissions. [Execution](execution.md) owns admission and generated-source handoff;
[package acceptance](../packages/acceptance.md) owns project decisions.

## Observation classes

Every build-host operation has a normalized observation ceiling:

| Class | Meaning |
| --- | --- |
| `Hermetic` | No external build-host state is observed. |
| `Receipted` | Every possible observation supplies enough value/content evidence for replay. |
| `Volatile` | Some possible observation lacks complete replay evidence. |

The ordering is `Hermetic < Receipted < Volatile`. This is an operation-contract
axis, not a service-name classification or a new build keyword. A selected
implementation must refine its requirement's ceiling. Release-capable clock,
randomness, environment, enumeration, and similar operations require replay
evidence. Volatile providers require explicit development policy; they are not
ambient defaults or eligible inputs to a source-rebuildable release.

For the concrete target/configuration, the static ceiling joins all reachable
operation classes. Proven unreachable branches contribute nothing; unresolved
branches contribute conservatively. Policy may reject the ceiling before
execution. The realized class joins operations actually reached and may be
narrower. Observation records are evidence, not a third classification.
Discarding a result does not erase an effectful observation.

## Two reproducibility claims

`ReplayableFromRecord` requires a realized class no greater than `Receipted` and
availability of every recorded input. `RebuildableFromSource` additionally
requires every input receipt to trace to a declared reproducible root, every
dependency artifact to be source-rebuildable, and pinned toolchain and
target-provider inputs.

A content-addressed dependency artifact can support replay even when its own
production used unrecorded inputs. Recording a volatile value does not establish
its source provenance. Report the static ceiling, realized class, receipt/input
identities, both verdicts, and the first failing provenance edge. Development,
record-replay release, and transitive source-rebuildable release policies test
these distinct graph properties; they do not rewrite local operation classes.

## Replay evidence

Every exercised filesystem attempt receives its own classification. It is
`Receipted` only when exact provider-free replay establishes the complete
observation; otherwise it is `Volatile`. An exercised filesystem attempt is not
`Hermetic`. Bind these classes into observation identity rather than hiding an
unsupported attempt behind an aggregate verdict. Unsupported replay prevents
the affected candidate's claim, not unrelated builds.

The filesystem replay disposition is a closed sum:

| Disposition | Established claim |
| --- | --- |
| `NotReplayed` | No replay claim. |
| `SourceInputsOnly` | Retained source-input operations, results, and observations agree under provider-free replay. |
| `Complete` | Exact attempts, results, handoffs, output namespace, teardown, and sponsored output custody agree. |

Partial replay is not complete build replay. Complete replay binds canonical
source custody even when no Source operation ran. Observation identity binds the
schema, disposition, ordered attempts, handoffs, and complete output commitment.
Whole-build replay also compares build results and BuildLog independently of the
filesystem record. The implementation's supported operation sequences and
private capacities live [beside replay code](../../../omega-rust/omega/build/build-evaluation/replay.md).

## Complete output custody

After evaluator/provider teardown, inspect the quiescent sponsored Output tree
before deleting the session. Retain its complete canonical tree: sorted portable
UTF-8 relative paths, entry kinds, empty directories, ordinary/executable mode,
file lengths/content commitments, and self-contained relative symlink targets.
An empty tree has an explicit commitment.

Exclude host roots, timestamps, ownership, ACLs, ambient permissions, and inode or
hard-link topology from canonical tree identity. Retain hard-link operation
relationships separately for replay. Reconcile namespace kinds, object groups,
extents, and sponsored usage; unknown objects, external links, inconsistent
custody, and exceeded provisions reject. Bind topology-independent unique-content
accounting alongside the tree commitment.

Materialization consumes retained complete content into an existing empty
destination and re-inspects exact paths, kinds, modes, targets, and bytes.
Generated source is only the explicitly selected handoff subset, never whatever
happens to have a source filename. Each handoff binds its completed attempt
ordinal; ordinals are nondecreasing, a path occurs once, and the corresponding
file is closed first. Equal ordinals are legal and handoff order need not match
output creation order. Source extension, reserved-name, regular/non-executable
file, and final frontend checks still apply.

## Limits of the claim

Replay is not host containment, authenticity, package approval, or evidence of
an audit. Scoped grants and deterministic resource accounts do not exclude a
hostile same-user process racing the session. A partial trace does not replace
the complete immutable compiler-request snapshot. Resource refusals and
unavailable evidence must remain explicit; neither becomes an empty permission
set or a fabricated successful receipt.

# Accepted project lock

`PackageLock` is inert retained project state. It joins immutable source pins,
complete compiler-normalized policy, and historical project decisions without
retaining an old checkout, compiler session, proof certificate, evaluator
receipt, or native replay. It does not write files, resolve selectors, acquire
sources, certify an audit, or authorize a changed candidate.

Each `PackageLockTarget` contains a canonical source subject, exactly one
`PackagePolicyBaseline` per source package in source order, and historical
decisions against that source subject's fingerprint. Baseline package identities
and targets must match exactly. Sections are strictly sorted by the target's
canonical semantic identity. Every section has the same root request and role,
package identities and immutable resolutions, navigations, authored dependency
projections, and selected dependency edges. Only target-sensitive baseline and
decision state may differ. The format lists explicitly supplied targets; it
does not discover a deployment set or establish a support matrix.

Every concrete package owner retained anywhere in the complete policy must
belong to that target's exact transitive source graph. Evidence owns this
enumeration, including compiler canonical type and callable identities; the
manager supplies package-key membership, not a second semantic parser. A
transitively carried type does not need a direct dependency edge. Toolchain
owners, absent optional owners, and symbolic binders do not assert package
membership. Foreign symbolic boundary demands additionally join the owning
baseline's exact boundary operator and Type-only telescope. These are inert
cross-record consistency checks, not source replay, public availability checks
for arbitrary carried declarations, or fresh audit certification.

## Version 1 text

The outer grammar is line-oriented ASCII. Counts and byte lengths are unsigned
decimal with no signs or leading zeroes. Target identities come from the trusted
toolchain catalog. All rows end in LF, and there is no trailing material.

```text
omega_lock 1
targets <count>
target <canonical target identity>
source <byte length>
<verbatim canonical source text>
baselines <source package count>
baseline <byte length>
<verbatim complete named policy text>
decisions <byte length>
<verbatim historical decision text>
end_target
end
```

Repeat `baseline` sections in source-package order and target sections in
canonical target order. Child text already includes its final LF; the envelope
does not insert another separator. Byte lengths delimit children without
escaping an entire source graph or policy into an opaque string. Each child
owner validates its own canonical format and typed meaning. The manager checks
the joins and outer framing. Unknown versions fail with recovery guidance;
loading never upgrades pins or treats unknown policy as empty.

## Decision history

New complete-policy choices use this versioned child section:

```text
omega-policy-decisions 2
source <candidate source-subject fingerprint>
baseline <prior source-subject fingerprint or none>
comparison <complete comparison fingerprint>
decisions <count>
decision root-role <accept or reject>
decision source-replacement <change fingerprint> <accept or reject>
decision row <change fingerprint> <accept or reject>
end
```

Only present choices are emitted. Subjects are strictly ordered: root role,
source replacements by fingerprint, then policy rows by fingerprint. The
comparison binds each choice to the reviewed change, including removed packages
and replaced roots. These subjects do not refer to candidate-package indices.
An initial or unchanged comparison may have zero required choices.

`HistoricalPackagePolicyDecisions::capture_policy` checks the completed
resolution's comparison and candidate source association, then records it
directly. Both acceptance and rejection survive serialization. This is not a
file publication operation. Reading history checks framing, order, duplicate
subjects, resource limits, and the retained candidate source association; it
does not recreate old comparisons or certify that recorded decisions were made
seriously. The project trusts whoever lands the lock. The previous source hash
is context, not a requirement to retain that checkout or a second full graph.

Version 1 decision sections remain readable and re-encode as version 1. Their
legacy package indices refer only to their associated source graph. No missing
full-comparison identity is inferred during loading, and unknown child versions
retain the existing unsupported-format recovery guidance.

## Resource accounting

`PackageLockRecoveryLimits` can lower, but not raise, these aggregate ceilings:

| Resource | Maximum |
| --- | ---: |
| Input text | 128 MiB |
| Requested recovery-owned storage | 256 MiB |
| Target sections | 32 |
| Source package rows, across target sections | 16,384 |
| Authored plus selected dependency-request rows | 262,144 |
| Policy sequence and recursive entries | 1,048,576 |
| Semantic identity traversal nodes | 1,048,576 |
| Historical decisions | 65,536 |

Child format ceilings also apply: source text is at most 64 MiB, each complete
policy text at most 32 MiB with its existing 4 MiB binary and semantic limits,
and each decision section at most 8 MiB. Source identity/request fields have
the source owner's 1 MiB hard ceiling. Counts do not reset at target boundaries.
Version 2 historical decisions require only the retained decision vector;
strict subject order makes duplicate-validation scratch unnecessary.

The owned-storage allowance counts requested vector, string, and box storage,
typed constructor allowances, retained source binary, and validation scratch.
Input text and previously owned values remain borrowed and are not charged;
allocator overhead is excluded. Each child reports its consumed allowance, which
the lock subtracts before recovering the next child. Outer target and baseline
vectors are charged before reservation.
The sorted package-owner index and any exact unescape buffers also consume
the same owned-storage allowance. Semantic identity work is cumulative across
baselines and targets, with an additional per-identity nesting ceiling of 128.

`canonical_text_with_limits` uses the same child recovery accounting so it does
not emit a record that exceeds the chosen recovery ceilings. It drops each
reconstructed child before checking the next; it does not retain a second whole
lock or compare compiler results. `canonical_text` selects the default ceilings.
This resource check does not turn a historical rejection into acceptance or
replace fresh review. Locked source acquisition and atomic install/update
publication belong to the separate operation owners.

`operations::recover_locked_sources` consumes this record only as exact source
expectations. It selects an explicitly recorded target before touching resolver
storage, reacquires and verifies sources under current limits, and requires the
entire freshly projected graph to match. Offline Git cache use never resolves a
mutable selector; allowed fetching requests the recorded commit only. Local
recovery still requires the live source for recapture. Failure leaves this
borrowed baseline readable, and success does not turn historical decisions into
fresh compiler findings or transaction authorization.

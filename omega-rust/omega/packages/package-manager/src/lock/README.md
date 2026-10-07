# Accepted project lock

The [lock specification](../../../../../../wiki/spec/packages/locks.md) owns
user-visible meaning; this note owns the current codec and recovery API.

`PackageLock` is inert retained project state. It joins immutable source pins,
compact compiler-normalized acceptance, exact accepted compilation inputs, and
historical project decisions without retaining an old checkout, compiler
session, proof certificate, evaluator
receipt, or native replay. It does not write files, resolve selectors, acquire
sources, certify an audit, or authorize a changed candidate.

Each `PackageLockTarget` contains a canonical source subject, one independent
`PackagePolicyOccurrence` per source-package/purpose pair in canonical source and
purpose order, and historical decisions against that source subject's fingerprint.
Each occurrence carries a `PackageCheckedContext` with purpose, checked target,
and explicit optional build execution profile. Product targets equal the section
target; Build targets equal its required execution profile. All occurrences in
one section agree on the execution profile. The derived source roster must match
these records bijectively; a single acceptance cannot cover two purposes.
Each occurrence also requires `AcceptedPackageCompilation`: a nonzero SHA-256
compiler input identity and the exact accepted semantic binding roster. Bindings
retain role, package owner, declaration path, normalized service-schema digest,
optional selected-provider-plan digest, and explicit terminal permissions.
`from_accepted_policies` checks full policy source membership and boundary owners
before retaining these inputs; `from_policies` requires an explicit compiler
identity and supplies an empty binding roster. Neither constructor guesses
permissions or a current compiler identity. Sections are strictly sorted by
the target's canonical semantic identity. Every section has the same root request and role,
package identities and immutable resolutions, navigations, authored dependency
projections, and selected dependency edges. Occurrence acceptance, compilation
inputs, and decision state remain section-specific. The format lists explicitly supplied targets; it
does not discover a deployment set or establish a support matrix.

For mutable local packages, root `omega.lock` is excluded
from source capture:
writing acceptance state cannot change the source pin stored inside that state.
Nested lock-named source files and exact repository materializations still
participate in their source identities. The manager reads the accepted lock
separately, so edits to retained policy and bindings remain visible even when
the source identity is unchanged.

Each `PackageAcceptanceRow` retains the complete canonical readable meaning of
one admission-claim or external-realization callable, external supply, dangerous
capability, terminal permission, or restricted build request. The coordinate key
identifies a row; exact
meaning comparison, not digest-only consent, identifies a policy change.
The manager owns these inert retained records separately from the compiler's
fresh `PackagePolicyBaseline`. Recovery does not fabricate compiler evidence.

The lock excludes full public API, ordinary checked callables, selected-provider
tables, representation snapshots, source-semantic dependency graphs, and symbolic
demands. Explicit review, acceptance, and update workflows produce those complete
findings and compare the fresh risk projection without an old-source analysis
pass. The small accepted binding roster is a compiler input, not one of these
review snapshots.

Under the owner ruling of 2026-10-03, ordinary compilation compares every accepted
occurrence's source and compiler identities, including the root, before using
its retained bindings. A match skips package-review discovery/bound passes. A
mismatch rejects with the package name and `omega update`; compilation does not
perform an inline audit or update acceptance. The compiler input digest joins
the compiler's embedded build-input identity with the selected toolchain library
content. It invalidates stale acceptance; it is not compiler, host, or reviewer
attestation. See [ordinary compilation](../../../../../../wiki/spec/packages/acceptance.md#ordinary-compilation).

## Version 4 text

The outer grammar is line-oriented ASCII. Counts and byte lengths are unsigned
decimal with no signs or leading zeroes. Target identities come from the trusted
toolchain catalog. All rows end in LF, and there is no trailing material.

```text
omega_lock 4
targets <count>
target <canonical target identity>
source <byte length>
<verbatim canonical source text>
occurrences <checked occurrence count>
occurrence <source package index> <product|build> <execution profile|none> <checked target>
acceptance <byte length>
<verbatim canonical acceptance text>
compilation <byte length>
<verbatim accepted compilation text>
decisions <byte length>
<verbatim historical decision text>
end_target
end
```

Repeat occurrence/acceptance/compilation groups in source-package then purpose
order and target sections in canonical target order. Child text already includes
its final LF; the envelope does not insert another separator. Byte lengths delimit children without
escaping an entire source graph or policy into an opaque string. Each child
owner validates its own format. The manager checks the joins and outer framing.
Acceptance children use:

```text
acceptance_schema 3
rows <count>
row <callable|external_supply|dangerous_capability|terminal_permission|restricted_build_request>
key <SHA-256 of canonical row coordinate bytes>
meaning <byte length>
<complete canonical readable risk-row text>
end_acceptance
```

Repeat row/key/meaning groups in strict kind/key order. The meaning includes its
final LF. Keys are lowercase hexadecimal, and duplicate coordinates reject.
The occurrence's source-package index supplies package identity; its explicit
checked context supplies target identity. Historical text is not promoted into
fresh typed analysis.

Every occurrence requires this compilation child:

```text
compiler <64 lowercase hexadecimal digits>
package <occurrence package identity digest>
purpose <product|build>
target <canonical checked target identity>
execution <canonical execution profile|none>
bindings <count>
role <semantic binding role tag>
owner <binding package identity digest>
declaration <byte length>
<declaration path bytes>schema <service schema digest>
plan <selected provider plan digest|none>
permissions <count>
schema <permission service schema digest>
requirement <byte length>
<requirement identity bytes>classes <count>
class <terminal authority class tag>
end_compilation
```

Repeat binding groups in the retained roster order, permission groups for each
binding, and class rows for each permission. Length-framed declaration and
requirement strings have no inserted LF; the next field starts immediately
after their counted bytes. Permission requirements and authority classes must
be strictly ordered without duplicates. Tags use the compilation-input and
terminal-authority owners' closed wire vocabularies, not unchecked Rust enum
ordinals. Digests are 32 bytes encoded as 64 lowercase hexadecimal digits;
the compiler identity cannot be zero. The repeated package/purpose/target/profile
must match the containing occurrence, and binding owners must belong to the
source graph. Role/plan combinations and permission/schema associations are
validated by the existing typed binding constructors. No permission is derived
from a role or readable name.

Ordinary compilation accepts only outer version 4. Explicit review/update can
recover version 3 as a historical baseline, preserving its source pins, target
sections and decisions while freshly reviewing the candidate. Missing compiler
identity and semantic bindings stay absent: a historical baseline cannot be
serialized as version 4 or used for ordinary compilation. Publication requires
freshly reviewed compilation inputs for every retained target. Other old versions
reject; recovery never invents missing consent or compilation inputs. The
compact acceptance child schema is version 5, matching the policy row schema.
Explicit review also recovers the preceding schema 4's opaque historical rows,
pins and decisions without inventing authored target context. Ordinary lock
recovery rejects that child schema; it supplies no compilation inputs or current
terminal permissions. Fresh checking and decisions replace its consent with
schema 5 before publication. Unknown schemas still reject.

## Decision history

Historical comparison choices use this versioned child section:

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
| Policy entries, semantic bindings, permissions, and authority classes | 1,048,576 |
| Semantic identity traversal nodes | 1,048,576 |
| Historical decisions | 65,536 |

Child format ceilings also apply: source text is at most 64 MiB, each acceptance
or compilation text at most 32 MiB,
and each decision section at most 8 MiB. Source identity/request fields have
the source owner's 1 MiB hard ceiling. Counts do not reset at target boundaries.
Version 2 historical decisions require only the retained decision vector;
strict subject order makes duplicate-validation scratch unnecessary.

The owned-storage allowance counts requested vector, string, and box storage,
typed constructor allowances, retained source binary, and validation scratch.
Input text and previously owned values remain borrowed and are not charged;
allocator overhead is excluded. Each child reports its consumed allowance, which
the lock subtracts before recovering the next child. Outer target and occurrence
vectors are charged before reservation.
Compilation decoding also charges its entire text length before copying strings,
and charges binding, permission, and authority-class vectors before reservation.
These counts and storage charges share the enclosing lock's remaining budget.
Occurrence vectors contain at most two entries per source package. Full-policy
construction additionally bounds source-owner identity traversal; compact
recovery does not manufacture full policy.

`canonical_text_with_limits` uses the same child recovery accounting so it does
not emit a record that exceeds the chosen recovery ceilings. It drops each
reconstructed child before checking the next; it does not retain a second whole
lock or compare compiler results. `canonical_text` selects the default ceilings.
This resource check does not turn a historical rejection into acceptance or
replace fresh review. Locked source acquisition and atomic install/update
publication belong to the separate operation owners.

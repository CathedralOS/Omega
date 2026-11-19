# Accepted project lock

The [lock specification](../../../../../../wiki/spec/packages/locks.md) says
what the lock means to users; this note covers the current text format and
the API that reads it.

`PackageLock` is inert stored project state recording immutable source pins,
compact compiler-normalized acceptance, the exact accepted compilation inputs
and past project decisions, not an old checkout, compiler session,
proof certificate, evaluator receipt or native replay. It writes no files,
resolves no selectors, fetches no sources, certifies no audit and approves no
changed candidate.

Each `PackageLockTarget` holds a canonical source subject, one
`PackagePolicyOccurrence` per source package and purpose (in canonical source
then purpose order), and past decisions against that source subject's
fingerprint. Each occurrence carries a `PackageCheckedContext`: purpose, checked
target and explicit optional build execution profile. Product targets equal the
section target; Build targets equal its required execution profile. All
occurrences in a section share one execution profile. The derived source roster
must match these records one to one; one acceptance cannot cover two purposes.

Each occurrence also needs an `AcceptedPackageCompilation`: a nonzero SHA-256
compiler input identity and the exact list of accepted semantic bindings. A
binding records its role, owning package, declaration path, normalized
service-schema digest, optional selected-provider-plan digest and explicit
terminal permissions. `from_accepted_policies` takes an explicit compiler
identity and checks full policy source membership and boundary owners before
storing these inputs. It never guesses permissions or a compiler identity. Sections are sorted strictly by the target's canonical semantic
identity. All sections share the root request and role, package identities and
immutable resolutions, navigations, authored dependency projections and
selected dependency edges; only occurrence acceptance, compilation inputs and
decision state differ. The format lists the targets it was given; it neither
discovers a deployment set nor defines a support matrix.

For mutable local packages, source capture skips the root `omega.lock`, so
writing acceptance state cannot change the source pin stored inside it. Nested
lock-named source files and exact repository materializations still count in
source identities. The manager reads the accepted lock separately,
so edits to stored policy and bindings show even when the source identity is
unchanged.

Each `PackageAcceptanceRow` keeps the full canonical readable meaning of one
admission-claim or external-realization callable, external supply, dangerous
capability, terminal permission, restricted build request or independent
component assumption. A `component_assumption` row names the component package,
its assumption digest and the description rows it covers; root Product
admission rejects any fresh row of that kind the lock does not hold. The
coordinate key identifies a row; comparing the exact meaning, not just a
digest, detects a policy change. These stored records stay separate from
the compiler's fresh `PackagePolicyBaseline`. Reading a lock never fabricates
compiler evidence.

The lock does not store the full public API, ordinary checked callables,
selected-provider tables, representation snapshots, source-level dependency
graphs or symbolic demands. Explicit review, acceptance and update
workflows produce those findings and compare the fresh risk rows without
re-analyzing the old source. The small binding list is a compiler input, not a
review snapshot.

Under the owner ruling of 2026-10-03, ordinary compilation compares every
accepted occurrence's source and compiler identities, root included, before
using its stored bindings. On a match it skips the package-review
discovery and bound passes. On a mismatch it fails with the package name and
`omega packages update`; it never audits or updates acceptance itself. The
compiler input digest combines the compiler's embedded build-input identity
with the selected toolchain library's content. It detects stale acceptance; it
does not vouch for the compiler, host or reviewer. See [ordinary compilation](../../../../../../wiki/spec/packages/acceptance.md#ordinary-compilation).

## Version 4 text

The outer format is line-based ASCII. Counts and byte lengths are unsigned
decimals without sign or leading zeroes. Target identities come from the
trusted toolchain catalog. Every row ends in LF; nothing follows the last.

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

Occurrence, acceptance and compilation groups repeat in source-package then
purpose order; target sections repeat in canonical target order. Each child
text already ends in LF; the envelope adds no separator. Byte lengths delimit
children, so no source graph or policy is escaped into an opaque string. Each
child's owner validates its format; the manager checks the outer framing and
the joins between children.
Acceptance children use:

```text
acceptance_schema 9
rows <count>
row <callable|terminal_permission|external_supply|dangerous_capability|restricted_build_request|assumed_binding_contract|service_audit|native_binding|component_assumption>
key <SHA-256 of canonical row coordinate bytes>
meaning <byte length>
<complete canonical readable risk-row text>
end_acceptance
```

Row, key and meaning groups repeat in strict kind then key order; each meaning
ends in LF. Keys are lowercase hexadecimal, and a duplicate coordinate
is rejected. The occurrence's source-package index gives the package identity;
its explicit checked context gives the target identity. Past text never
becomes fresh typed analysis.

Each occurrence needs this compilation child:

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

Binding groups repeat in stored roster order, permission groups per binding,
and class rows per permission. Length-framed
declaration and requirement strings have no LF after them; the next field
starts right after their counted bytes. Permission requirements and authority
classes must be strictly ordered with no duplicates. Tags use the closed wire
vocabularies of the compilation-input and terminal-authority owners, not
unchecked Rust enum ordinals. Digests are 32 bytes written as 64 lowercase
hexadecimal digits; the compiler identity is nonzero. The repeated
package, purpose, target and profile must match the enclosing occurrence, and
binding owners must belong to the source graph. The typed binding constructors
validate role/plan combinations and permission/schema pairs. No permission is
derived from a role or readable name.

Ordinary compilation accepts only outer version 4. Explicit review or update
can read version 3 as a historical baseline, keeping its source pins, target
sections and decisions while reviewing the candidate afresh. Its compiler
identity and semantic bindings stay absent, so it cannot be written as version
4 or used for ordinary compilation. Publishing needs
freshly reviewed compilation inputs for every stored target. Other old
versions reject; reading never invents missing consent or compilation
inputs. The acceptance child schema is version 9, matching the policy row
schema (`PACKAGE_POLICY_ROW_VERSION`). Schemas 4 and 5 are historical:
explicit review also reads their opaque rows, pins and decisions without
inventing authored target context. Ordinary lock reading rejects them, since
they lack compilation inputs and current terminal permissions. Fresh checking
and decisions replace their consent with schema 9 before publication. Unknown
schemas reject.

## Decision history

Past comparison choices use this child section:

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

Only choices made are written. Subjects are strictly ordered: root
role, then source replacements by fingerprint, then policy rows by
fingerprint. The comparison ties each choice to the reviewed change, including
removed packages and replaced roots. Subjects never use candidate-package indices. A
first or unchanged comparison may need no choices.

`HistoricalPackagePolicyDecisions::capture_policy` checks the finished
resolution's comparison and its link to the candidate source, then records it.
Acceptances and rejections are both written; it does not publish a file.
Reading history checks framing, order, duplicate subjects, resource limits and
the stored link to the candidate source. It neither recreates old comparisons
nor certifies that decisions were made carefully: the project trusts whoever
lands the lock. The previous source hash is context, not a
requirement to keep that checkout or a second full graph.

Version 1 decision sections stay readable and are written back as version 1. Their old package indices refer only to their own source graph. Loading
never infers a missing full-comparison identity, and unknown child versions
keep the existing unsupported-format recovery guidance.

## Resource accounting

`PackageLockRecoveryLimits` can lower, but not raise, these total limits:

| Resource | Maximum |
| --- | ---: |
| Input text | 128 MiB |
| Requested recovery-owned storage | 256 MiB |
| Target sections | 32 |
| Source package rows, across target sections | 16,384 |
| Authored plus selected dependency-request rows | 262,144 |
| Policy entries, semantic bindings, permissions, and authority classes | 1,048,576 |
| Semantic identity traversal nodes | 268,435,456 |
| Historical decisions | 65,536 |

The identity-node limit follows the input-text limit. Each node uses lock
text; a nominal name opens at most two scopes per byte (one for its atom,
one for its owner), so 128 MiB of text gives at most 268,435,456 nodes. A
fixed 1,048,576-node limit refused a 17-package application.

Child formats have their own limits: source text at most 64 MiB, each acceptance
or compilation text at most 32 MiB, and each decision section at most 8 MiB.
Source identity and request fields have the source owner's 1 MiB hard limit.
Counts do not reset between target sections. Version 2 decisions need only
the stored decision vector; strict subject order removes the need for
duplicate-checking scratch.

The owned-storage allowance counts requested vector, string and box storage,
typed constructor allowances, the stored source binary and validation scratch.
Borrowed input text and already-owned values are not charged, nor is allocator
overhead. Each child reports the allowance it used, which the lock subtracts
before reading the next child. Outer target and occurrence
vectors are charged before they are reserved. Compilation decoding also charges
its whole text length before copying strings, and charges binding, permission
and authority-class vectors before reserving them. These counts and charges share
the enclosing lock's remaining budget. Occurrence vectors hold at most
two entries per source package. Full-policy construction also bounds
source-owner identity traversal; compact reading builds no full policy.

`canonical_text_with_limits` uses the same child accounting, so it never
writes a record over the chosen limits. It drops each rebuilt child
before checking the next, keeping no second whole lock and comparing no
compiler results. `canonical_text` uses the default limits. This check neither
turns a past rejection into acceptance nor replaces fresh review. Locked
source fetching and atomic install or update publication are separate
operations.

# Exact-rule promotion evidence

These records support the checked [rule inventory](../rules.md), not a release diary.

One file promotes one exact optimizer rule. Copy the schema below to
`<ExactRuleName>.md`; do not create a suite-wide or optimization-level record.
Every field must point to reviewed, reproducible evidence and must not remain
`PENDING` when the release inventory status changes. A record may be staged
while its row stays `Experimental`: carry the full schema with `PENDING`
values, keep the `Exact rule` and `Rollback` lines exact, and leave `Approved
status` incomplete — the architecture gate rejects a completed approval that
the inventory does not yet reflect.

Cite evidence as backticked repository citations: `` `path` `` names a file or
directory under the repository root, and `` `path::subject` `` requires the
file's text to name `subject` (a test function, section, or record). The
architecture gate resolves every citation in the record, and each completed
evidence field must carry at least one — unverifiable prose does not count as
evidence. Bare identifiers (`Optimization::ALL`, target names, flags) stay
prose and are not resolved.

```text
# <ExactRuleName> Promotion

- Exact rule: <ExactRuleName>
- Approved status: Recommended | Default
- Owner approval: <owner, review, date>
- Semantic and corruption evidence: <tests/results>
- Differential evidence: <corpus/results>
- Determinism and bounded-work evidence: <tests/results>
- Target matrix evidence: <targets/results>
- Measurement evidence: <versioned benchmark/results>
- Rollback: --disable-optimization <ExactRuleName>
```

Promotion never creates or authorizes a broad `O1`/`O2`/`O3`, debug, or
release bundle. The source-visible exact name and rollback mechanism remain
available after promotion.

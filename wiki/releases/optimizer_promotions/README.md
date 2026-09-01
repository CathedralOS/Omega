# Optimizer Promotion Records

One file promotes one exact optimizer rule. Copy the schema below to
`<ExactRuleName>.md`; do not create a suite-wide or optimization-level record.
Every field must point to reviewed, reproducible evidence and must not remain
`PENDING` when the release inventory status changes.

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

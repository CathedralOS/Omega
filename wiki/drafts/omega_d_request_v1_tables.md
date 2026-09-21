# OMEGA-D-REQUEST-V1-TABLES — re-verification ledger

Re-verified on Linux x86-64 at `bbc42c098b54` (board tip at claim time) by
Zergling-126 under claim ticket `6fbd3ba2` (fence: `bootstrap/5_omega/outcome.epsilon`,
`tests/bootstrap/omega-outcome`). The item re-mines the OMEGA-D EREQ v1
request clause's outcome-table surface.

## New evidence: the gate is green natively on Linux x86-64

The row's annotation records "it requires a macOS arm64 or Windows x64 seed,
so Linux x86-64 evidence is the recorded run". That is stale — the alpha seed
env (`tools/bootstrap/alpha/seed_env.sh::require_seed_execution_host`) now
lists Linux x86-64 as a seed-executing host, and the full gate ran to a
native green here:

```text
$ sh tests/bootstrap/omega-outcome/run.sh
Complete D customer: 581426 bytes, SHA-256 c0fc0f0abc69130b8e7549db419c0fda7be2fbbea92ef5ffc52a00adf64a0435
Epsilon receipt reconstruction: status 0, 266.064s
Omega outcome customer: status 0, 3442.940s
PASS: OCOUT tables, frame encodings, refusals, bounded arithmetic, and
recorded outcome tuples match the request contract
```

(The bound materializers refuse before writing when the canonical entry,
manifest, members, packed closure, or composed record differ from the
audited edge records — so identity checks also passed.)

## What this gate exercises

Per `tests/bootstrap/omega-outcome/README.md`: every scalar-resource code's
limit + coordinate space, the parser-resource projection onto wire codes
3..11, the lexical-diagnostic projection onto Reject codes 2..9, every
assigned Reject/InternalFailure code-to-space pair, exact 40/48-byte
canonical-source OCOUT frames, refusal of unassigned/noncanonical tuples,
the bounded publication sum, phase-1 declared-extent provision, and the
recorded scalar outcome tuples.

## Remaining clause legs (out of fence)

Per the sibling OMEGA-D-REQUEST-OUTCOME-TABLES row, the residual legs are
producer wiring rather than table work: the syntax Reject inventory
(codes 13-89), checking codes 90-97, and Incomplete coverage provisions
(15-24) are assigned in the contract but producers still record the
unassigned marker; the semantic phases over the decoded request fields
remain unimplemented. Those producers are live-fenced: `request_and_utf8.epsilon`
under OMEGA-D-REQUEST-ADMISSION/semantics-4-5 (Zergling-128, exp
2026-09-22T03:12Z), `scalar_compilation.epsilon` under OMEGA-D (z178, exp
2026-09-21T11:17Z).

## Verdict

The outcome-table slice inside this fence is landed and now carries a
green native Linux x86-64 gate run (previously recorded as needing a
macOS/Windows seed). No unclaimed slice remains inside the claimed paths;
producer-side legs sit under sibling claims.

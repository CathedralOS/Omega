# Omega compiler observatory

A dependency-free dashboard over existing repository evidence. Agents keep using
the sample gate, native corpus recorder and outcome claims; there is no dashboard
status file to maintain. The collector never launches the compiler or modifies
the evidence it reads. Python 3.9+ and Git are sufficient.

## Local dashboard

From any Omega checkout:

```text
python tools/compiler_dashboard.py --serve
```

Open `http://127.0.0.1:8765`. **Refresh evidence** re-reads that checkout and makes
one bounded read of the shared claims registry. The server binds only loopback.
An unavailable registry is unknown, never zero claims. An expired lease stays
unresolved; it does not establish that the owner stopped.

Use `--repository PATH` to inspect another checkout. By default, ignored
`build/sample-gate-runs/*.json` records are read from the main checkout beside
the common Git directory, matching the sample gate's shared record location
across worktrees. `--evidence-root PATH` overrides that location for imported
evidence. Records on another machine must be shared before they can appear.
Preserve their original revision, producer and target; never restamp them to
match a newer checkout. Native goldens and claims already have shared homes.

## Portable and hosted snapshot

```text
python tools/compiler_dashboard.py --output build/compiler-dashboard/index.html
```

The output is one self-contained HTML file: no dependencies, remote fonts, API
keys or external data requests. Open it directly or serve it on static hosting.
Its button explains snapshot age; it does not imply a connection to the repo.
The source revision, tracked-tree dirty state, collection time, row revision
stamps, native golden update dates and registry revision remain in Evidence details.

The owner-private hosted snapshot is registered in `hosting.json`. To update it:

1. Export into the existing Sites source checkout's `dist/index.html` with
   `--output /path/to/site/dist/index.html`. Use the intended evidence checkout.
2. Use the Sites workflow with the **same project ID**: obtain a fresh source
   credential, push/package that exact source, save a version and deploy it
   privately. Keep credentials out of files and preserve the current audience.
3. Confirm the native deployment receipt. A new local export does not update the
   hosted page until publication succeeds. No scheduled publication is enabled.

## Meaning of the views

- **Overview:** one chart at a time, selected with Same set, Growth or History.
  Same set connects each host's old and latest rates on the frozen denominator;
  full-corpus totals remain in Evidence details. Facts and Work open separate panels. Evidence
  details holds provenance, native receipts, warnings and comparison methods.
- **Fixed cohort:** the pinned September 30 baseline for each host keeps its
  original fixture-ID denominator. Compare its recorded runs then and now
  against the current full corpus. Missing old IDs stay in the denominator;
  retained losses and removed previously running IDs are reported separately.
  Growth splits recovered old IDs from new IDs running now. New does not mean
  first-recording success, and source changes mean recovery is not a causal
  measurement of compiler improvement. An unavailable baseline stays unknown.
- **Native history chart:** the last recorded golden per UTC day across the
  30 days before the inspected HEAD, at its real commit timestamp. The blue area counts valid programs
  with recorded exits. All available hosts share count and date axes by default;
  the target selector can isolate a host. Cohorts may grow or shrink, so this
  is not a fixed-cohort success rate. Point selection
  exposes the exact count, timestamp and golden revision. No daily points are
  invented between records.
- **Fact observation chart:** categorical bars aggregate the grid's selected
  target into recorded pass, historical/qualified pass, recorded failure and
  unmeasured places. Selecting a bar exposes its count and denominator; this
  does not convert historical observations into current acceptance.
- **Fact grid:** exact standard place paths in `samples/cli/facts/<fact>/`;
  positives require `main.omg`, rejects require `invalid/main.omg` or an existing
  `controls/<name>/main.omg`. The source count is structural coverage only.
  Observations are the latest record per exact fact/place/target/control, not
  substring matches or one aggregate run. Different revisions, dirty runs,
  stale CLIs, missing compiler profiles and unmeasured rejects remain qualified.
  No automatic claim is made that retained evidence applies to current source.
- **Native results:** reuse `corpus_records` and `corpus_scoreboard` parsing and
  exit-oracle matching, including target-specific exit oracles. Denominators
  contain valid `pass/` and `run/` rows actually in that host's golden; hosts
  can have different cohorts. Exit matches do not establish stdout correctness
  or Terminal/native agreement. Per-row revision stamps remain available.
- **Active claims:** `python tools/claims.py status`, using its existing bounded
  acquisition path. Notes are owner-reported, not independent acceptance.

The dashboard does not audit fixture pins, rejection diagnostic relevance,
producer digest applicability, runtime falsifiability or task acceptance. Those
remain governed by [testing](../testing.md) and the fact READMEs. Refreshing the
dashboard does not close a fact or a capability.

## Checks and design

```text
python -m unittest discover -s tools/tests -p test_compiler_dashboard.py
node tools/tests/test_dashboard_charts.cjs
```

The renderer checks use a Node-compatible JavaScript runtime without packages;
the dashboard itself still needs only Python and Git.

Visual direction: the user's Claude dashboard screenshot and pasted artifact,
supplied 2026-10-29, plus [Mebius Charts](https://www.mebius.design/freebies/charts),
inspected the same day. A warm charcoal workspace uses one muted accent for the
fixed-cohort comparison, plain navigation and secondary evidence views.
The implementation uses original HTML/CSS/JS; no Mebius assets or gated source
were copied. Native exit-oracle results are under **Native result receipts**.

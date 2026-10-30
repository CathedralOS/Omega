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

- **Overview:** history, fixed-cohort comparison and growth sources are all
  visible. History leads; the two comparisons share a row on wide screens and
  stack on narrow screens. The cohort chart shows baseline, same-set-now and
  full-corpus rates together. Facts and Work open separate panels. Evidence
  details holds provenance, native receipts, warnings and comparison methods.
- **Fixed cohort:** the pinned September 30 baseline for each host keeps its
  original fixture-ID denominator. Compare its recorded runs then and now
  against the current full corpus. Missing old IDs stay in the denominator;
  retained losses and removed previously running IDs are reported separately.
  Growth splits recovered old IDs from new IDs running now. New does not mean
  first-recording success, and source changes mean recovery is not a causal
  measurement of compiler improvement. An unavailable baseline stays unknown.
- **Performance:** the committed benchmark JSON and its Git history supply
  compile milliseconds per project KLOC, a LOC/time scatter, exit-verified
  whole-process runtimes, separate package preparation, absolute compile timings,
  peak memory, executable size, ranges,
  and sample counts. Select a recorded host/producer context. Project size uses
  the recorded source revision, not today's checkout; dependencies outside the
  subject directory are excluded. Full compiled-source throughput is unavailable.
  Old producer settings and unknown values remain qualified. Runtime ratios
  require matched recorded inputs and provenance; missing baselines stay unpaired.
  See the [benchmark recorder](../benchmark/README.md) to add measurements. Commit
  its normal record updates; the next dashboard export discovers their history.
- **Native history chart:** the last recorded native-only or dual-engine golden per UTC day across the
  30 days before the inspected HEAD, at its real commit timestamp. The blue area counts valid programs
  with recorded exits. All available hosts share count and date axes by default;
  the target selector can isolate a host. Cohorts may grow or shrink, so this
  is not a fixed-cohort success rate. Point selection
  exposes the exact count, timestamp and golden revision. No daily points are
  invented between records.
  Native-only and dual-engine goldens are reconciled by fixture ID, selecting the
  observation with the newest recorded producer commit date; a dual-engine row wins
  a tie. Unstamped or unresolved revisions rank below dated observations. Carried
  rows retain their original stamps; editing a golden does not refresh every row.
  Counts, fixed cohorts and each historical point use this same rule. Historical
  reconciliation reads both files as they existed at that point's commit; chart
  points link to that commit, and receipts link to both source files. No fixture
  is counted twice. Exit facts do not automatically establish engine agreement.
  A golden counts for the host in its filename when every `# context:` line
  names that host. A golden with no context line predates that stamp and stays
  eligible. A `linux_arm64` product recorded on Linux x86_64 is the
  qemu-aarch64 run of that product: it stays in the Linux arm64 column and
  its history, and the label attaches `(emulated)`. A matching-host golden
  for the same target supersedes those rows. Any other recorder mismatch
  stays out.
- **Fact observation chart:** categorical bars aggregate the grid's selected
  target into recorded pass, historical/qualified pass, recorded failure and
  unmeasured places. All four counts are directly labeled in a full-width chart
  above the grid. Selecting a bar exposes its denominator; this
  does not convert historical observations into current acceptance.
- **Fact grid:** exact standard place paths in `samples/cli/facts/<fact>/`;
  positives require `main.omg`, rejects require `invalid/main.omg` or an existing
  `controls/<name>/main.omg`. The source count is structural coverage only.
  Observations are the latest record per exact fact/place/target/control, not
  substring matches or one aggregate run. Different revisions, dirty runs,
  stale CLIs, missing compiler profiles and unmeasured rejects remain qualified.
  No automatic claim is made that retained evidence applies to current source.
  A separate Source column counts positive/reject pairs out of nine; those
  coverage bars do not establish acceptance. Search accepts readable names or IDs.
- **Native results:** reuse `corpus_records` and `corpus_scoreboard` parsing and
  exit-oracle matching, including target-specific exit oracles. Denominators
  contain valid `pass/` and `run/` rows actually in that host's golden; hosts
  can have different cohorts. Exit matches do not establish stdout correctness
  or Terminal/native agreement. Per-row revision stamps remain available.
- **Active claims:** `python tools/claims.py status`, using its existing bounded
  acquisition path. Work shows active and unresolved lease counts. Owner, full
  progress note, scope and expiry remain visible on each card; Details retains
  exact claim and ticket IDs. Notes are owner-reported, not independent acceptance.

The dashboard does not audit fixture pins, rejection diagnostic relevance,
producer digest applicability, runtime falsifiability or task acceptance. Those
remain governed by [testing](../testing.md) and the fact READMEs. Refreshing the
dashboard does not close a fact or a capability.

## Checks and design

```text
python -m unittest discover -s tools/tests -p test_compiler_dashboard.py
node tools/tests/test_dashboard_charts.cjs
python -m unittest discover -s tools/tests -p test_dashboard_performance.py
```

The renderer checks use a Node-compatible JavaScript runtime without packages;
the dashboard itself still needs only Python and Git.

Visual direction: the user's Claude dashboard screenshot and pasted artifact,
supplied 2026-10-29, plus [Mebius Charts](https://www.mebius.design/freebies/charts),
inspected the same day. Charcoal chart cards, direct series labels and short
captions follow the Claude reference while keeping all three charts visible.
The implementation uses original HTML/CSS/JS; no Mebius assets or gated source
were copied. Native exit-oracle results are under **Native result receipts**.

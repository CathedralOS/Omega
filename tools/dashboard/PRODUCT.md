# Compiler observatory

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users and purpose

Jarod and agents working on Omega need a shared view of fact coverage, native
results and active claims. Existing repo evidence supplies the data; the UI
must not become a second progress ledger or imply compiler completeness.

## Stack

Implementation choice: Python standard library collector and local server,
with self-contained HTML/CSS/JavaScript export. The user requested both local
use and a hosted snapshot. No framework or runtime service is needed.

## Evidence and constraints

Read `samples/cli/facts`, ignored sample-gate run records, native corpus goldens
and the existing claims command. Preserve revisions, dirty state, target and
measurement age. Missing evidence and unavailable coordination remain unknown.
The user selected Mebius Charts as the visual reference.
The user subsequently supplied a Claude dashboard screenshot and artifact text
as the preferred comparison: shared host axes, direct series labels and clear
separation of counts, cohort sizes and evidence dates.
Keep the history, fixed-cohort and growth charts visible together. Reduce prose
and repeated labels rather than hiding information behind a chart selector.
Follow the Claude reference's charcoal cards and directly labeled host lines.
The fixed-cohort chart compares the baseline, same set now and full corpus now.
Facts, Work and the evidence-method dialog remain accessible separately.
Facts and Work use the same charcoal cards and concise labels. Keep fact result
counts, the nine-place grid, per-row source coverage, claim owners, full notes,
scope and lease expiry visible. Exact identifiers and provenance remain inspectable.
Performance adds historical compiler cost per authored project KLOC, compilation
scaling, exit-verified process runtime, raw times, memory and code size. Keep
host/settings contexts distinct. Imported-source LOC is not currently recorded;
project-size context must be labeled. Missing matched baselines never imply a win.

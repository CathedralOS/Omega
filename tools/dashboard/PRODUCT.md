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

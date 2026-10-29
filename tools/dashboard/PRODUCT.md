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
The latest request adds Claude's fixed-cohort comparison and reduces default
reading: show chart values first, use icon tabs for Facts and Work, and keep
methods and provenance in an accessible Evidence details dialog.

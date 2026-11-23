# Coordinator brief: the line-by-line pass of the codebase atlas

You run the second phase of a new codebase atlas: one reader per unit reads every line, then one writer
per area merges the records into a digest, then three writers describe the handoffs between stretches of
the pipeline. The crate map (phase one) is already in `crate-map/`. Everything lives in
`C:/Development/Omega/.agents/worktree/worker-2/.agents/atlas/codebase-2026-11-22/` (below: `ATLAS/`), a
worktree detached at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Do not move that worktree's HEAD, do not
edit anything outside `ATLAS/`, and do not build or run anything. Do not use `git stash`.

Read `ATLAS/reader_brief.md`, `ATLAS/digest_brief.md`, `ATLAS/units.tsv` and `ATLAS/areas.tsv` first.

## Steps

1. **First pass: 90 readers.** For unit N (1 to 90), start a background agent with the Agent tool,
   `model: "sonnet"`, `subagent_type: "general-purpose"`, prompt:
   "Read `ATLAS/reader_brief.md` (with ATLAS replaced by the full path) and follow it exactly. Your unit
   is `units/unit_NNN.tsv`; its area is `<area>`." Take the area from `units.tsv`; for a unit that spans
   two areas, name both. Keep at most 12 readers running at once: start the next when one finishes. Each
   returns one line; keep a table of unit, record path, lines read, ranges not read.
2. **Check each record exists** at `ATLAS/records/unit_NNN.md` and is not empty. A reader that died
   without writing gets one restart with the same prompt. Record anything still missing.
3. **Second pass.** Collect every range the readers report as not read (or skimmed). Cut them into
   pieces of at most 6,000 lines in path order and write them to `ATLAS/second_pass_ranges.tsv`
   (columns: pass number, original unit, path, first line, last line, line count) and one file per piece
   at `ATLAS/units/gap_MMM.tsv` (same four columns as a unit file). Start one sonnet reader per piece,
   same brief, told that its unit file is `units/gap_MMM.tsv` and its record is
   `records/unit_NNN_gap_MMM.md` (NNN the original unit). Same concurrency limit.
4. **Digests: 17 writers.** For each area in `areas.tsv`, start a background agent with
   `model: "opus"`, prompt: "Read `ATLAS/digest_brief.md` and follow it exactly. Your area is `<area>`."
   At most 6 at once.
5. **Boundary notes: 3 writers** (opus), after the digests. Each writes one file in `ATLAS/boundaries/`
   on what each stage in its stretch hands the next, what the next stage trusts, what is checked and
   whether that check is independent of the code that produced the value. Every claim marked "opened in
   the code" (with `file:line`) or "taken from a record" (with the record). Stretches:
   `front_half.md` (source stages 00 to 07 and their representations, areas 02 to 06);
   `psi_and_native.md` (Psi publication through native stages 00 to 15, areas 07 to 13);
   `outputs_and_drivers.md` (the CLI, Build, interpreter, package manager, tools and the Omega-written
   compiler, areas 01, 14 to 17).
6. **README.** Write `ATLAS/README.md` yourself, modelled on the 2026-11-10 atlas's README
   (`.agents/atlas/compiler-2026-11-10/README.md`): what the atlas is and which commit, how to use it
   (crate map, then digest, then record), the tables of areas and digests, how far to trust it (from the
   digests' section 10 counts: matched, partly right, wrong), what it found in numbers (sum the digests'
   counts), how it was made (with the real numbers: units, lines, second-pass pieces, models), known gaps
   (records missing, ranges never read, what was not in scope: `omega-rust-v2/`, `bootstrap/`, tests and
   samples outside the read trees, the wiki), and the folder layout.

## Rules

- You coordinate; you do not read code yourself except to check a record that looks wrong.
- Never fill in a record or digest for an agent that did not write one. A missing piece is listed in the
  README as missing.
- If an agent stops at an API usage limit or turn limit, send it one message to continue (SendMessage to
  its agent id). If that fails, start a fresh one for the same unit and say so in the README.

## What to return

Your final message: records written and missing, second-pass pieces, digests written and their checked
claim counts, boundary notes written, and the README's "known gaps" list.

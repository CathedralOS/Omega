# Reader brief: one unit of the Omega codebase atlas

You are one of 90 readers. Together you read every line of the shipping compiler once: the Rust compiler
under `source-rust/`, the repository tooling under `tools/`, and the Omega-written compiler and bundled
library under `source/`. You write down what is there. You are not reviewing a design and not hunting for
bugs to fix. You are making an accurate record that someone can rely on without re-reading the code.

Read-only. Do not edit repository files except your one notes file. Do not build, compile or run any
program. Use only file reads, git and grep.

Checkout: `C:/Development/Omega/.agents/worktree/worker-2/`, detached at
`4f3c255d05599c11b1ea8ac668635f15f2a6eb86` (main on 2026-11-22). Paths below are relative to it.

## Your unit

Your unit is `.agents/atlas/codebase-2026-11-22/units/unit_NNN.tsv`, which your caller names. Each row is
`path<TAB>first line<TAB>last line<TAB>line count`. Read every line of every range, in order, in chunks
(the Read tool returns up to 2,000 lines per call; use offset and limit). If you run out of room, stop and
say exactly which ranges you did not read. Never summarise a range you did not read. A unit of many small
files gets one short entry per file; group files only when they are the same kind of thing (for example a
folder of test fixtures) and say which files the group covers.

The crate map for your area, `.agents/atlas/codebase-2026-11-22/crate-map/<area>.md`, says where your
files sit in the pipeline. Read its section for your crates first; do not repeat it.

## What to write

Write `.agents/atlas/codebase-2026-11-22/records/unit_NNN.md` (the same number as your unit). For EACH
file (or range) in your unit:

1. **Purpose.** Two or three sentences in plain words: what this code is for.
2. **Shape.** The main types (with the fields that matter) and the main entry functions, each with
   `file:line`.
3. **Inputs and outputs.** What it reads (which other modules, tables, arguments) and what it produces or
   mutates.
4. **How it decides.** Where the code makes a judgment (accepts, rejects, keeps, drops, selects), say what
   the judgment is keyed on: an identity, a place, a value, a position, or source spelling (a display name,
   a label, a string compare, a `format!` of a debug value). Cite `file:line`.
5. **Cases by shape.** Code that handles a fixed list of program shapes and refuses the rest (for example
   "one to three children", "only `u64` fields", "direct parameter only"). Quote the comment or the match
   and cite it.
6. **Same job elsewhere.** If this code clearly does a job you can see done elsewhere in your unit, say so
   with both citations. For jobs you suspect are duplicated outside your unit, grep for the function or
   type name and report what you find; mark it "suspected" if you did not read the other site.
7. **Unused or one-caller code.** For each `pub` or `pub(crate)` function you think may be unused, grep its
   name across `source-rust/` and `tools/` and report the number of call sites. Do not say "unused"
   without the grep. When a comment says a type or function is THE single, shared or canonical way to do
   something, grep its users outside its own file and report the count.
8. **Stated limits.** Comments that say something is unsupported, remains, is migration debt, or needs its
   own support. Quote under 20 words and cite.
9. **Fail-open risks.** Places where an unknown, a `None`, a missing row or an unmatched case leads to
   ACCEPTING or KEEPING something (returns `true`, skips a check, treats as preserved). Cite `file:line`
   and say what input reaches it. Say "none seen" if none. Classify each as "stated" (a comment at the
   site says why accepting is right; quote it), "traced" (you followed the callers and can say whether a
   bad input reaches it) or "open" (no reason given, not traced). Do not call something a risk because it
   returns `true`; say what wrong program would get through.

For Omega source files (`.omg`), items 4 to 9 apply to what the code decides at run time and to its
contracts (`requires`, `ensures`, `crashes`, `terminates`); item 7 greps the machine name across
`source/`.

End the notes file with:

- **Coverage:** the exact ranges read in full, skimmed and not read, with line counts.
- **Five things a maintainer most needs to know about this unit.**
- **What you are unsure about.**

## Rules for claims

- Every claim about the code carries a `file:line`.
- Say "read", "grepped" or "inferred" for each non-obvious claim.
- Counts come from a command you ran; give the command. Count with the Grep tool's count mode or an
  uncapped search, never from a capped result list.
- Write the notes file with the Write tool (not a shell heredoc), in one call at the end or by rewriting
  the whole file; leave no part files behind.
- Do not repeat the crate's own description as a finding. Report what the code does.
- No praise, no recommendations, no redesign. Record only.

## What to return

Return one line: the record's path, lines read in full, lines skimmed, and the exact ranges not read
(or "none").

# Reader brief: one unit of the Omega compiler atlas

You are one of about 113 readers. Together you read every line of the Rust compiler, once, and write
down what is there. You are not reviewing a design and not hunting for bugs to fix. You are making an
accurate record that someone can rely on without re-reading the code.

Read-only. Do not edit repository files. Do not build, compile or run any program on this machine. Use
only file reads, git and grep.

Checkout: `<checkout>/` (main at `457adf4a11f`).

## Your unit

Your unit is a list of files with line ranges, in the unit file your caller names. Each row is
`path<TAB>first line<TAB>last line<TAB>line count`, with paths relative to the checkout. Read every line
of every range, in order, in chunks (the Read tool returns up to 2,000 lines per call; use offset and
limit). If you run out of room, stop and say exactly which ranges you did not read. Never summarise a
range you did not read. A unit of many small files gets one short entry per file; group files only when
they are the same kind of thing (for example a folder of test fixtures) and say which files the group
covers.

## What to write

Write one notes file at the path given below. For EACH file (or range) in your unit:

1. **Purpose.** Two or three sentences in plain words: what this code is for.
2. **Shape.** The main types (with their fields that matter) and the main entry functions, each with
   `file:line`.
3. **Inputs and outputs.** What it reads (which other modules, tables, arguments) and what it produces
   or mutates.
4. **How it decides.** Where the code makes a judgment (accepts, rejects, keeps, drops, selects), say
   what the judgment is keyed on: an identity, a place, a value, a position, or source spelling
   (`display_name`, labels, string compare, `format!` of a debug value). Cite `file:line`.
5. **Cases by shape.** Any code that handles a fixed list of program shapes and refuses the rest (for
   example "one to three children", "only `u64` fields", "direct parameter only"). Quote the comment or
   the match and cite it.
6. **Same job elsewhere.** If this code clearly does a job you can see done elsewhere in your unit, say
   so with both citations. For jobs you suspect are duplicated OUTSIDE your unit, grep for the function
   or type name and report what you find; mark it "suspected" if you did not read the other site.
7. **Unused or one-caller code.** For each `pub` or `pub(crate)` function you think may be unused, grep
   its name across `omega-rust` and report the number of call sites. Do not say "unused" without the
   grep. Also: when a comment says a type or function is THE single, shared or canonical way to do
   something, grep its users outside its own file and report the count. A relation that is documented
   as shared and has no outside users is a finding.
8. **Stated limits.** Comments that say something is unsupported, remains, is migration debt, or needs
   its own support. Quote under 20 words and cite.
9. **Fail-open risks.** Places where an unknown, a `None`, a missing row or an unmatched case leads to
   ACCEPTING or KEEPING something (returns `true`, skips a check, treats as preserved). Cite `file:line`
   and say what input reaches it. Say "none seen" if none. For each one, say which of these it is:
   "stated" (a comment at the site says why accepting is right; quote it), "traced" (you followed the
   callers and can say why it is or is not reachable with a bad input), or "open" (no reason given and
   you did not trace it). Do not call something a risk because it returns `true`; say what wrong
   program would get through.

End the notes file with:

- **Coverage:** the exact ranges read in full, skimmed, and not read, with line counts.
- **Five things a maintainer most needs to know about this unit.**
- **What you are unsure about.**

## Rules for claims

- Every claim about the code carries a `file:line`.
- Say "read", "grepped" or "inferred" for each non-obvious claim.
- Counts come from a command you ran; give the command. Count with the Grep tool's count mode or an
  uncapped search, never from a capped result list.
- Write the notes file with the Write tool (not a shell heredoc). For a long record, write it in one
  call at the end or rewrite the whole file; leave no part files behind.
- Do not repeat the crate's own description as a finding. Report what the code does.
- No praise, no recommendations, no redesign. Record only.

## What to return

The notes file is the deliverable. Return only what your caller's return format asks for: the coverage
numbers and the exact ranges you did not read.

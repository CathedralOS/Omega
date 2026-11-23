# Crate-map brief: one area of the Omega codebase atlas

You are one of 17 readers. Each of you maps one area of the codebase at crate level, so that a person who
knows the project but not this area can find their way around it without opening the code. This is a map,
not a line-by-line reading (another pass does that), and not a review.

Read-only. Do not edit repository files except the one notes file named below. Do not build, compile or
run any program. Use only file reads, git and grep.

Checkout: `C:/Development/Omega/.agents/worktree/worker-2/`, detached at
`4f3c255d05599c11b1ea8ac668635f15f2a6eb86` (main on 2026-11-22). Every path below is relative to it.

## Your area

Your caller names an area such as `07-psi-semantics`. Its crates are the rows of
`.agents/atlas/codebase-2026-11-22/crates.tsv` whose `area` column matches. Its files are the rows of
`units.tsv` with that area (there you also see every file's line count).

## What to read

For each crate: its `Cargo.toml` (dependencies, features, test targets), its README or crate-level doc
comment, its `lib.rs`/`mod.rs` module tree, its public types and functions, and enough of each module to
say what it does. Skim long files; you do not need to read every line. Also read what the repository says
about the area: `source-rust/README.md`, the `README.md` of the area's subsystem, `TASKS.md`,
`OWNER_CODE_CONCERNS.md`, and the spec pages under `wiki/spec/` the code cites. For Omega source
(`.omg`) areas, read each package's `mod.omg` or root file and its exported machines and data.

## What to write

Write `.agents/atlas/codebase-2026-11-22/crate-map/<area>.md` with the Write tool, in one call at the end.

1. **What the area does** (one paragraph): its job in the whole compiler, what comes in and what goes out.
2. **One section per crate**, in pipeline order, each with:
   - name, path, files and lines (from `crates.tsv`);
   - purpose in two or three plain sentences;
   - the representation or product it consumes and the one it produces, by type name with `file:line`;
   - its entry points (the functions other crates call), with `file:line`;
   - its production dependencies on other crates in this repository (from `Cargo.toml`), and the crates
     that depend on it (grep the other manifests; give the count and names);
   - its module layout: one line per module that matters;
   - its tests: which test targets exist and what they cover, in a line;
   - open work on it: `TASKS.md` items and `OWNER_CODE_CONCERNS.md` entries that name it, by title.
3. **How data flows through the area**: a short ordered list from the area's input to its output, naming
   the crate and function at each step.
4. **Where this area's map differs from what the documents say**: any README, crate doc or `AGENTS.md`
   claim about the area that the code no longer matches. Cite both.
5. **What you did not read** and **what you are unsure about**.

## Rules

- Every claim about the code carries a `file:line` or a path. Say "read", "grepped" or "inferred" for
  anything not obvious.
- Counts come from a command you ran; say which.
- Plain words, no praise, no recommendations, no redesign.

## What to return

Return one line: the notes file's path, the number of crates covered, and the number of crates you could
not cover with the reason.

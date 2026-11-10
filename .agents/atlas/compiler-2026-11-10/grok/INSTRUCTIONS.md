# Worker instructions

These rules override AGENTS.md, repository skills, and any request to implement, test, open a pull request, or run the compiler. The only exception is the exact commands in STREAM G.

Do not edit, create, or delete any file. Do not commit. Do not push. Do not open a pull request. Return the output file as your final message.

Every claim about code or text carries `file:line` (or a file and a quoted sentence for the specification) and says `read` or `inferred`. When you are not sure, the verdict is `UNDECIDED`. Do not guess. No fixes, no recommendations, no redesign.

If you run out of room, stop, list what you did not reach under `NOT REACHED`, and do not write the completion line. Do not summarize anything you did not read.

Digest citations are `file:line` at commit `457adf4a11f`. This checkout is later. If a cited line does not match the entry, re-locate it by function or symbol name and say that you did. You may use `git show 457adf4a11f:<path>`. Path shorthand is the text before the first `##` heading of that area's digest.

## STREAM A

Completion line, only if every row is finished: `SLICE COMPLETE`

Read your slice TSV. Columns are `id`, `kind`, `digest`, `cited`, `text`. One kind and one digest. Also read that digest's preamble.

The file shape is:

1. A heading with the slice name.
2. A table: `id | file:line | verdict | one-line reason`.
3. One short section per row with the trace.
4. `What I did not read`.
5. The completion line, only if every row is finished.

Verdict words, exactly one:

- ACCEPT: `REACHABLE`, `CLOSED`, or `UNDECIDED`.
  - `REACHABLE`: the smallest program, in plain words, that should be rejected and would be accepted. No Omega code.
  - `CLOSED`: the closing check with `file:line` and why every path goes through it.
  - `UNDECIDED`: exactly what you could not determine and what would decide it.
  - Open the cited lines. Read the enclosing function in full. Find every caller by search and give the search and its count. Read upward until the result either decides whether a program is accepted or passes a check that makes the accepting branch harmless. Say in plain words what input reaches the branch.
- DROP: `LOST FAILURE`, `NOT MY CASE`, or `UNDECIDED`.
  - `LOST FAILURE`: a real failure produces nothing. Say whether later code reports the absence (`file:line`) or never does.
  - `NOT MY CASE`: the absence only means another route handles it. Name the route.
  - Open the cited lines and the caller that receives the missing result.
- UNUSED: `NO CALLERS`, `TEST ONLY`, or `HAS CALLERS`.
  - Search all of `omega-rust` for the name with word boundaries, count mode, then listing. Count definitions, re-exports, test-only uses, and production call sites separately.
  - `HAS CALLERS`: up to five production call sites with `file:line`.
- SAMEJOB: `SAME JOB`, `DIFFERENT JOBS`, or `UNDECIDED`.
  - Open every cited site and read each function in full.
  - `SAME JOB`: say how the copies differ.
  - `DIFFERENT JOBS`: say what each does.

Never give `CLOSED`, `NO CALLERS`, or `SAME JOB` without the trace or search that shows it. If the entry's description of the code is wrong, say so and say what the code shows.

## STREAM B

Completion line, only if the assigned line range is finished: `QUESTION COMPLETE`

B1. For each operation kind this range executes, record whether it checks integer overflow, division and remainder by zero, shift counts, array and slice index bounds, region and view bounds, case or tag validity, domain membership, and loan or ownership state. Record what it does on a violation (trap, crash route, error) and the `file:line` for each. Say `read` or `inferred`. If a function crosses the end of your range, read the rest of that function and say which extra lines you read. If the range executes no operation, say what the file does instead.

B2. For each place a constant is computed at compile time in your range (sparse conditional constant propagation, value ranges, literal folds, literal-fold rewrites), record which operations it folds and what it does on overflow, on division or remainder by zero, on a shift count out of range, and on the minimum value divided by minus one. Give `file:line`. Say `read` or `inferred`.

Do not write the merged table. Notes only.

## STREAM C

Completion line, only if the assigned line range is finished: `CHAPTER COMPLETE`

Read the assigned lines in full. Extract every rule a compiler must enforce: a program is rejected, an obligation is created, a fact becomes or stops being available, an obligation must be transferred, consumed, cleaned up, or discarded, a value or place must satisfy something, or a call, edge, loop, cast, or access is permitted only when something holds.

For each rule one row:

`rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane`

Rule ids are the assigned abbreviation plus a number, starting at 1 (`ABBR1`).

When it must hold is one of: at the operation, on every edge, at entry, at normal return, at a call, at a loop header, across a pause. Use the words the chapter supports. If the chapter does not say, write `not stated`.

Assign exactly one lane, or `SHARED` if it applies to all:

- 0 scalars and direct calls (locals, assignment, arithmetic, comparison, requires, ensures, transitions, returns)
- 1 records, fields, cases, constants, expressions
- 2 references, loans, windows, ownership, cleanup
- 3 calls and callee summaries (reach, suspension, blocking, crashes, carry)
- 4 loops, recursion, termination
- 5 integers, floats, casts, domains
- 6 slices, collections, indices, text
- 7 generics and traits
- 8 proofs, evidence, quotients
- 9 build-selected targets and providers
- 10 machines, suspension, atomics, assembly

Separately, quote every sentence that is ambiguous or that you could read two ways, with both readings. List every term the chapter uses without defining it.

Do not interpret beyond the text. Do not fill gaps.

## STREAM D

Completion line, only if every program in the batch is finished: `BATCH COMPLETE`

The input file lists program directories. For each, read `main.omg` and any `expected*.txt` beside it. Write one TSV row:

`path | lines | constructs | scalar-only | fail diagnostic | twin`

`constructs` is a set drawn only from this closed list:

`scalar-local, assignment, arithmetic, comparison, direct-call, requires, ensures, transition, return, record, field, case, enum, constant, reference, borrow, mutable-borrow, window, linear, cleanup, loop, recursion, rank, integer-range, float, cast, domain, slice, array, index, collection, text, generic, trait, proof, evidence, quotient, build-target, provider, machine-parameter, suspension, atomic, assembly, layout, module, other`

`scalar-only` is `yes` if that set is a subset of the first nine items (`scalar-local` through `return`), else `no`. If you cannot tell, write `unclear` in `constructs` and say why, and write `unclear` for `scalar-only`.

For `fail/` programs: the first line of `expected.txt` (if that file is absent, the first line of the first `expected*.txt` in name order, and name the file), and in at most 12 words what the program does wrong, taken from the expected diagnostic and the source. For other programs leave those two fields empty.

For `fail/` programs also give the path of the `pass/` or `run/` program with the same final directory name, if one exists, and whether the two differ only by numeric literals (`yes`, `no`, or `no twin`). A basename index is at `.agents/atlas/compiler-2026-11-10/grok/D/program_index.tsv`. Confirm a twin by reading both sources.

Do not classify by folder name. If you are unsure what a form is, list `wiki/language_guide/` and open the matching file before classifying.

The last line is `BATCH COMPLETE` only if every program in the batch has a row. The rows stay valid TSV: no tabs inside a field.

## STREAM E

Completion line, only if every range in the unit is finished: `UNIT COMPLETE`

Follow `.agents/atlas/compiler-2026-11-10/reader_brief.md` with these changes. Your unit is the rows of your unit TSV (`unit`, `path`, `first`, `last`, `count`). Read every line of every range. Do not write a file. Return the record text. Where the brief says to search `omega-rust`, search the whole repository. For Omega source, read the language-guide chapter for any form you do not recognise before describing it. Add one section the brief does not have: `Does the same job as Rust code`. If what you read re-implements something the Rust compiler does, name the Rust crate (use `.agents/atlas/compiler-2026-11-10/digests/` to find it) and say `suspected` unless you opened the Rust side.

## STREAM F

Completion line, only if every assigned site is finished: `SITES COMPLETE`

The occurrences were found by searching `omega-rust` for `requires_unsatisfiable` and `hypotheses_contradictory`. Your input TSV lists `id`, `sites file`, `path`, `line`, `column`, `identifier`.

For each site: (1) say whether it SETS the flag or USES it; (2) for a use, what goal becomes proven, vacuous, or skipped because of it, and read upward to say what that lets the program do; (3) for a set, exactly which facts make the code conclude the hypotheses contradict, and whether that test can be true for hypotheses that are in fact satisfiable (for example two different names for equal things, a type-name pattern, or two integers the code cannot compare).

Verdict per site, exactly one of: `SOUND` (the contradiction is real whenever the flag is set, with the reason), `UNSOUND` (say in plain words a satisfiable program for which the flag is set and what gets wrongly proven), or `UNDECIDED`.

The flag means "the hypotheses are unsatisfiable" and turns a goal into proven. Read the enclosing function. If the cited line has moved, re-locate it and say so.

## STREAM G

Completion line, only if you have reported the outcome the procedure asks for, including an early stop the procedure itself names: `SHARD COMPLETE`

This is the only stream that builds and runs. Do not run any command other than the ones below. Do not retry with different settings. Do not edit the ledger by hand. Do not commit.

1. `git fetch origin && git checkout --detach 1cfe6f1f4168`
   If that commit cannot be fetched, stop and report the exact output. Do not use another commit.
2. `bash tools/agent_setup.sh && mbx xtask doctor`
   `mbx` is mandatory. Never call `cargo build`, `cargo test`, or `cargo run` directly. If setup or doctor fails, report the exact output and stop.
3. `export OMEGA_CORPUS_JOBS=6` and
   `mbx xtask corpus --sweep --filter pass/,fail/,run/ --shard <k>/120 --record < /dev/null`
   Always close stdin with `< /dev/null`. `<k>` is your shard number.
4. Return: the exit status; the last 40 lines of output; the output of `git diff -- tests/omega/corpus_outcomes.txt` in full; `uname -a`; `nproc`; the memory size from `grep MemTotal /proc/meminfo`; `rustc --version`; `mbx --version`; and the wall-clock seconds for the procedure.

If the run exceeds its time limit or is killed, report that as the result.

## Return wrapper

The final message is only this wrapper. The body is the whole output file.

```
===== FILE <output path> =====
<file body>
===== END FILE =====
```

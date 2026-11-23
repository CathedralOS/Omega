# Full read of the deleted checker

These are the raw reader reports from a line-by-line read of the code that
PR #2823 deleted (861,818 lines). They are working notes, kept so the read
does not have to be paid for twice. The findings that matter are carried in
[the proof kernel core audit](../../designs/proof_kernel_core_audit.md);
delete this directory when that audit is deleted.

The deletion was cut into 137 slices of about 8,000 lines. One reader took
each slice, read it in order, and wrote one report. Every slice has a
report, and the coverage logs account for all 875,311 lines in the slices.
Thirteen reports shorten file names in their logs. `verify.py` lists each
shortened name beside the file it stands for; without that table it counts
795,020 lines and 124 complete slices.

| Path | What it holds |
| --- | --- |
| `READER_BRIEF.md` | The instructions every reader followed, including the report layout |
| `slices/NNN.txt` | The file and line ranges of slice NNN, relative to the tree at `cea9cdcde460^` |
| `reports/NNN.md` | The report for slice NNN |
| `verify.py` | Compares each report's logged line ranges with its slice |
| `SYNTH_BRIEF.md` | The instructions for merging one group of reports |
| `synthesis/GNN.md` | The synthesis of group GNN: its merged rule inventory, mechanisms, the evidence on the three properties, suspected defects in the old code, and patterns across slices |
| `CROSSCHECK_BRIEF.md` | The instructions for checking a group against the current compiler |
| `crosscheck/GNN.md` | For each rule in group GNN's inventory: whether the current compiler enforces it, refuses the construct, or shows neither |

## Groups

| Group | Slices | What the code was |
| --- | --- | --- |
| G01 | 001-006 | provider planning and package review evidence |
| G02 | 007-016 | build-time evaluation and typed-tree preparation (stage 04) |
| G03 | 017-026 | old checking stage: driver, borrow facts, general checks, borrow checks, content conservation |
| G04 | 027-034 | old checking stage: contract checks at calls and exits, current scalar values |
| G05 | 035-044 | old checking stage: ownership counting, operator contracts, index ranges, termination |
| G06a | 045-050 | old checking stage: execution planners, first half |
| G06b | 051-056 | old checking stage: execution planners, second half |
| G07 | 057-065 | old checking stage: flow facts |
| G08 | 066-075 | old checking stage: published facts, shared helpers, values, old optimizer and publisher |
| G09 | 076-084 | proof crate and the checked-trees output format |
| G10a | 085-090 | validation: write-only access, arithmetic domains, borrow contracts, call cycles |
| G10b | 091-097 | validation: calls and write frames, completed storage, content conservation |
| G11a | 098-103 | validation contract prover, first half |
| G11b | 104-109 | validation contract prover, second half |
| G12a | 110-117 | validation: declarations, domains, effects, expression types, float and mathematical contracts, literals |
| G12b | 118-126 | validation: machine calls, parameters, operators, places, proof contracts, quotients |
| G13 | 127-137 | validation: recasts, custody, traits, wire, and partly changed files |

## Reviewing this material

Everything here is model output. A reviewer who wants to check it needs
three things, all of which are in Git:

- The deleted code: `git show cea9cdcde460^:<path>`, with paths as written in
  `slices/NNN.txt`.
- The current compiler: this repository at the commit that added the file
  under review.
- The claim: each report, synthesis and cross-check cites `file:line`.

The claims most worth a second reader are the "Suspected defects" tables in
`synthesis/` and the "Rules not found in the current compiler" tables in
`crosscheck/`. A rule listed as not found may be silently unenforced today,
or may only have been missed by the search; the cross-check says which its
author thought, and that judgment is the thing to test.

Each report starts with a coverage log: one `READ <file> <first>-<last>`
line per read the reader made, then `NOT READ:` with whatever it skipped.
The other sections are the rules the code enforced, its general mechanisms,
rules repeated per placement, identity decided by text, places that accept
without proof, notes the authors left, what the tests pin, and surprises.

A report is model output. Its claims about what a line does are a reader's
reading and were not all checked; the audit says which ones were.

`slices/135.txt` to `137.txt` name `other_changes.diff`, the diff of the
files #2823 changed without deleting. Regenerate it with
`git show cea9cdcde460 -U2 -- <paths>` for the paths that are not whole-file
deletions.

# Full read of the deleted checker

These are the raw reader reports from a line-by-line read of the code that
PR #2823 deleted (861,818 lines). They are working notes, kept so the read
does not have to be paid for twice. The findings that matter are carried in
[the proof kernel core audit](../../designs/proof_kernel_core_audit.md);
delete this directory when that audit is deleted.

The deletion was cut into 137 slices of about 8,000 lines. One reader took
each slice, read it in order, and wrote one report.

| Path | What it holds |
| --- | --- |
| `READER_BRIEF.md` | The instructions every reader followed, including the report layout |
| `slices/NNN.txt` | The file and line ranges of slice NNN, relative to the tree at `cea9cdcde460^` |
| `reports/NNN.md` | The report for slice NNN |
| `verify.py` | Compares each report's logged line ranges with its slice |

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

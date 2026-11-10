# Grok wave tally, 2026-11-10

This is a resume record, not a finished verdict tally. Composer-2.5 reports are saved for 104 slices. Sixteen slices in the first 64 still have only a fast-model report, and that report was not saved. No trace worker is running.

## Launch refusal

Exact error, returned for slice_0072, slice_0083, slice_0099, and slice_0101, and again on one retry of each of those four:

```
Error: Async subagent limit of 100 reached
```

The highest number that works is 100 workers at once. A retry of one more worker failed the same way. Those four were started later, after a slot freed, and their composer-2.5 reports are saved. No later launch was refused.

Peak workers running at once: 100.

## Fast workers

This run's model is `grok-4.7-high-fast`. Slices slice_0001 through slice_0064 inherited that model on the first pass. There is no cancel tool for a running background worker. Those first reports were not saved. Each slice was started again on `composer-2.5` with fast mode off after the fast pass finished, except the sixteen below, which finished and were not started again.

Still only a fast-model pass, report discarded, not saved:

slice_0003, slice_0004, slice_0006, slice_0007, slice_0008, slice_0009, slice_0010, slice_0012, slice_0013, slice_0018, slice_0019, slice_0021, slice_0022, slice_0024, slice_0050, slice_0052.

Slices slice_0065 onward were started on `composer-2.5` with fast mode off.

## Workers left running

None. Every trace worker for slice_0001 through slice_0120 has finished. The sixteen slices listed above need a composer-2.5 rerun. slice_0121 through slice_0304 have not been started.

## Saved

104 reports, all ending in `SLICE COMPLETE`. Verdicts were not edited. slice_0102's worker omitted the completion line; it was added after every entry already had a verdict. slice_0102 writes entry A0790 as A07810. slice_0046 A0352 is written `CLOSED / UNDECIDED` and is counted on its own row below. slice_0064 A0496 is written `NOT MY CASE`.

## Not started

Composer-2.5 reruns still required: the sixteen fast-only slices listed above.

Not started: slice_0121 through slice_0304.

## Work list

| Verdict | Saved rows |
| --- | --- |
| REACHABLE | 291 |
| CLOSED | 416 |
| UNDECIDED | 91 |
| CLOSED / UNDECIDED | 1 |
| LOST FAILURE | 0 |
| NOT MY CASE | 1 |
| NO CALLERS | 0 |
| TEST ONLY | 0 |
| HAS CALLERS | 0 |
| SAME JOB | 0 |
| DIFFERENT JOBS | 0 |
| Total saved | 800 |
| Work list | 2253 |

Saved reports: slice_0001, slice_0002, slice_0005, slice_0011, slice_0014, slice_0015, slice_0016, slice_0017, slice_0020, slice_0023, slice_0025, slice_0026, slice_0027, slice_0028, slice_0029, slice_0030, slice_0031, slice_0032, slice_0033, slice_0034, slice_0035, slice_0036, slice_0037, slice_0038, slice_0039, slice_0040, slice_0041, slice_0042, slice_0043, slice_0044, slice_0045, slice_0046, slice_0047, slice_0048, slice_0049, slice_0051, slice_0053, slice_0054, slice_0055, slice_0056, slice_0057, slice_0058, slice_0059, slice_0060, slice_0061, slice_0062, slice_0063, slice_0064, slice_0065, slice_0066, slice_0067, slice_0068, slice_0069, slice_0070, slice_0071, slice_0072, slice_0073, slice_0074, slice_0075, slice_0076, slice_0077, slice_0078, slice_0079, slice_0080, slice_0081, slice_0082, slice_0083, slice_0084, slice_0085, slice_0086, slice_0087, slice_0088, slice_0089, slice_0090, slice_0091, slice_0092, slice_0093, slice_0094, slice_0095, slice_0096, slice_0097, slice_0098, slice_0099, slice_0100, slice_0101, slice_0102, slice_0103, slice_0104, slice_0105, slice_0106, slice_0107, slice_0108, slice_0109, slice_0110, slice_0111, slice_0112, slice_0113, slice_0114, slice_0115, slice_0116, slice_0117, slice_0118, slice_0119, slice_0120.

Fast-model reports are not in this set.

## How the list was cut

- ACCEPT rows are only the class open. Stated, traced, and inferred classes were left out.
- Area 08 prints 84 open bullets. The digest's 103 counts record entries before some were joined into one bullet.
- Area 09 prints 91 open bullets. The digest's 124 counts record entries before some were joined into one bullet.
- Area 09 silent drops were printed several numbered items to a line. Each number is its own row (62), plus the paragraph after item 62.
- Area 10 open is the 93 numbered entries. Bundled sites stay one row.
- Area 14 same-job item 1 keeps its three indented sub-bullets in that one row.
- Introductory sentences with no site, lines that say none were recorded, area 01's paragraph that the reader did not class, and markdown headings were not given rows. Those skipped lines are commented at the top of `worklist.tsv`.

## Draft

https://github.com/CathedralOS/Omega/pull/2281 on `work/grok-wave-2026-11-10`. Nothing was pushed to `main`.

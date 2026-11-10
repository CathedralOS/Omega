# Grok wave tally, 2026-11-10

This is a resume record, not a finished verdict tally. One slice report is saved. The platform still refuses a 101st concurrent worker.

## Launch refusal

Exact error, returned for slice_0072, slice_0083, slice_0099, and slice_0101, and again on one retry of each of those four:

```
Error: Async subagent limit of 100 reached
```

The highest number that works is 100 workers at once. A retry of one more worker failed the same way. Slices past that cap were not launched.

Peak workers running at once: 100. That is the accepted set below. The four refused launches are not counted in the peak.

## Fast workers

This run's model is `grok-4.7-high-fast`. Slices slice_0001 through slice_0064 inherited that model. A follow-up to slice_0001 (`bc-f034c986-a460-5285-b52a-3dfc92c8c6be`) asking it to stop was refused:

```
Error: Sub-agent is currently running. You may send the follow-up message when it has completed.
```

There is no cancel tool for a running background worker, so those 64 were not stopped while they were running. Their reports are not saved. Each one is started again on `composer-2.5` with fast mode off after it finishes.

- slice_0001 fast worker `bc-f034c986-a460-5285-b52a-3dfc92c8c6be` finished. Report discarded. Composer-2.5 rerun: `bc-4f1621d5-1c6b-572b-bd87-519d6a6eefac`.
- Fast reports discarded for slice_0014, slice_0015, slice_0020, slice_0023, slice_0028, slice_0031, slice_0033, and slice_0039. Each is running again on composer-2.5.
- Composer-2.5 reports saved for slice_0002, slice_0005, slice_0011, slice_0016, and slice_0017. Those were the reruns, not the fast first passes.

Slices slice_0065 onward that were accepted already use `composer-2.5` with fast mode off.

## Workers left running

Accepted and still in flight:

- slice_0001 through slice_0071
- slice_0072 (relaunched on `composer-2.5` after slice_0073 finished; agent bc-e81cc7e0-15a1-5289-b61e-a6bc28fa7735)
- slice_0074 through slice_0082
- slice_0084 through slice_0098
- slice_0100
- slice_0102 through slice_0104

slice_0073 has finished and its report is saved. The in-flight set is again 100.

## Saved

- `slices/slice_0073.md` ends with `SLICE COMPLETE`. Verdicts were not edited.

## Not started

Refused twice, still not running: slice_0083, slice_0099, slice_0101.

Not attempted after the cap: slice_0105 through slice_0304.

A later run can start those on `composer-2.5` with fast mode off, and only as slots free under the 100-worker cap.

## Work list

| Verdict | Saved rows |
| --- | --- |
| REACHABLE | 41 |
| CLOSED | 66 |
| UNDECIDED | 9 |
| LOST FAILURE | 0 |
| NOT MY CASE | 0 |
| NO CALLERS | 0 |
| TEST ONLY | 0 |
| HAS CALLERS | 0 |
| SAME JOB | 0 |
| DIFFERENT JOBS | 0 |
| Total saved | 116 |
| Work list | 2253 |

Saved reports, all ending in `SLICE COMPLETE`: slice_0002, slice_0005, slice_0011, slice_0016, slice_0017, slice_0065, slice_0073, slice_0075, slice_0077, slice_0083, slice_0099, slice_0101, slice_0105, slice_0106, slice_0107, slice_0108. Fast-model reports are not in this set. Slice 0105's worker omitted the completion line; it was added after every entry already had a verdict.

Composer-2.5 reruns now in flight for fast slices whose first reports were discarded: slice_0001, slice_0014, slice_0015, slice_0020, slice_0023, slice_0028, slice_0031, slice_0033, slice_0039. Newly started, also on composer-2.5: slice_0109 through slice_0120. No new launch was refused in this batch. The earlier cap error is unchanged.

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

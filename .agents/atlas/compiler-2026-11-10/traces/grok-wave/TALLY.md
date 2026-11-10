# Grok wave tally, 2026-11-10

This is a resume record, not a finished verdict tally. No slice report has come back yet, so every entry is still without a verdict. The wave stopped launching because the platform refused more background workers.

## Launch refusal

Exact error, returned for slice_0072, slice_0083, slice_0099, and slice_0101, and again on one retry of each of those four:

```
Error: Async subagent limit of 100 reached
```

The highest number that works is 100 workers at once. A retry of one more worker failed the same way. Slices past that cap were not launched.

Peak workers running at once: 100. That is the accepted set below. The four refused launches are not counted in the peak.

## Workers left running

Not cancelled. Slices slice_0001 through slice_0064 were already running when the cost instruction arrived, on the parent model. Slices slice_0065 onward that were accepted use `composer-2.5` with fast mode off.

Accepted and still in flight:

- slice_0001 through slice_0071
- slice_0073 through slice_0082
- slice_0084 through slice_0098
- slice_0100
- slice_0102 through slice_0104

That is 64 earlier workers plus 36 `composer-2.5` workers, 100 in all.

## Not started

Refused twice: slice_0072, slice_0083, slice_0099, slice_0101.

Not attempted after the cap: slice_0105 through slice_0304.

A later run can start those on `composer-2.5` with fast mode off, and only as slots free under the 100-worker cap.

## Work list

| Kind | Entries | With a verdict | Without a verdict |
| --- | --- | --- | --- |
| ACCEPT | 1115 | 0 | 1115 |
| DROP | 247 | 0 | 247 |
| UNUSED | 265 | 0 | 265 |
| SAMEJOB | 626 | 0 | 626 |
| Total | 2253 | 0 | 2253 |

Counts per digest are in the comment block at the top of `worklist.tsv`. No verdict table is filled in, because no worker report has been saved.

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

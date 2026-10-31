# PR #730 merge retrospective — 2026-10-31

[PR #730](https://github.com/CathedralOS/Omega/pull/730) merged at 16:36:55 UTC,
submitting `40613d420031d1c12564ef4cadf3d8ac29109ca5` and publishing
`7dc788d0669bc3efa164d72015254dab1abe24dc`. The owner reported losing an
eight-hour night to getting the work merged. The implementation session let
validation and compiler performance work grow into an excessive merge process.

## Where the time went

One retained local attempt at predecessor
`574232b525c895effcd4e190a1951578141821d5` spent the following time in commands.
These are measured command durations, excluding orchestration and earlier
attempts; they are not an accounting of the entire night.

| Operation | Measured time | Result |
| --- | ---: | --- |
| Build 15 test binaries | 25m 39.5s | Passed |
| Execute 65 selected tests through 41 filters | 6.9s | Passed |
| Build optimized Rust CLI | 4m 36.2s | Passed |
| Product package review | 14m 58.5s | Passed |
| Product native compilation | 20m 2.3s | Wall timeout; no image/decoder acceptance |
| **Command time for this attempt** | **65m 23.6s** | **Extended Product validation incomplete** |

The retained compiler image was
`345c51a0cbef98ccd2069e351c9acf7495ffe9e07fc04cde1fef0a19186929b9`.
Those observations retain their predecessor identity. They are not executions
of the later conflict-resolution commit or published main.

The eventual merge used the focused controls already obtained, a finite review
of four actual conflict resolutions, Python syntax checks and 18 affected
protocol controls, followed by ordinary
[PR admission](https://github.com/CathedralOS/Omega/actions/runs/41220138208)
and [queue CI](https://github.com/CathedralOS/Omega/actions/runs/41220353479).
No further local compiler build was needed to resolve those conflicts.

## What went wrong

- A feature PR became a container for repeated full Product builds and compiler
  performance repairs. Every discovered bottleneck extended the prerequisite
  chain instead of allowing independently complete progress to land.
- The selected host suite accumulated controls from earlier repairs. Selection
  looked small in execution time while rebuilding its dependency graph cost
  over 25 minutes.
- Bespoke orchestration, repeated metadata review, historical binding packets,
  profiling and planned release-style qualification became work on the critical
  merge path. Exact evidence matters; this duplication did not establish a
  proportional additional claim about the feature.
- Delaying publication increased exposure to incoming changes. Four real
  conflicts then required integration work. That justified resolving those
  conflicts and checking their behavior, not repeating the full compiler run.
- Existing instructions already called for focused checks and a small queue
  gate. Unconditional wording about pinned qualification, combined with failure
  to enforce scope and stopping conditions, allowed a much larger process.

## Changes to the working process

[Testing policy](../../../../tools/testing.md#merge-validation-budget) now
requires a short, bounded validation plan, including build cost and reusable
results. A run expected to exceed about 15 minutes triggers a scope reassessment
before more work is launched. It is not a waiver of affected checks or a new
approval step.

Full Product, self-hosting, broad runtime and qualification work require an
assigned acceptance that needs them. They do not automatically gate complete
progress PRs. Pinned qualification is conditional rather than a universal
per-PR ceremony. Reviews stay focused on changed meaning and controls; evidence
lives in the existing PR/claim rather than accumulating another framework.

[Landing instructions](../../../../tools/landing.md#mark-ready-and-enqueue-when-the-change-can-merge)
require proceeding to the normal queue when the scoped checks and required
review pass. Incoming main or queue movement alone never starts another local
build. Extended failures stay visible, with their actual identities and scope.

Product native compilation remained unsuccessful at the recorded predecessor.
This landing does not claim full Product, release or cross-host qualification.
That remaining compiler work must have its own concrete acceptance and budget.

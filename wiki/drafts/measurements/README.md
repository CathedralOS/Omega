# Measurements

Summaries of measured runs. A run folder holds a `README.md` that states what was measured, at which
revision, and the headline numbers.

Raw run data does not belong in git: CSV, JSON, logs, per-fixture output, archives, patches and probe
scripts stay out. `tools/check_rules.py` rejects a pull request that adds a file here other than a
`.md` file of about 300 lines or fewer. Commit the summary, then link raw data by a pinned commit or
keep it in the claim note.

Every raw file that earlier runs committed was removed in one commit. Its parent, `6e5131c510b9`, still
holds all of it: browse [wiki/drafts/measurements at that commit](https://github.com/CathedralOS/Omega/tree/6e5131c510b9e6227b4592652c9a5a39e350a970/wiki/drafts/measurements).
Links from the summaries to those files point there. A plain file name in a summary that is not a link
names a file in the run folder at that commit.

## Records without a run folder

- [checked_provider_linear_result_qualification_2026_10_13.json](https://github.com/CathedralOS/Omega/blob/6e5131c510b9e6227b4592652c9a5a39e350a970/wiki/drafts/measurements/checked_provider_linear_result_qualification_2026_10_13.json):
  qualification of linear provider results on `linux_x86_64`, producer `8be2e4cc7d33` on base
  `1b5cc675a718` (baseline `9dd545defbc0` refused with "provider structural result disagrees with its
  boundary requirement"). Native and Terminal agreed on 7 engine fixtures, 5 of 5 source negatives were
  rejected, 40 of 69 broader source fixtures were rejected with 0 lost successes, and an independent
  review approved the tree. It was re-qualified after integrating main.
- [reference_custody_join_cost.json](https://github.com/CathedralOS/Omega/blob/6e5131c510b9e6227b4592652c9a5a39e350a970/wiki/drafts/measurements/reference_custody_join_cost.json):
  compile time of two indexed-array fixtures at 2, 8 and 32 elements on macOS 26.5.1 arm64 (base
  `a195e6cdf066`, 2026-10-05) and Windows AMD64 (base `0b9891fafba9`, 2026-10-12). Compile time stayed near 3.9 s (float-record fixture, 2 to 32 elements) and 1.6 s (reference-array
  fixture, 2 to 8) on macOS; Windows read 14.5 s and 8.0 s. The profile did not show a superlinear custody join.
  The probe also exposed a frame-planning defect that kept a link register live in a call-free body,
  which was repaired; the 32-element reference-array build was refused by that defect.

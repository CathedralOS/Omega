# Repository tools findings

| Atlas finding | Disposition | Evidence |
| --- | --- | --- |
| Area 16 sections 5 and 6. A Git diff failure skipped file hygiene and measurement checks. | Fixed. Admission now returns the Git error when the comparison fails. An empty successful diff is still valid. | On Windows, setting Git’s diff.orderFile to a missing file made Git exit 128 while the old check-rules printed clean and exited 0. The repaired command reports the failing git diff and missing filename and returns nonzero. The fixture `rules::tests::unreadable_diff_cannot_pass_as_an_empty_change_set` compares the same two commits before and after setting the missing order file. |
| The changed-program sweep discarded errors while collecting changed diagnostics. | Previously fixed. | The [area digest](../digests/16-tools.md) records the public CLI regression `changed_compiler_sweep_guards_programs_from_the_invocation_checkout`. |

The rule for reading the contents of an added measurement, the policy on modified measurements, and the remaining area 16 findings still need individual dispositions.

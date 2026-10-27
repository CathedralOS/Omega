# Descriptors stored in arrays of records

A whole-array shared view of four noncopy Entries has length 4; reading its extent satisfies require_four(view.len).

This production belongs to `STRUCTURAL-BORROW-IDENTITY` and the generic fixed-array view requirement of `as_slice_views`: completing and storing a returned descriptor preserves its exact extent and backing identity. The enclosing row remains [generic](../README.md).

The frozen root records a fixed-baseline defect at published revision `7f7ad2cb686b8f3223106859bcf23908c55f03ec`: the exact optimized Linux producer refused the established length at `require_four(holders[1].view.len)`, after two completed views were stored in an array of records. The frozen root source has SHA256 `7f82896ba82dfc08b5c30eab8a73ed055748f45e41ec10f4936bed3a3219ac02`. A single record without the array checked successfully on the same producer.

Each transport place retains the same four-element fact and consuming requirement. Its twin changes only the second backing extent to three and keeps the consuming requirement at four. The positive also checks both original backing addresses and noncopy element values; the match join runs both branches with distinct record padding.

| Place | Positive | Reject twin |
|---|---|---|
| Root | [main.omg](main.omg) | [invalid](invalid/main.omg) |
| Local | [local](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [field](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [parameter](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [call_result](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [match_join](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Constant name | [const_name](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [other_state](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [other_machine](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Target body | [target_body](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Completed calls establish the exact descriptor length. Typed inline projections retain that immutable metadata on the completed value; current storage versions and exact write invalidation govern reuse after mutation. The consuming call proves its entry obligations directly, without a caller equality guard. Ownership and reference authority remain separate from descriptor metadata. Runtime address comparisons exercise checked-to-Terminal lowering, layout, activation storage and native realization on the observed host.

Qualification is in progress. The frozen root passed Linux Terminal Psi and native execution with prototype `b7632bb198f1be66598d566cc9a4fd1ecbabf69e`; this is not a qualification claim for the generated transport row. All places, reject reasons, affected corpus comparisons, independent final review, publication and published-source readback remain open. Four target bindings do not establish matching-host execution on unobserved ARM hosts. Full parent capability acceptance remains open.

# Conditional callback carry

The rules for conditional callback carry live in
[conditional suspension carry](../../spec/language/effects.md#conditional-suspension-carry),
[borrows across suspension](../../spec/resources/carry.md#borrows-across-suspension)
and [conditional suspension-carry summaries](../../spec/terminal-psi/encoding.md#conditional-suspension-carry-summaries);
this note keeps only the acceptance cases that close `STATIC-MACHINE-MIGRATION`
in [TASKS.md](../../../TASKS.md). Delete it when they pass.

## Acceptance

Extend existing generic/carry suites with these six cases and their minimal
reject controls. Use the same helper body and retained resource across each
pair, changing only the selected callback or valid suspension permission.

| Case | Required result |
| --- | --- |
| Strict claim, synchronous callback | Accept; the published contract guarantees no suspension and discharges only the suspension-permission obligation. |
| Strict claim, suspending callback | Reject at application, naming the claim, originating crossing and selected callback. A callback that can park but completes immediately on this run still rejects. |
| Independent suspension | With the same strict claim live, adding an independent suspending call requires permission unconditionally, regardless of a synchronous callback selection. |
| Established permission | Accept a suspending callback when the claim validly establishes `Carry::AcrossSuspend` and all other carry requirements hold. A type-wide suspension prohibition still rejects. |
| Forwarding and multiple callbacks | Compose a call's condition over both callbacks. Test different resource arguments and projected fields, scoped local acquisitions and nested wrappers so evidence for one subject cannot authorize another. |
| Ordinary invariant fence | A synchronous callback still rejects when the call itself exposes an unfinished invariant. Keep outcome and cancellation obligations intact. |

Cover both structural and nominal static callbacks, including existing checked
forwarding summaries. Check interface identity, codec round trips, stale/missing
rows and the fixed runtime-callable contract boundary. Preserve previously valid
cases and pin intentional rejection changes. Execute positive cases with
`omega run --both` across supported hosts; no new test subsystem is required.

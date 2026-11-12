# Context cleanup closure

The generated caller owns a context with a nominal cleanup hook. Its only
CleanupAudit invocation is in that hook, not the borrowed entry. The matching
fail fixture excludes that service and must reject at behavior admission.

The UEFI image emits A on entry and retains the two synthetic roots forever.
This pins lowering and demand retention, not executed normal-return cleanup.

# Borrowed sum repair

Detach a sum through an exclusive loan, transform it, and restore that exact
caller field. Failure retains the token; success consumes it and installs a
fresh token with value 42. The disjoint marker stays 19. Correct execution
returns 0; any wrong observation traps.

From the repository root, with the compiler available as `omega`:

```text
omega update --project samples/cli/basics/borrowed_sum_repair
omega run --both samples/cli/basics/borrowed_sum_repair/main.omg
```

Current frontier: macOS ARM64 native execution returns 0, but Terminal
execution fails with `ClaimTransferMismatch` when `consume(holder.choice)`
joins the repaired `Success` tag to the caller's retained `Failure` claim.
**BORROWED-STORAGE-RESTORATION** owns that loan-completion custody repair.
The body is the original retagging reproduction, not a tag-preserving
substitute; native success is not dual-engine acceptance.

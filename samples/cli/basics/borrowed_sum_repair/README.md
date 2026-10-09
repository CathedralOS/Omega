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

On macOS ARM64, native and Terminal execution both return 0. Normal exclusive
call completion refreshes only the repaired field's caller-local payload
claims, using its stored tag rather than retaining the old `Failure` claim.
The body remains the original retagging reproduction, not a tag-preserving
substitute. Matching-host execution on the other supported targets remains
under **BORROWED-STORAGE-RESTORATION**.

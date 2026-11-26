# Borrowed sum repair

Detach a sum through an exclusive loan, transform it, and restore that exact
caller field. Failure retains the token; success consumes it and installs a
fresh token with value 42. The disjoint marker stays 19. Correct execution
returns 0; any wrong observation traps.

Run the probe from the repository root.

```text
omega packages update --project samples/cli/probes/borrowed_sum_repair
omega run --engine native samples/cli/probes/borrowed_sum_repair/main.omg
omega run --engine psi samples/cli/probes/borrowed_sum_repair/main.omg
```

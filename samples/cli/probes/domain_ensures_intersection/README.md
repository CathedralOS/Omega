# Domain Ensures + Intersection

Compile-scope probe: domain establishment through `ensures` on
ordinary machines, and the intersection requires spelling
`password in Valid & Secure`. No standard-library dependency.

- `mark_valid`/`mark_secure` carry `ensures password in D` — the
  checker proves the body's field writes satisfy the domain clause,
  so the fact is established at the call site.
- `touch_salt` mutates an unrelated Wrapping field in between —
  established domain facts survive unrelated writes.
- `accept` requires the intersection; both facts must still hold.
- `hits` counts each arm reached; 3 proves ordered execution.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/domain_ensures_intersection/main.omg
mbx run -p omega -- update --project samples/cli/probes/domain_ensures_intersection
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

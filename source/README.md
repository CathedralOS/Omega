# Final Omega source

This tree holds the final self-hosted product compiler and the Omega
libraries it uses:

```text
library/  core, allocation, and standard-library packages
psi/      target-neutral source processing through Terminal Psi
omega/    target realization, artifact emission, and product entrypoint
```

`psi/` and `omega/` are one compiler written in Omega, split at Terminal Psi
by the Psi/Omega firewall. The Epsilon-written bootstrap compiler D compiles
them first, and the resulting `omega0` tape compiles them again:

```text
bootstrap/interpreted/5_omega D + source/{psi,omega,library} -> omega0
omega0 + source/{psi,omega,library}            -> omega
```

The trust-minimizing languages and compiler D are in
[`../bootstrap/`](../bootstrap). The maintained Rust implementation under
[`../omega-rust/`](../omega-rust) is a development comparator, not a
language rung or source of authority.

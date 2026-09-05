# Final Omega source

This tree contains the final self-hosted product compiler and the Omega
libraries it consumes:

```text
library/  core, allocation, and standard-library packages
psi/      target-neutral source processing through Terminal Psi
omega/    target realization, artifact emission, and product entrypoint
```

`psi/` and `omega/` are one Omega-written compiler split by the Terminal Psi
ownership firewall. They are compiled first by the Epsilon-written bootstrap
compiler D and then by the resulting `omega0` tape:

```text
bootstrap/omega D + source/{psi,omega,library} -> omega0
omega0 + source/{psi,omega,library}            -> omega
```

The trust-minimizing languages and compiler D live under
[`../bootstrap/`](../bootstrap/). The maintained Rust implementation under
[`../omega-rust/`](../omega-rust/) is a development comparator and is not a
language rung or source of authority.

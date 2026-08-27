# Replaceable on-ramps

This directory contains implementation machinery that helps reach or compare
the canonical semantic owners without acquiring ownership of them.

- [`rust/`](rust/) contains the temporary Rust Psi/Omega product implementation
  and development CLI. The former Rust Alpha, Beta, and Delta producers were
  retired rather than maintained as a parallel boot lattice.
- [`omega-bootstrap/`](omega-bootstrap/) is the temporary Delta-written bridge
  used to perform the first hosted build of the Omega-written product compiler.

On-ramps may remain useful as differential implementations after the hosted
path closes, but no release or proof claim gains authority from their pedigree.

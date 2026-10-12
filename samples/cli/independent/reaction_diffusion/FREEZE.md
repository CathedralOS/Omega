# Pre-execution oracle freeze

Frozen UTC: 2026-10-12T06:03:30.267749+00:00

This freeze precedes all Omega check, update, native and Terminal invocations.
Integer outputs and exits are exact; absolute and relative tolerances are zero.
The reaction-diffusion update is reconstructed provenance (see README): the
original IND10 candidate and frozen Decimal matrix oracles were unrecoverable,
so these fixtures pin the re-derived semantics, not recovered bytes.

Cases: 26

`reference.py` SHA-256: `5232139ade985ed00b925a310ff8bcb0258e666fb3e1fab89876fe4fb1b77918`
`fixtures/oracle.json` SHA-256: `186a83808a2f572022fca48443884ba0a4c48973152683f08b17adb07261767b`

The oracle is an independent Python implementation of the same decimal-thousandth
explicit Euler update; no result is derived from Omega output.
Golden fixture bytes and exits are committed; validation rejects hash changes.

# Qualified alias cases through other machine

Fact: A transparent generic alias retains its underlying case identity and payload type through qualified construction and matching.

The actual imported public leaf producer constructs both cases; its bool twin leaves main.omg byte-identical. The same Some42/Empty17 operation, explicit Choice/Chosen interchange and generic-record contrast remain. The sole twin changes only the producer's Some payload to true and matches the original bool-to-u64 field guard.

Actual [41421752230](https://github.com/CathedralOS/Omega/actions/runs/41421752230), source `9715740cac5f6f3bec28a5d528f6e593f45ee1a8`: sample and corpus positive native0 / Terminal0 / agreement; both intended-invalid observations pass. Linux x86-64 opt1 CLI `daf126ca2d5db4cd66538d2f1324ddaedf43a74d0adaa16fac2ac1a86936ecc2`, runner `17c5a3b5850867acdce3628aa5ad56282a052c35186ad17ac1b98ab94449f9e2`.

No compiler edit or prior-place replay. Raw reports/frozen sources: `/Users/zcanann/Omega-build/alias-qualified-case-final-20261102/`. [Complete row](../README.md) retains exact producer provenance and keeps other hosts and Product open.

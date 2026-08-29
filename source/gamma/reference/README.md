# Gamma executable reference

This directory contains one independent, untrusted evaluator for the current
Gamma reference semantics and one bounded deterministic differential against
`../interp.beta`. It is a diagnostic, not a compiler stage or language owner.

| Retained file | Bounded role | Deletion condition |
| --- | --- | --- |
| `gamma_ref.py` | Independently execute the current ADT, match, recursion, arithmetic, and trap surface. | Delete when a stronger checked Gamma semantic relation subsumes it. |
| `gamma-fuzz-gen.py` | Deterministically generate terminating discriminator programs for the reference comparison. | Delete with the diamond or when fixed spec-derived cases fully subsume its shapes. |
| `gamma-diamond-py.sh` | Compare the reference and canonical Beta interpreter over the bounded corpus. | Delete when the compiler's direct checked semantics covers the same observations. |

The evaluator has no authority and must not become an external runtime stage of
the future Gamma compiler.

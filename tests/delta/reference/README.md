# Delta executable reference

This directory contains one independent, untrusted evaluator for a bounded
subset of the Delta semantics fixed by `../LANGUAGE.md` and one deterministic
differential against `../interp.gamma`. It is a diagnostic, not a compiler stage
or language owner. Agreement cannot establish a rule both evaluators omit.
It is temporary development scaffolding and is deleted once the direct checked
Delta edge subsumes its bounded role; no Python evaluator belongs to the
completed offline bootstrap.

| Retained file | Bounded role | Deletion condition |
| --- | --- | --- |
| `delta_ref.py` | Independently execute the bounded ADT, match, recursion, arithmetic, and trap surface. | Delete when a stronger checked Delta semantic relation subsumes it. |
| `delta-fuzz-gen.py` | Deterministically generate terminating discriminator programs for the reference comparison. | Delete with the diamond or when fixed spec-derived cases fully subsume its shapes. |
| `delta-diamond-py.sh` | Compare the reference and canonical Gamma-written Delta interpreter over the bounded corpus. | Delete when the compiler's direct checked semantics covers the same observations. |

The evaluator has no authority and must not become an external runtime stage of
the future Delta compiler. While retained, it consumes raw bytes and implements
the same textual-ASCII envelope, explicit identifier classes, and CR/LF comment
termination as the Gamma-written oracle it diagnoses.

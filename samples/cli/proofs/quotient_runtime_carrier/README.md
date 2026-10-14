# Quotient copy representatives

Expected native and captured Terminal exit: **70**. The sample constructs an
opaque quotient over a custody-free copy record, then calls checked `define`
and transport-backed `lift` operations through local, field, parameter and
call-result paths. Each observed value is checked before the success exit.

| Path | Chosen representative | Result |
| --- | ---: | ---: |
| Local | 11 | 11 |
| Field | 13 | 13 |
| Parameter | 17 | 17 |
| Call result | 29 | 29 |

`python3 reference.py` computes this table separately, without reading compiler
output. Its finite relation checks illustrate the selected laws; they do not
replace the universally checked Omega theorem bodies.

Run `omega update --project samples/cli/proofs/quotient_runtime_carrier --target linux_x86_64`,
complete the ordinary package review, then run
`omega run --both --keep samples/cli/proofs/quotient_runtime_carrier/main.omg`.
The same source declares the other supported target entries.

The representative relation is exact record equality. Quotient formation does
not expose fields or synthesize equality, serialization, pattern matching or
representative extraction. The public operations retain their mathematical
identity while executing ordinary representative calls and checked nominal
adaptations. Theorem declarations are retained source admission, not an
independently checked PCC theorem proof.

The corpus companion `run/proofs/quotient_copy_paths` also exercises an identity
cast and duplicate unused observers. Invalid controls reject false relations and
result laws, affine/routed carriers, representation observation and selected
representative or theorem bodies with private crashes. Existing managed
proof-only `define` and `lift` fixtures remain separate controls.

# Integer-multiple remainder

Fact: For a u64 count from 1 through 4095, the exact u16 byte count count * 2 + 0 has remainder zero modulo 2 and constructs an EvenLength.

Each place returns 14 at count 7 and binds all four hosted ProgramEntry targets.
Run `omega run --both samples/cli/facts/integer_multiple_remainder/<place>/main.omg`.
The places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Each `invalid/`
twin changes only `OFFSET` from 0 to 1. The input bound, carrier, modulo
obligation, and authored observation remain unchanged; the byte count stays
representable but is no longer even. The same programs and twins are pinned
under `tests/omega/{pass,fail}/facts/integer_multiple_remainder_<place>/`.

The native consumer is Windows relative-name acquisition: a bounded UTF-16
component supplies an exact even UNICODE_STRING byte length. This row isolates
its arithmetic prerequisite without requiring Windows or a foreign binding.
It does not establish Windows filesystem execution.

## Requirements and evidence

| Requirement | What the row exercises |
| --- | --- |
| Establishment | A symbolic bounded count forms an exact product, narrows to u16, and establishes the even-length constructor predicate. No post-hoc guard re-establishes parity. |
| Identity and lifetime | Local values, projected fields, parameters and completed call results preserve their own integer observations. |
| Composition | The same fact crosses all nine places; the match join exercises both arms and target bodies retain identical arithmetic. |
| Soundness boundaries | OFFSET 1 rejects at the unchanged consuming obligation. Arithmetic fixtures separately keep overflowing products, signed MIN/-1, zero divisors, stale writes, wrapping and saturation from supplying exact-product laws. |
| Checked to Terminal Psi, Psi 05–07 | Checked arithmetic, construction, call/return and state edges erase to Terminal Psi; `--both` executes that artifact with the same authored result. |
| Provider settlement, geometry and ABI, Omega 00–02 | These source-only programs require no foreign provider. Scalar u16/u64 values and the record result use ordinary layout and calling plans. |
| Selection, register homes and frames, Omega 03–06 | Runtime products, casts, remainder comparisons, calls and state arguments use the native integer path and preserve the observed result. |
| Encoding and native emission, Omega 07–09 | Host image execution must exit 0 and agree with Terminal execution; foreign target entry declarations are not claimed as foreign-host execution. |

The implementation normalizes only polynomials whose every integer coefficient
is divisible by one nonzero constant. It does not claim general integer-valued
polynomial reasoning, arbitrary runtime-divisor cancellation, or permission to
erase intermediate overflow and definedness obligations. Windows and Linux
runtime acceptance for the parent filesystem task remains open.

# NF02 numeric accuracy policy

The two complete consumer projects import the shared `numerics` package directly through exact declaration imports.
`classification` covers finite/nonfinite classification, conversion, signed zero, and
invalid tolerance precedence. `error_budget` covers symmetric-any and reference-additive
comparisons, overflow, subnormal rounding, and asymmetric reference scaling.

`EXPECTATIONS.txt` freezes the mathematical results. The original worker source and
validation are retained in `evidence/README_original_handoff.md` and companion logs;
they are historical receipts.

The separate `invalid_suffix` project must reject an explicitly binary64 literal
assigned to binary32. Runtime `InvalidTolerance` values in admitted programs are a
separate error boundary. Rebase and qualification on the later provider-plan
changes, exact-candidate landing verification, and normal publication remain
pending.

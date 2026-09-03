# Gamma evaluator

This owner contains trusted Beta source for the Gamma evaluator. The admitted
Beta compiler translates it to Alpha tape, making this the Beta-to-Gamma
compiler edge rather than optional reconstruction tooling.

The evaluator implements exact-ended request loading, source-envelope validation,
bounded definition rows, exact name lookup, hexadecimal words, stack operations,
fixed cells, sealed input, append-only byte/word output, wrapping arithmetic,
signed division/comparison, ordinary calls, explicit tail `jump`/`branch`, and
terminal status classes. Every storage region is fixed and checked.

| Retained file | Role | Deletion condition |
| --- | --- | --- |
| `gamma_evaluator.beta` | Beta-authored source for the executable Gamma evaluator. | Replace only with a smaller checked evaluator and synchronized Gamma gates. |
# Matching floating-point values

This compile-only sample compares record fields and a computed sum with two runtime parameters. The first equal value selects `7` or `9`, and the fallback selects `11`. The subjectless `transition` spells out each equality. A bare name in a subject pattern would introduce an arm-local binding.

```sh
mbx run -p omega -- --check samples/cli/probes/match_bound_patterns/main.omg
mbx run -p omega -- packages update --project samples/cli/probes/match_bound_patterns
```

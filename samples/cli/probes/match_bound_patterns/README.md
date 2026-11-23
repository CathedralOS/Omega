# Matching floating-point values

This sample compares record fields and a computed sum with two runtime parameters. The first equal value selects `7` or `9`, and the fallback selects `11`. The subjectless `transition` spells out each equality. A bare name in a subject pattern would introduce an arm-local binding.

Check the sample from the repository root.

```sh
mbx run -p omega -- --check samples/cli/probes/match_bound_patterns/main.omg
```

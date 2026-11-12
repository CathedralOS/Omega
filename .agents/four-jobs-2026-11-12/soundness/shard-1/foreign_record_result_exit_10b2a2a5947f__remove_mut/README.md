# Foreign record results

On macOS ARM64, after ordinary package preparation:

```text
omega run tests/omega/pass/providers/foreign_record_result_exit/main.omg
```

The program must exit 70. It calls the host C library's `div(int, int)`, whose
`div_t` result contains quotient followed by remainder as two `int` fields.
It retains two independent results and checks both fields, including negative
values. The foreign calling policy owns the record's ABI classification; the
compiler must not treat it as an interchangeable scalar of the same byte width.

The isolated native regression, including test package admissions, is:

```text
python3 tools/corpus_gate.py --native --filter providers/foreign_record_result_exit
```

An ordinary CLI invocation without package review is expected to stop at review;
the regression harness does not grant admissions to that project.

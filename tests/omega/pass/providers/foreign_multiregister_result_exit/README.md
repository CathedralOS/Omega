# Multiple-register foreign results

After ordinary package preparation on macOS ARM64:

```text
omega run tests/omega/pass/providers/foreign_multiregister_result_exit/main.omg
```

The program must exit 70. Two libc `lldiv` calls return records containing
64-bit quotient and remainder fields. Each result spans two ABI registers;
both complete records must survive the later call. Quotients exceed 32 bits,
and the negative input checks signed transport.

The isolated native regression supplies test package admissions:

```text
python3 tools/corpus_gate.py --native --filter providers/foreign_multiregister_result_exit
```

The harness grants no package admission to an ordinary CLI project.

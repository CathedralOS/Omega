# Nested and sibling record receivers

The scalar expression observes two independently constructed records; one contains
another computed observation and a scalar call in an unread field. Each complete
receiver precedes its observation. The result is `left ^ left`, or zero, and the
program exits 70 without output.

```text
python3 tools/corpus_gate.py --both --filter expressions/runtime_nested_record_field_reads_exit
```

The stronger `arithmetic/runtime_nested_record_observation_exit` retains Exact
division in both receivers and tracks the separate checker-discharge dependency.

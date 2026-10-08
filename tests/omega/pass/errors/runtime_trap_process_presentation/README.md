# Typed trap process presentation

Run the actual product command from the repository root:

```text
omega run --both tests/omega/pass/errors/runtime_trap_process_presentation/main.omg
```

The native child executes the architectural crash leaf and the same retained
Terminal program reports typed `Trap`. The CLI must report host-presentation
agreement, not decline or numeric exit agreement. The command preserves the
native child's termination policy; agreement does not turn this crash into a
successful process exit. On macOS ARM64 the native observation is signal 5 and
the CLI exits 255.

The corpus gate independently checks the authored trap oracle:

```text
python3 tools/corpus_gate.py --both --filter errors/runtime_trap_process_presentation
```

The host observer's unit controls refuse segmentation faults, absent status,
ordinary exits and an unmapped `Abort`. Agreement compares only the host's
observable termination, not crash location, stdout, cleanup or full traces.
On Windows an explicitly supplied exit status can have the same bits as the
illegal-instruction exception; this channel cannot prove which caused it.

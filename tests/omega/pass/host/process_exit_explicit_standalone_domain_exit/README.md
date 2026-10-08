# Explicit standalone domain exit

From the repository root on macOS arm64, after ordinary package review:

```sh
omega run --both tests/omega/pass/host/process_exit_explicit_standalone_domain_exit/main.omg
```

Both engines exit 70 through the exact canonical ProcessExit requirement.
The program has neither Console reach nor an erased service field. The standalone
host explicitly delegates death of its own fresh isolated domain; importing core,
selecting std's provider or establishing a Binding does not grant that permission.
The runtime request defaults to no domain-exit delegation for embeddings. The
core's actual admission still binds one program, instance and exact requirement,
never the embedding host or another instance.

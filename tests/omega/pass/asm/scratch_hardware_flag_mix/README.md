# Scratch arithmetic and flags observation

This is the unchanged source previously rejected as a scratch/hardware-flag
composition. It subtracts 1 from 2, stores output 1, and deliberately takes its
`Trap` branch because its authored runtime guard asks for output 0.

The corpus oracle is `trap`, not a numeric exit: `tools/corpus_gate.py` checks
Linux x86-64 native `exit:signal signal:4` and the Terminal engine's typed
`interpreter:trapped`. The actual development product command
`omega run --both .../main.omg` reported native signal 4 and
`interp crash: Trap (host presentation agrees)`; the CLI returned 255.
Source acceptance does not change that authored behavior.

Development evidence: frozen optimized producer `producer-build12`, SHA-256
`b69bc7ed0782549a44a88dbd7244de91b2ea8f2a5efccf5c28e928d83b41bf74`,
Linux x86-64, 7.442 seconds. Invocation-local exact commands, source digest and
outputs are retained at `build/slot7/producer-build12/additional-host/`.
This is development execution evidence, not final qualification.

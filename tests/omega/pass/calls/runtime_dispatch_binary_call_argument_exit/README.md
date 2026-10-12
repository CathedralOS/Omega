# Guarded state arrival and call argument

The four visits to `carve_at` must leave tag 103 in room two and exit 70.
The binary expression is stored before being passed to `carve`; state re-entry
must retain the guard's fact about `next` without reevaluating the tag's
initializer. The former `index: u64 [0..=3]` is now an ordinary `u64` with
`requires index <= 3`. This is an arrival obligation, not a nominal type.

```sh
omega run --both tests/omega/pass/calls/runtime_dispatch_binary_call_argument_exit/main.omg
python3 tools/corpus_gate.py --both --filter calls/runtime_dispatch_binary_call_argument_exit
python3 tools/corpus_gate.py --filter calls/state_arrival_call_result_above_bound
```

The public command requires normal local package and entry review. The corpus
owns its reviewed test invocation; no broad acceptance policy is included.
The invalid control supplies four through a result guarantee and must reject
the state's requirement rather than treating the contract as type decoration.

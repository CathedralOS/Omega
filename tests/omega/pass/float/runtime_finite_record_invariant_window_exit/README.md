Run `omega run --both tests/omega/pass/float/runtime_finite_record_invariant_window_exit/main.omg`; native and checked execution must exit 0.

The constructor proves its selected field's finite first value. Inside the
exclusive invariant window, a temporary NaN may inhabit the ordinary f32 carrier;
the subsequent repair restores finiteness before borrowing the record to `read`.
Storage eligibility must neither erase that consumption obligation nor impose a
new per-store invariant that forbids a repaired window.

Shared Terminal execution still needs the `TERMINAL-RUNTIME-DRIVERS` host-import
adapter for `Float::divide`. `python3 tools/corpus_gate.py --both --golden tests/omega/corpus_outcomes.txt --filter runtime_finite_record_invariant_window_exit`
reports that dependency without rewriting the NaN-producing operation.

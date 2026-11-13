# Projected write-only state loans

Run `cargo run -p omega -- run --both tests/omega/pass/borrow/write_only_composed_forward_exit/main.omg` from the repository root.

The same record loan crosses either branch's state edge before lending a field
to an ordinary helper. Two calls select opposite fields, storing 42 and -31.
Exit **70** checks both updates in the original caller record; exit 71 reports
incorrect writeback or a clobbered sibling. Neither callee observes the loan.

The corpus execution gate is `python3 tools/corpus_gate.py --both --filter borrow/write_only_composed_forward_exit`.
It checks native and interpreter agreement. The public command additionally
requires ordinary package review; this fixture does not authorize bypassing it.

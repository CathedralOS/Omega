# Topological order (source removal)

Kahn-style topological order over a directed graph held in CSR form
(`offsets: [u64; 9]` + `targets: [u64; 16]`). Instead of a
FIFO queue, each pass scans all nodes and emits every node whose in-degree
reached zero, decrementing its successors immediately — so one pass can unlock
later-scanned nodes and edges running high-to-low force multiple passes.

The program runs two graphs:

- a 9-edge DAG over 8 nodes whose answer needs four passes and emits
  `[0, 5, 2, 4, 1, 6, 3, 7]` — verified by rebuilding a position table and
  checking `pos[u] < pos[v]` for every edge, plus an elementwise comparison
  against the hand-computed order;
- a 4-node graph with the 3-cycle `0->1->2->0` plus one isolated node, which
  must emit exactly node 3 and report `1 < 4` (cyclic remainder detected).

On agreement it prints `PASS` and exits `0`.

An intended-invalid control in `invalid/` marks node `8` (violating
`node < 8`) as emitted and is rejected at compile time
(`cannot prove requires contract for call mark_emitted from Main::main`).

After ordinary package review on macOS arm64:

```sh
python3 tools/sample_native_gate.py --cli build/dev-target/debug/omega --target macos_arm64 --filter algorithms/topological_order --jobs 1
build/dev-target/debug/omega run --both samples/cli/algorithms/topological_order/main.omg < /dev/null
```

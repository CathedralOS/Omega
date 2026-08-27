# Conservative CKIR15 recurrent-view backend implementation note

[`CKIR15`](OMEGA_BOOTSTRAP_CHECKED_IR_V15.md) inherits CKIR14 and replaces the
single CKIR12 synthetic nonempty edge with at least two independently guarded
occurrences. The shared checked-IR backend accepts schema major 15 without
changing any accepted older identity. Major 14 continues to require selected
full-width arithmetic; major 15 permits that complete arithmetic family but
does not require it. No later major is admitted by this implementation.

The backend validates the complete carrier before frame sizing or emission.
For each synthetic block it independently checks the exact leading view and
ordered direct-parameter vector, unique authored true predecessor, guard/source
identity, exact false-edge pass vector, head-then-tail operation order, and the
complete authored jump vector. It also checks that all partial slice operations
belong to synthetic blocks, synthetic blocks follow authored blocks in source
order, and every target type and arity is exact. A malformed carrier therefore
publishes no partial artifact.

## Conservative selection

The existing generic edge emitter remains the only edge implementation. It
copies each already validated argument into the target's private frame slots in
encoded order. CKIR15 adds no specialized pass-through or recurrence emitter:
the stronger validator proves that the branch true vector is exactly `(v,P...)`,
the false vector is exactly `P`, and the synthetic jump is the exact authored
interleaving of `P`, head, and tail.

The inherited operation templates remain unchanged:

- `StaticByteView`, when present, materializes one private 16-byte descriptor
  over the checked read-only literal image;
- `SliceNonEmpty` tests only that descriptor's length;
- `SliceHead` checks nonemptiness before loading one byte; and
- `SliceTailOne` checks nonemptiness before deriving the pointer-plus-one,
  length-minus-one descriptor.

Every true edge copies its source view and pass vector once, executes one head
and one tail in that order, then uses generic edge copying for the complete
authored target vector. Every false edge bypasses both partial templates.
Recurrent execution receives the previously produced tail through the same
ordinary target slots; pass-through values are copied, not recomputed.

CKIR15 can omit `StaticByteView` and all constants when the exact view is a
runtime machine parameter. It can also omit arithmetic. When Add, Subtract, or
Multiply is present, the inherited CKIR14 full-width type identity, recursive
custody, operation order, unsigned trap predicates, and delayed result store
remain mandatory and use the unchanged CKIR14 templates.

Full-width semantic-word decoding does not make every ordinary `u32` row the
selected arithmetic carrier. In particular, the canonical inherited plain-u32
position may retain its bounded `0..=0x7fffffff` row. The backend accepts that
valid row and other independently valid semantic endpoints; arithmetic custody
still recognizes only the separate exact `(u32, Trapping, 0, 0xffffffff)` row.
This distinction is exercised by the real resolver/lowerer cross-pair rather
than inferred from the handcrafted arithmetic fixture.

## Failure and evidence boundary

Unknown identity, fewer than two synthetic blocks, authored/synthetic ordering,
owner or predecessor mismatch, computed/duplicated/reordered pass values,
head/tail omission or reorder, partial operations outside a synthetic block,
target type/arity mismatch, and cross-version relabeling select status 251.
Declared component or literal-image exhaustion selects 252. Neither status
publishes ELF bytes.

Backend evidence covers deterministic recurrent two-byte, one-byte, empty,
runtime-parameter/no-static-root, and optional-arithmetic carriers; exact
native/self artifact identity; one actual resolver/lowerer-produced recurrent
carrier with its bounded ordinary-u32 canonical row; both emitted head/tail
templates; reference
execution of the recurrent, one-byte, and empty paths; local status-251 and
status-252 controls; artifact mutation rejection; and saved CKIR14/CKIR12
backend regressions. The artifact recognizer observes emitted operation
templates and literal bytes, while the Delta backend's independent structural
validator owns vector identity. This avoids duplicating a second incomplete
edge decoder in the artifact fixture.

This note grants no mutable view, dynamic indexing, computed or effectful
pass-through, `u64` collection operation, provider call, public slice ABI,
allocation, optimizer permission, `reaches`, or termination-ranking claim.

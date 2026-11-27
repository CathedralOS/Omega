# Parked native Psi rule engine

These files are native stage 03's former Psi optimization engine: the pass
manager loop, the rule catalog (`PSI_PASS_CATALOG` in `rules.rs`), the
analyses the rules declared, and each rule (sparse conditional constant
propagation, control-flow cleanup, copy propagation, global value numbering,
proof-check elision, dead pure scalar elimination, state and representation
specialization, loop-invariant scalar motion). No build could reach them:
native lowering receives only post-Terminal selections
(`PostTerminalOptimizationSelections` refuses every Psi-phase optimization),
so stage 03's Psi selection was always empty and it always took the identity
path.

The Psi phase runs before Terminal publication
([optimization ordering](../../../../../../wiki/spec/terminal-psi/product.md#optimization-ordering)),
and every optimization has exactly one phase owner
([exact selection](../../../../../../wiki/spec/build/optimizations.md#exact-selection)).
That owner is source stage 06,
[`publication/optimization.rs`](../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/publication/optimization.rs).
It implements `SparseConditionalConstantPropagation` and refuses the other
Psi rules as not supported yet. Migrate rules there one at a time, over
Terminal Psi rather than the abstract-operations unit, with their
positive/negative tests.

`mod.rs`, `pass_manager.rs`, `publication.rs`, `validation.rs` and
`verified_unit.rs` here are the full files as they stood at `079e25a8735569a47488b13c39b20d26219efcf2`; the
active stage keeps the identity-run, publication, context-admission,
projection and manifest parts of them. The rest of the files moved here whole,
with their tests (`control_flow_analyses_tests.rs` and the test modules
inside each rule file, plus the whole-unit validation tests at the end of
`verified_unit.rs`). The rewrite and whole-unit validators they used are in
[`optimization-unit-semantics`](../../representations/abstract-operations/optimization-unit-semantics/README.md).

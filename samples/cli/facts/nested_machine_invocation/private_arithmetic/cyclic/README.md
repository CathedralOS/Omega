# Cyclic private arithmetic

Every cyclic private arrival carries `depth >= 1`, so `consume` can form
`depth - 1`. This is the recurring-entry part of
`NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../TASKS.md).

The [root witness](main.omg) reduces the directory-stack decrement in the
standard library: an explicit guard precedes the nested helper's exact
subtraction, and its terminal backedge revisits that guard. The caller observes
seven iterations, the zero fallback, completion writes and an untouched canary.
The [reject twin](invalid/main.omg) permits zero at the same guard.

Current optimized producer `14a211b80ef82c2d89dfcabbd4b5fdc9d44bb7b6`
refuses both at `consume`'s assignment with `exact arithmetic ... may overflow
u64`. The positive is a compiler defect; the twin's refusal must remain.
The source and oracle are frozen before compiler edits. No helper precondition
has been added. Native/Terminal acceptance and the nine transport places remain
unfinished; this root witness alone does not close the fact or parent task.

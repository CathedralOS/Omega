# Retained mutable foreign backing

Parent: **NORMALIZED-ABI-LOWERING** in `TASKS.md`.

A mutable foreign registration retains exclusive access to the exact original Buffer backing until completion consumes Pending; completion writes 73 while an earlier copied value remains 37.

Current main `29f58e13cc0` rejects the existing mutable control at source checking: retained lifetime-bound custody admits shared access only. The new local witness uses an actual C provider that writes 41 on registration and 73 on completion. Its reject twin reads the original cell while the exclusive registration remains live.

This row is under implementation; no nine-place or hosted execution acceptance is claimed. Normal call restoration does not release the retained exclusive loan. Completion must consume the exact Pending before original access returns; copied scalar values keep their captured meaning.

This complete control intentionally fails proof checking. Invoke `omega --check
--target linux_x86_64 rejected.omg` from this directory (after preparing its
local std dependency normally). Its unconstrained index must not be admitted.
The root filename deliberately differs from `main.omg`, keeping an expected
rejection out of the runnable sample discovery gate. `build.omg` binds the
ordinary program-entry slot to the root's `main` machine.

# Suspension and a token domain

The `Sleeper` boundary trait declares a suspending operation. The caller keeps its token across the call, then removes its `AcrossSuspend` domain before returning a plain token. The domain proves that the token ID is positive.

Run `omega --check main.omg` from this directory. The entry records a hit without calling the suspending operation.

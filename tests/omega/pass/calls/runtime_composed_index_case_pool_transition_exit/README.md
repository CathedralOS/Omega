# Bind a complete selector without replaying unrelated arguments

This variation uses `next + key` for the same 8192-slot operation. The separate `key <= 8191 - next` premise proves addition safety. The payload comes from an effectful helper that is not an input to the selector, and all three calls must execute, including the occupied-slot fallback. The program checks the same payload, fallback, Empty-neighbor and surrounding-canary results plus the helper call count.

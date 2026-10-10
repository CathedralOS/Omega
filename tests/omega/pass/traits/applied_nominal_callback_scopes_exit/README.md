Two applications of one nominal visitor requirement have distinct callback
carriers. Both callbacks inspect their own record and return true. Preparing or
specializing either application must not change the other or the declaration.

Run: `omega run --both main.omg` (native and Terminal exit 0).

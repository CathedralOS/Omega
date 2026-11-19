# Portable bootstrap checks

Copy this directory with the winning chain. Run the checks from any working directory with Python 3 and the executable under test. The fixtures, expected results and request framing travel together.

| Check | Command | Observations |
| --- | --- | --- |
| Gamma evaluator | `python3 check.py gamma /path/to/evaluator` | Framed source and stdin, exit status, output bytes, traps and malformed-source refusals. |
| Epsilon evaluator | `python3 check.py epsilon /path/to/evaluator /path/to/packed-evaluator.delta` | Exit, stdin, trap, refusal and malformed-envelope bytes. The request includes the digest of the supplied closure. |
| Omega program | `python3 check.py omega /path/to/compile-and-run` | The command receives the source path as its last argument and executes it. The program must exit 42 with empty stdout and stderr. |

A route supplies the adapter for the interface it implements. The compiled routes also use these vectors in their differential checks. The sources in `omega/` exercise the bootstrap compiler through its existing entries. The Epsilon entry and Omega inputs retain their published byte identities.

Construction, seed audits, rebuild equality and route-specific limits live with the candidate. The portable checks exercise observable behavior.

At revision `7e358753723`, the compiled Epsilon evaluator refuses the specified `@[u8]` Console declaration at the `@` in the exit case. The check retains the expected exit and output so a future run can distinguish that refusal from a working implementation.

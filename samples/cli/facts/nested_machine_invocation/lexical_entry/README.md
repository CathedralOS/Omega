# Lexical entry selection

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md).

Every call to entry names the visible machine and supplies all three declared parameters.

The visible entry helper has three parameters; its sibling next has two.
The compiler-generated body coordinate is not an authored declaration.
The twin removes only the third actual argument. Runtime probes distinguish
7/11 and 9/13, and observe the explicitly supplied nonce 17.

Optimized exact baseline `70d5cda38eda5688b811050f86a3938b50e3c553`
rejects all nine positives with `entry expects 2 argument(s), got 3`, and
wrongly accepts all nine missing-argument twins. This is an invocation-identity
error: the current helper has two parameters and its generated body entry must
not shadow the visible three-parameter source helper. End-to-end repair
acceptance remains pending.

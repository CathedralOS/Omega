# Ranked calls through a subordinate state

Run the complete hosted customer from the repository root:

```sh
omega run --both tests/omega/pass/termination/rank_range_state_call/main.omg
```

Both native execution and the checked interpreter must exit **0**. The hosted
wrapper checks the returned value; it does not replace the ranked algorithm.

`Main::main` enters the `count → hold → step → count` cycle with rank 5.
`count → hold` and `hold → step` preserve the rank; `step → count` decreases it.
The subordinate state's `pending` parameter carries the entry's `remaining`
subject. Each edge explicitly forwards the borrowed `Main` context. The result
is 0, and an incorrect result makes the wrapper exit 1.

This pins an unranked entry leading into a ranked component, ordinary borrowed
state parameters, weak and strict rank transport, and a scalar result returned
to an ordinary caller. A check-only result is not execution evidence. Removing
the context argument or replacing the component with a self-loop changes the
behavior under test.

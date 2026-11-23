# Task activation declarations

The worker keeps a local token across a call to the selected `Sleeper::park` operation. A separate machine uses `suspend block TaskLifecycle::finish` to consume a linear task and return its outcome. The entry does not start a task or invoke a runtime provider.

Run `omega --check main.omg` from this directory to check the declarations.

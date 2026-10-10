The public selected requirement guarantees that its readonly scalar-reference
result equals the input observed at invocation. Direct, nested and builtin
addition initializers preserve that same completed value. The caller proves
three identity Requires from the public contract, then main checks result 4.
The concrete implementation does not strengthen the selected interface.

`omega run --both main.omg` must exit 0 in native and Terminal execution.
The original ignored source bytes are retained (SHA256
5037cacbfa97515a275e55eb9680dc1af87d1e6f6079df3ee84a159e5aaa0d50).
Frozen 7e8e991e0662 with the matching library tree and adjacent Build bindings
executes native 0 and Terminal 0. Changed input and private-only guarantee
controls remain independent invalid judgments.

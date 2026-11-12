# Let of a matched mutable window

A `let` binds the match expression. Each arm is a mutable window of length
2. The bound window is then used as a local, stored in a record field,
passed as a parameter, and produced as a call result inside the match.

Native and Terminal execution both exit 0.

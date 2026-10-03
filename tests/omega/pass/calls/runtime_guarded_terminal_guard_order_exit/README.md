# Guard calls before terminal operand evaluation

Run `omega run --both main.omg`; both engines must exit 0.

Each guard increments the shared counter by 10 before selecting an arm. The
first invocation skips the arm with effectful operands. The second evaluates
the guard and then its two operands, delivering 21 followed by 22 to the terminal
call. The final counter must be 22. This checks guard execution once per
invocation, selected-arm isolation, and left-to-right operand evaluation.

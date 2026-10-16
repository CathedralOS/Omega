# Checked assembly label edges

Run `omega run main.omg` on Linux x86-64. Expected exit: 0.

The assembly body copies an ordinary value, jumps over a conflicting write,
and proves the destination label's contract. This is the first runnable
control-flow case needed by the interrupt acknowledgement and secondary
processor completion bodies. Label contracts must be checked on actual edges;
ordinary source variables carry communication across the body boundary.

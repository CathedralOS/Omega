# Nominal cleanup after selected transfer

An owner transferred by an executed Boolean operand drops in that call. A
skipped owner stays alive until normal completion, behind owners declared later.
The byte output distinguishes `ACB` from `CBA`; cleanup at the selection join
would produce the wrong order.

The program covers `&&`, `||`, nested selection, branch-only construction, and a
mutable condition overwritten after its value selected the call. The final two
cases put selection inside an argument schedule: the subsequent mutation must
run once, and linear call results, nested construction and forwarded claims
must retain distinct custody through the continuation. A selected linear owner
also retains its exact cleanup claim on the skipped path. Envelope children are
explicitly extracted; each resulting linear local has an eligible pure drop.
Both native execution and the Terminal interpreter must match the authored
stdout and exit zero.

# Shared case selection with a waived Atomic payload

After `omega update --project . --dangerously-accept-all`,
`omega run --both main.omg` must report native and interpreter exit 70.
A shared case dispatch waives the Atomic field and reads only the ordinary
scalar payload. Selecting the case must not introduce an ordinary Atomic
snapshot or require exclusive backing for that unused field.

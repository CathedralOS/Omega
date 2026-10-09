# Runtime-indexed case payload forwarding

After `omega update --project . --dangerously-accept-all`,
`omega run --both main.omg` must report native and interpreter exit 70.
The second array element selects the payload-bearing arm. Its snapshots are
forwarded before the successor changes the selector and replaces that element.
The dispatch's temporary shared loan must end before those writes.

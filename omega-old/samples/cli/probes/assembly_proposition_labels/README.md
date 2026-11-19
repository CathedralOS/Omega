# Assembly proposition labels

Expected native and Terminal exit: 0. Run `omega run main.omg` on Linux x86-64.

Label and whole-body assembly assertions use ordinary propositions. This probe
copies a value through fields and a local, follows a forward label edge, and
checks transparent aliases against the values visible at each assertion.
An opaque proposition helper checks preservation of an authored caller premise.
No privileged instructions or hardware I/O occur.

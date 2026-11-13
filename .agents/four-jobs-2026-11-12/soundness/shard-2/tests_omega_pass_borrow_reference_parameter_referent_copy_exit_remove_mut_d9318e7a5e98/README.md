# Copy through a borrowed parameter

Run `omega run --both main.omg`. Both engines must exit 0.

The destination parameter lends mutable access to its scalar referent. Shared
sources arrive through a projected field, a retained local and a helper result.
Both original source values stay intact. Local and projected shared descriptor
slots remain replaceable without mutating either pointee.

Returned mutable aliases also write a nested signed-integer field and an
ordinary Boolean field. Both the alias reads and the original aggregate fields
must contain the authored replacements, preserving their declared storage.

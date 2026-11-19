# Schema descriptions

This crate carries immutable declaration, field, case and nominal-reference
descriptions. Its decoder checks the retained graph's shape and its codec keeps
the existing runtime image and revision framing.

Source settlement constructs descriptions using the requester's visibility.
Descriptions grant no authority to inspect another declaration, call a machine
or access storage. Consumers receive the description itself, never a source
lookup or a mutable typed-program table.

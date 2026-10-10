# Returned view through a captured-context callback

Run `omega run --both tests/omega/pending/generics/contextual_static_machine_captured_operand_return_exit/main.omg`.
Both engines must return 0. The callback returns its independently fresh
item, not the caller's captured Holder reference. Region formation is not
enough: declaration-dependent Context frontiers must join instantiated call
loans, and the specialized consumer's result must retain that source relation.
The consumer explicitly relates its result to the item: an elided result would
be ambiguous after Context instantiates to a borrow-carrying Holder.

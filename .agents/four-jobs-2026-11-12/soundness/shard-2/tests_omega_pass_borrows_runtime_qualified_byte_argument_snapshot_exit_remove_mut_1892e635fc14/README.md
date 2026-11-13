The earlier owned argument captures the qualified byte buffer's current contents
and live length. A later argument replaces its backing with shorter Unicode text.
The callee sees the original ASCII text; the caller sees the replacement and an
untouched sibling. The declared contract is UTF-8, not the initializer's stronger
ASCII property.

Run `omega run --both tests/omega/pass/borrows/runtime_qualified_byte_argument_snapshot_exit/main.omg`
after preparing the fixture's package inputs. Both engines must exit 70.

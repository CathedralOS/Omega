Owned byte arguments consume their exact completed value, including its declared
UTF-8 qualification. The same call accepts ASCII, Unicode and empty literals,
an established concatenation and a returned value while preserving a sibling.
Literal presentation must not bypass the value's scheduled producer.

Run `omega run --both tests/omega/pass/borrows/owned_byte_value_call_exit/main.omg`
after preparing the fixture's package inputs. Both engines must exit 70.

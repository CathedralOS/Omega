Eight readable record references and a ninth mutable record reference exceed
the ordinary AArch64 argument-register bank. The caller must observe both
field writes through the returned reference and, after that reference's last
observation, through the original owner. The readable neighbors remain
unchanged. The first argument is a retained local descriptor; the returned
reference's explicit lifetime connects it to the ninth argument.

Checking only the callee's result would allow a copied destination to pass.
Checking both owner fields after return rules out that substitute. The ninth
argument also prevents register-only native coverage from standing in for
stack reference transport.

Run: `omega run --both tests/omega/pass/borrows/borrowed_record_stack_parameter_writeback_exit/main.omg`.

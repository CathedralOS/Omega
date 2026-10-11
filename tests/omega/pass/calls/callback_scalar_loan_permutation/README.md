# Scalar-loan callback permutation

After accepting the generated package review, run `omega run tests/omega/pass/calls/callback_scalar_loan_permutation/main.omg`.

Darwin qsort_r supplies the context before its two element pointers. The authored
callback takes the first element, context loan, then second element: native
registers X1/X0/X2 must become internal descriptor pointers X0/X1/X2.
All three scalar loans remain backed by their original storage. An ordinary
helper compares the elements; the context selects ascending then descending
sorts. Both authored outcomes must hold and the program exits 0, otherwise 71.

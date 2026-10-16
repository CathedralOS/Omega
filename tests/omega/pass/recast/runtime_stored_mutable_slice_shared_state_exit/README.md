# Stored mutable slice presented to shared states

A mutable recast descriptor lends its original backing to a shared parameter,
which forwards the same pointer and element count to another state. Both
execution engines return 70 after checking the two element values. The stored
access ceiling permits shared observation without granting another mutable loan.

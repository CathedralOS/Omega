# Pass a checker together with the helper it promises

A generic wrapper accepts a Boolean helper and a checker that promises to use it. The checker confirms that a referenced field equals 37, so the wrapper's caller can safely subtract 37. A different input takes the other branch and returns 91. The program checks both results.

The compiler previously lost the selected helper while checking the wrapper's arguments and copying the checker's guarantee. It now keeps the complete selection, including configured constants, and prepares all helpers before copying guarantees that refer to them. The original function bounds and exact named implementation still require checking.

The [wrong-helper control](../../../fail/generics/dependent_checker_wrong_helper/main.omg) supplies a different helper with the same signature while keeping the checker unchanged. It must reject the mismatched promise.

The [configured-helper program](../dependent_checker_configured_exit/main.omg) uses constants 37 and 91. The [reversed-order program](../dependent_checker_checker_first_exit/main.omg) declares the checker before its helper. The [borrowed-input program](../dependent_checker_borrowed_exit/main.omg) keeps the same distinction with lifetime-bound references. Each checks both results at runtime. The [wrong-constant control](../../../fail/generics/dependent_checker_wrong_constant/main.omg) changes only the second supplied constant and must reject that checker; the first pairing stays valid.

This repairs the generic validator needed by Cathedral's interrupt-table installer. The installer and repeated hardware timer delivery remain unfinished.

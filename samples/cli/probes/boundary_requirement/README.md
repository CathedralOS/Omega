# boundary requirement

`ReceiptSettlement::settle` is a one-member boundary trait with an empty reach. The caller matches `ReceiptState`. Empty carries no claim. Live passes the linear receipt to the trait, and the selected `abs` provider returns 4 for code -4.

Expected exit: 0.

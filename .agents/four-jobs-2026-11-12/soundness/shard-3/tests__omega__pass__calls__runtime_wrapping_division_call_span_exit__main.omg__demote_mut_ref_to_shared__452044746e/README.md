# Division across a Console call

The complete iterative formatter retains loop-carried unsigned values across
division, remainder and Console writes. x86 division inputs need instruction-local
fixed-register transport; an original live value cannot remain pinned to a
register clobbered by the intervening provider. Frozen output is `288` plus LF;
exit is 70.

/* Harness bring-up only: NOT compiler-emitted Omega code or ack authority. */
.text
.code64
.global control
control:
 pushq %rax
#ifdef DEBUG_MARKER
 movb $'T', %al
 outb %al, $0xe9
#else
 movq $0x300000, %rax
 incq (%rax)
#endif
 popq %rax
#ifdef CLOBBER_GPR
 movq $0xdead, %rax
#endif
#ifdef WRONG_RETURN
 retq
#else
 iretq
#endif

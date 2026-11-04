#include "boot.inc"
 /* Install the exact dedicated interrupt stack in TSS.IST1. */
 leaq irq_stack_top(%rip), %rax
 movq %rax, tss+36(%rip)
 movw $104, tss+102(%rip)
 leaq tss(%rip), %rax
 movw $103, gdt+24(%rip)
 movw %ax, gdt+26(%rip)
 shrq $16, %rax
 movb %al, gdt+28(%rip)
 movb $0x89, gdt+29(%rip)
 movb $0, gdt+30(%rip)
 movb %ah, gdt+31(%rip)
 shrq $16, %rax
 movl %eax, gdt+32(%rip)
 movw $24, %ax
 ltr %ax
 /* The CPU forms the frame; no hand-written synthetic return frame. */
 leaq payload_entry(%rip), %rax
 movw %ax, idt+1024(%rip)
 movw $8, idt+1026(%rip)
 movb $1, idt+1028(%rip)
 movb $0x8e, idt+1029(%rip)
 shrq $16, %rax
 movw %ax, idt+1030(%rip)
 shrq $16, %rax
 movl %eax, idt+1032(%rip)
 lidt idt_pointer(%rip)
 movabsq $0x123456789abcdef0, %rax
 movq %rax, irq_guard_low(%rip)
 movq %rax, irq_guard_high(%rip)
 movq %rax, boot_guard_low(%rip)
 movq %rax, boot_guard_high(%rip)
 /* The memory oracle owns this counter; marker payloads may occupy it. */
#ifndef ORACLE_DEBUG_MARKER
 movq $0x300000, %rax
 movq $0, (%rax)
#endif
 movq $0, round(%rip)
 /* Keep hardware IRQs masked while testing restoration of an enabled IF. */
 movb $0xff, %al
 outb %al, $0x21
 outb %al, $0xa1
 movb $'B', %al
 outb %al, $0xe9
next_round:
 movq %rsp, expected_rsp(%rip)
 .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
 movdqa vector_pattern(%rip), %xmm\n
 .endr
 ldmxcsr mxcsr_default(%rip)
 /* Every ordinary GPR has a distinct value; RSP has its live stack value. */
 movq $0x1111, %rax
 movq $0x2222, %rbx
 movq $0x3333, %rcx
 movq $0x4444, %rdx
 movq $0x5555, %rsi
 movq $0x6666, %rdi
 movq $0x7777, %rbp
 movq $0x8888, %r8
 movq $0x9999, %r9
 movq $0xaaaa, %r10
 movq $0xbbbb, %r11
 movq $0xcccc, %r12
 movq $0xdddd, %r13
 movq $0xeeee, %r14
 movq $0xffff, %r15
 pushq $0xed7
 popfq
 int $0x40
 /* Snapshot before checking: checks themselves may change flags/registers. */
 pushfq
 popq observed_flags(%rip)
 movq %rax, observed+0(%rip)
 movq %rbx, observed+8(%rip)
 movq %rcx, observed+16(%rip)
 movq %rdx, observed+24(%rip)
 movq %rsi, observed+32(%rip)
 movq %rdi, observed+40(%rip)
 movq %rbp, observed+48(%rip)
 movq %r8, observed+56(%rip)
 movq %r9, observed+64(%rip)
 movq %r10, observed+72(%rip)
 movq %r11, observed+80(%rip)
 movq %r12, observed+88(%rip)
 movq %r13, observed+96(%rip)
 movq %r14, observed+104(%rip)
 movq %r15, observed+112(%rip)
 movq %rsp, observed+120(%rip)
 .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
 movdqa %xmm\n, observed_vectors+(16*\n)(%rip)
 .endr
 stmxcsr observed_mxcsr(%rip)
 cld
 leaq expected(%rip), %rsi
 leaq observed(%rip), %rdi
 movq $16, %rcx
 repe cmpsq
 jne fail_gpr
 movq observed_flags(%rip), %rax
 andq $0xed5, %rax
 cmpq $0xed5, %rax
 jne fail_flags
 leaq observed_vectors(%rip), %rsi
 movq $32, %rcx
vector_check:
 movabsq $0x13579bdf2468ace0, %rax
 cmpq %rax, (%rsi)
 jne fail_vector
 addq $8, %rsi
 loop vector_check
 cmpl $0x1f80, observed_mxcsr(%rip)
 jne fail_vector
 movabsq $0x123456789abcdef0, %rax
 cmpq %rax, irq_guard_low(%rip)
 jne fail_stack
 cmpq %rax, irq_guard_high(%rip)
 jne fail_stack
 cmpq %rax, boot_guard_low(%rip)
 jne fail_stack
 cmpq %rax, boot_guard_high(%rip)
 jne fail_stack
 incq round(%rip)
#ifndef ORACLE_DEBUG_MARKER
 movq round(%rip), %rax
 movq $0x300000, %rbx
 cmpq %rax, (%rbx)
 jne fail_witness
#endif
 movb $'.', %al
 outb %al, $0xe9
 cmpq $8, round(%rip)
 jne next_round
 movb $'P', %al
 outb %al, $0xe9
 movl $0x10, %eax
 jmp exit
fail_gpr: movb $'G', %al; jmp fail
fail_flags: movb $'F', %al; jmp fail
fail_vector: movb $'V', %al; jmp fail
fail_stack: movb $'S', %al; jmp fail
fail_witness: movb $'W', %al
fail:
 outb %al, $0xe9
 movl $0x11, %eax
exit:
 movw $0xf4, %dx
 outl %eax, %dx
 cli
2: hlt; jmp 2b
.balign 16
vector_pattern: .quad 0x13579bdf2468ace0, 0x13579bdf2468ace0
mxcsr_default: .long 0x1f80
.balign 8
gdt: .quad 0, 0x00af9a000000ffff, 0x00cf92000000ffff, 0, 0
gdt_pointer: .word 39; .long gdt
idt_pointer: .word 4095; .quad idt
expected: .quad 0x1111,0x2222,0x3333,0x4444,0x5555,0x6666,0x7777,0x8888,0x9999,0xaaaa,0xbbbb,0xcccc,0xdddd,0xeeee,0xffff
expected_rsp: .quad 0
.section .bss,"aw",@nobits
.balign 4096
pml4: .skip 4096
pdpt: .skip 4096
page_directory: .skip 4096
idt: .skip 4096
tss: .skip 104
.balign 16
observed: .skip 128
observed_flags: .skip 8
observed_mxcsr: .skip 4
round: .skip 8
.balign 16
observed_vectors: .skip 256
boot_guard_low: .skip 8
.balign 16
boot_stack: .skip 32768
boot_stack_top:
boot_guard_high: .skip 8
irq_guard_low: .skip 8
.balign 16
irq_stack: .skip 32768
irq_stack_top:
irq_guard_high: .skip 8
.section .payload,"ax"
.global payload
payload:
.incbin "payload.bin"
.global payload_entry
.set payload_entry, payload + ENTRY_OFFSET

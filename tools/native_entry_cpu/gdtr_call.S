/* Ordinary System V call: caller-saved registers and arithmetic flags may change. */
#include "boot.inc"
 /* No asynchronous interrupts are injected in this isolated load/restore test. */
 movb $0xff, %al
 outb %al, $0x21
 outb %al, $0xa1
 movabsq $0x123456789abcdef0, %rax
 movq %rax, boot_guard_low(%rip)
 movq %rax, boot_guard_high(%rip)
 sgdt original_gdtr(%rip)
 movq $0, round(%rip)
 movb $'B', %al
 outb %al, $0xe9
next_round:
 /* Three 32-byte buffers, each with immediate byte guards after byte ten. */
 leaq buffers(%rip), %rdi
 movq $152, %rcx
 movb $0xa5, %al
 cld
 rep stosb
 leaq buffers(%rip), %rdi
 movq $3, %rcx
init_buffer:
 movabsq $0x1122334455667788, %rax
 movq %rax, 0(%rdi)
 movl $0x44332211, 18(%rdi)
 movw $0x6655, 22(%rdi)
 movabsq $0x8877665544332211, %rax
 movq %rax, 24(%rdi)
 addq $32, %rdi
 loop init_buffer
 movw $23, buffers+96(%rip)
 leaq buffers+120(%rip), %rax
 movq %rax, buffers+98(%rip)
 movabsq $0x13579bdf2468ace0, %rax
 movq %rax, buffers+112(%rip)
 movq $0, buffers+120(%rip)
 movabsq $0x00af9a000000ffff, %rax
 movq %rax, buffers+128(%rip)
 movabsq $0x00cf92000000ffff, %rax
 movq %rax, buffers+136(%rip)
 movabsq $0xfedcba9876543210, %rax
 movq %rax, buffers+144(%rip)
 movq %rsp, expected_rsp(%rip)
 movq $0x2222, %rbx
 movq $0x7777, %rbp
 movq $0xcccc, %r12
 movq $0xdddd, %r13
 movq $0xeeee, %r14
 movq $0xffff, %r15
 leaq buffers(%rip), %rdi
 /* Ordinary ABI requires DF clear. IF starts set and must be restored. */
 pushq $0x202
 popfq
 call payload_entry
 pushfq
 popq observed_flags(%rip)
 movq %rax, observed_result(%rip)
 cli
 cld
 cmpq expected_rsp(%rip), %rsp
 jne fail_stack
 cmpq $0x2222, %rbx
 jne fail_gpr
 cmpq $0x7777, %rbp
 jne fail_gpr
 cmpq $0xcccc, %r12
 jne fail_gpr
 cmpq $0xdddd, %r13
 jne fail_gpr
 cmpq $0xeeee, %r14
 jne fail_gpr
 cmpq $0xffff, %r15
 jne fail_gpr
 movq observed_flags(%rip), %rax
 andq $0x600, %rax
 cmpq $0x200, %rax
 jne fail_flags
 cmpq $0, observed_result(%rip)
 jne fail_result
 /* Read CPU state independently; source saved/restored equality is insufficient. */
 sgdt restored_gdtr(%rip)
 leaq original_gdtr(%rip), %rsi
 leaq restored_gdtr(%rip), %rdi
 movq $10, %rcx
 repe cmpsb
 jne fail_gdtr
 leaq original_gdtr(%rip), %rsi
 leaq buffers+8(%rip), %rdi
 movq $10, %rcx
 repe cmpsb
 jne fail_gdtr
 leaq original_gdtr(%rip), %rsi
 leaq buffers+72(%rip), %rdi
 movq $10, %rcx
 repe cmpsb
 jne fail_gdtr
 leaq buffers+96(%rip), %rsi
 leaq buffers+40(%rip), %rdi
 movq $10, %rcx
 repe cmpsb
 jne fail_gdtr
 cmpw $23, buffers+96(%rip)
 jne fail_gdtr
 movq buffers+98(%rip), %rax
 leaq buffers+120(%rip), %rdx
 cmpq %rdx, %rax
 jne fail_gdtr
 movabsq $0x13579bdf2468ace0, %rax
 cmpq %rax, buffers+112(%rip)
 jne fail_neighbor
 cmpq $0, buffers+120(%rip)
 jne fail_neighbor
 movabsq $0x00af9a000000ffff, %rax
 cmpq %rax, buffers+128(%rip)
 jne fail_neighbor
 movabsq $0x00cf92000000ffff, %rax
 cmpq %rax, buffers+136(%rip)
 jne fail_neighbor
 movabsq $0xfedcba9876543210, %rax
 cmpq %rax, buffers+144(%rip)
 jne fail_neighbor
#ifdef CORRUPT_NEIGHBOR
 movb $0, buffers+18(%rip)
#endif
 leaq buffers(%rip), %rdi
 movq $3, %rcx
check_buffer:
 movabsq $0x1122334455667788, %rax
 cmpq %rax, 0(%rdi)
 jne fail_neighbor
 cmpl $0x44332211, 18(%rdi)
 jne fail_neighbor
 cmpw $0x6655, 22(%rdi)
 jne fail_neighbor
 movabsq $0x8877665544332211, %rax
 cmpq %rax, 24(%rdi)
 jne fail_neighbor
 addq $32, %rdi
 loop check_buffer
 cmpl $0xa5a5a5a5, buffers+106(%rip)
 jne fail_neighbor
 cmpw $0xa5a5, buffers+110(%rip)
 jne fail_neighbor
 movabsq $0x123456789abcdef0, %rax
 cmpq %rax, boot_guard_low(%rip)
 jne fail_stack
 cmpq %rax, boot_guard_high(%rip)
 jne fail_stack
 incq round(%rip)
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
fail_stack: movb $'S', %al; jmp fail
fail_result: movb $'R', %al; jmp fail
fail_gdtr: movb $'D', %al; jmp fail
fail_neighbor: movb $'N', %al
fail:
 outb %al, $0xe9
 movl $0x11, %eax
exit:
 movw $0xf4, %dx
 outl %eax, %dx
 cli
2: hlt; jmp 2b
.balign 8
gdt: .quad 0, 0x00af9a000000ffff, 0x00cf92000000ffff
gdt_pointer: .word 23; .long gdt
.section .bss,"aw",@nobits
.balign 4096
pml4: .skip 4096
pdpt: .skip 4096
page_directory: .skip 4096
.balign 16
buffers: .skip 152
original_gdtr: .skip 10
restored_gdtr: .skip 10
.balign 8
observed_flags: .skip 8
observed_result: .skip 8
expected_rsp: .skip 8
round: .skip 8
boot_guard_low: .skip 8
.balign 16
boot_stack: .skip 32768
boot_stack_top:
boot_guard_high: .skip 8
.section .payload,"ax"
.global payload
payload:
.incbin "payload.bin"
.global payload_entry
.set payload_entry, payload + ENTRY_OFFSET

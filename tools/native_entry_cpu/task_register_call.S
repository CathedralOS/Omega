/* The harness owns GDT/TSS bytes until shutdown, beyond every source call. */
#include "boot.inc"
 movb $0xff, %al
 outb %al, $0x21
 outb %al, $0xa1
 cli
 cld
 leaq buffers(%rip), %rdi
 movq $320, %rcx
 movb $0xa5, %al
 rep stosb
 movabsq $0x1122334455667788, %rax
 movq %rax, buffers(%rip)
 movq %rax, buffers+48(%rip)
 movq %rax, buffers+160(%rip)
 movq %rax, buffers+312(%rip)
 movq %rax, boot_guard_low(%rip)
 movq %rax, boot_guard_high(%rip)
 movq $0, buffers+8(%rip)
 movabsq $0x00af9a000000ffff, %rax
 movq %rax, buffers+16(%rip)
 movabsq $0x00cf92000000ffff, %rax
 movq %rax, buffers+24(%rip)
 leaq buffers+56(%rip), %rax
 movq %rax, %rdx
 andq $0xffffff, %rax
 shlq $16, %rax
 movabsq $0x0000890000000067, %rcx
 orq %rcx, %rax
 movq %rdx, %rcx
 shrq $24, %rcx
 andq $0xff, %rcx
 shlq $56, %rcx
 orq %rcx, %rax
 movq %rax, buffers+32(%rip)
 movq %rax, expected_descriptor(%rip)
 shrq $32, %rdx
 movq %rdx, buffers+40(%rip)
 leaq buffers+56(%rip), %rdi
 movq $104, %rcx
 xorl %eax, %eax
 rep stosb
 movw $104, buffers+158(%rip)
 movw $39, installed_gdtr(%rip)
 leaq buffers+8(%rip), %rax
 movq %rax, installed_gdtr+2(%rip)
 /* The alternate table has an otherwise valid descriptor. Only its address
    differs, so omitting the actual-GDTR association would be observable. */
 leaq buffers+8(%rip), %rsi
 leaq buffers+168(%rip), %rdi
 movq $5, %rcx
 rep movsq
 lgdt installed_gdtr(%rip)
 movq $0, round(%rip)
 movb $'B', %al
 outb %al, $0xe9
next_round:
 movq %rsp, expected_rsp(%rip)
 movq $0x2222, %rbx
 movq $0x7777, %rbp
 movq $0xcccc, %r12
 movq $0xdddd, %r13
 movq $0xeeee, %r14
 movq $0xffff, %r15
 leaq buffers(%rip), %rdi
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
 testq $0x600, observed_flags(%rip)
 jne fail_flags
 cmpq $0, observed_result(%rip)
 jne fail_result
 str %ax
 cmpw $24, %ax
 jne fail_task
 sgdt observed_gdtr(%rip)
 leaq installed_gdtr(%rip), %rsi
 leaq observed_gdtr(%rip), %rdi
 movq $10, %rcx
 repe cmpsb
 jne fail_task
 movq expected_descriptor(%rip), %rax
 btsq $41, %rax
 cmpq %rax, buffers+32(%rip)
 jne fail_task
 cmpq $0, buffers+40(%rip)
 jne fail_task
 cmpq $0, buffers+8(%rip)
 jne fail_neighbor
 movabsq $0x00af9a000000ffff, %rax
 cmpq %rax, buffers+16(%rip)
 jne fail_neighbor
 movabsq $0x00cf92000000ffff, %rax
 cmpq %rax, buffers+24(%rip)
 jne fail_neighbor
#ifdef CORRUPT_NEIGHBOR
 movb $0, buffers+48(%rip)
#endif
 movabsq $0x1122334455667788, %rax
 cmpq %rax, buffers(%rip)
 jne fail_neighbor
 cmpq %rax, buffers+48(%rip)
 jne fail_neighbor
 cmpq %rax, buffers+160(%rip)
 jne fail_neighbor
 cmpq %rax, buffers+312(%rip)
 jne fail_neighbor
 cmpq %rax, boot_guard_low(%rip)
 jne fail_stack
 cmpq %rax, boot_guard_high(%rip)
 jne fail_stack
 leaq buffers+56(%rip), %rsi
 movq $102, %rcx
check_task:
 cmpb $0, (%rsi)
 jne fail_neighbor
 incq %rsi
 loop check_task
 cmpw $104, (%rsi)
 jne fail_neighbor
 cmpq $0, buffers+168(%rip)
 jne fail_neighbor
 movabsq $0x00af9a000000ffff, %rax
 cmpq %rax, buffers+176(%rip)
 jne fail_neighbor
 movabsq $0x00cf92000000ffff, %rax
 cmpq %rax, buffers+184(%rip)
 jne fail_neighbor
 movq expected_descriptor(%rip), %rax
 cmpq %rax, buffers+192(%rip)
 jne fail_neighbor
 cmpq $0, buffers+200(%rip)
 jne fail_neighbor
 leaq buffers+208(%rip), %rsi
 movq $104, %rcx
check_other:
 cmpb $0xa5, (%rsi)
 jne fail_neighbor
 incq %rsi
 loop check_other
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
fail_task: movb $'T', %al; jmp fail
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
buffers: .skip 320
installed_gdtr: .skip 10
observed_gdtr: .skip 10
.balign 8
expected_descriptor: .skip 8
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

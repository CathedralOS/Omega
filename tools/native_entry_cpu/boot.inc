/* Test infrastructure only. The payload is supplied separately. */
.section .boot,"ax"
.code32
.global _start
.balign 4
header:
.long 0x1badb002, 0x10003, -(0x1badb002 + 0x10003)
.long header, 0x100000, image_end, memory_end, _start
_start:
 cli
 movl $boot_stack_top, %esp
 /* Identity-map the first GiB with two-MiB pages. */
 movl $pdpt, %eax
 orl $3, %eax
 movl %eax, pml4
 movl $page_directory, %eax
 orl $3, %eax
 movl %eax, pdpt
 xorl %ecx, %ecx
1:
 movl %ecx, %eax
 shll $21, %eax
 orl $0x83, %eax
 movl %eax, page_directory(,%ecx,8)
 incl %ecx
 cmpl $512, %ecx
 jne 1b
 movl %cr4, %eax
 orl $0x620, %eax
 movl %eax, %cr4
 movl $pml4, %eax
 movl %eax, %cr3
 movl $0xc0000080, %ecx
 rdmsr
 orl $0x100, %eax
 wrmsr
 movl %cr0, %eax
 andl $~4, %eax
 orl $0x80000003, %eax
 movl %eax, %cr0
 lgdt gdt_pointer
 ljmp $8, $long_mode
.code64
long_mode:
 movw $16, %ax
 movw %ax, %ds
 movw %ax, %es
 movw %ax, %ss
 movq $boot_stack_top, %rsp

__RNvMNtNtCslTHnsk5Kthf_12syntax_trees12syntax_trees11body_copiesNtNtB4_11item_copies10SyntaxCopy29remap_static_machine_argument:
000000010012c84c	stp	x22, x21, [sp, #-0x30]!
000000010012c850	stp	x20, x19, [sp, #0x10]
000000010012c854	stp	x29, x30, [sp, #0x20]
000000010012c858	add	x29, sp, #0x20
000000010012c85c	mov	x21, x2
000000010012c860	mov	x19, x1
000000010012c864	mov	x20, x0
000000010012c868	ldr	w2, [x2, #0xb8]
000000010012c86c	ldr	w3, [x21, #0xbc]
000000010012c870	bl	__RNvMNtNtCslTHnsk5Kthf_12syntax_trees12syntax_trees11body_copiesNtNtB4_11item_copies10SyntaxCopy26copy_type_reference_handle
000000010012c874	stp	w0, w1, [x21, #0xb8]
000000010012c878	ldr	x8, [x21, #0xc0]
000000010012c87c	cbz	x8, 0x10012c8b0
000000010012c880	ldr	x9, [x8, #0x18]
000000010012c884	cbz	x9, 0x10012c8b0
000000010012c888	ldr	x2, [x8, #0x10]
000000010012c88c	mov	w8, #0xc8
000000010012c890	mul	x21, x9, x8
000000010012c894	add	x22, x2, #0xc8
000000010012c898	mov	x0, x20
000000010012c89c	mov	x1, x19
000000010012c8a0	bl	__RNvMNtNtCslTHnsk5Kthf_12syntax_trees12syntax_trees11body_copiesNtNtB4_11item_copies10SyntaxCopy29remap_static_machine_argument
000000010012c8a4	mov	x2, x22
000000010012c8a8	subs	x21, x21, #0xc8
000000010012c8ac	b.ne	0x10012c894
000000010012c8b0	ldp	x29, x30, [sp, #0x20]
000000010012c8b4	ldp	x20, x19, [sp, #0x10]
000000010012c8b8	ldp	x22, x21, [sp], #0x30
000000010012c8bc	ret
__RNvMs1_NtNtCslTHnsk5Kthf_12syntax_trees12syntax_trees11item_copiesNtB7_11SyntaxTrees33transfer_static_machine_arguments:
000000010014be54	sub	sp, sp, #0x80
000000010014be58	stp	x26, x25, [sp, #0x30]
000000010014be5c	stp	x24, x23, [sp, #0x40]
000000010014be60	stp	x22, x21, [sp, #0x50]
000000010014be64	stp	x20, x19, [sp, #0x60]
000000010014be68	stp	x29, x30, [sp, #0x70]
000000010014be6c	add	x29, sp, #0x70
000000010014be70	mov	x19, x3
000000010014be74	mov	x20, x2
000000010014be78	stp	x2, x3, [sp]
000000010014be7c	cbz	x3, 0x10014becc
000000010014be80	mov	x21, x1
000000010014be84	mov	x22, x0
000000010014be88	mov	w8, #0xc8
000000010014be8c	mul	x24, x19, x8
000000010014be90	mov	w25, #0x4
000000010014be94	mov	x23, x20
000000010014be98	stp	xzr, x25, [sp, #0x10]
000000010014be9c	stp	xzr, x22, [sp, #0x20]
000000010014bea0	add	x0, sp, #0x10
000000010014bea4	mov	x1, x21
000000010014bea8	mov	x2, x23
000000010014beac	bl	__RNvMNtNtCslTHnsk5Kthf_12syntax_trees12syntax_trees11body_copiesNtNtB4_11item_copies10SyntaxCopy29remap_static_machine_argument
000000010014beb0	add	x0, sp, #0x10
000000010014beb4	mov	w1, #0x4
000000010014beb8	mov	w2, #0x10
000000010014bebc	bl	__RNvMs3_NtCscU4bHatjcek_5alloc7raw_vecNtB5_11RawVecInner10deallocateCs2qEGCSZWdXs_5gimli
000000010014bec0	add	x23, x23, #0xc8
000000010014bec4	subs	x24, x24, #0xc8
000000010014bec8	b.ne	0x10014be98
000000010014becc	mov	x0, x20
000000010014bed0	mov	x1, x19
000000010014bed4	ldp	x29, x30, [sp, #0x70]
000000010014bed8	ldp	x20, x19, [sp, #0x60]
000000010014bedc	ldp	x22, x21, [sp, #0x50]
000000010014bee0	ldp	x24, x23, [sp, #0x40]
000000010014bee4	ldp	x26, x25, [sp, #0x30]
000000010014bee8	add	sp, sp, #0x80
000000010014beec	ret
000000010014bef0	mov	x19, x0
000000010014bef4	add	x0, sp, #0x10
000000010014bef8	mov	w1, #0x4
000000010014befc	mov	w2, #0x10
000000010014bf00	bl	__RNvMs3_NtCscU4bHatjcek_5alloc7raw_vecNtB5_11RawVecInner10deallocateCs2qEGCSZWdXs_5gimli
000000010014bf04	b	0x10014bf0c
000000010014bf08	mov	x19, x0
000000010014bf0c	mov	x0, sp
000000010014bf10	bl	__RINvNtCs4aqKKZZmviT_4core3ptr9drop_glueINtNtCscU4bHatjcek_5alloc5boxed3BoxSNtNtNtNtCslTHnsk5Kthf_12syntax_trees12syntax_trees6values10expression21StaticMachineArgumentEEB1j_
000000010014bf14	mov	x0, x19
000000010014bf18	bl	0x100197a58 ; symbol stub for: __Unwind_Resume
000000010014bf1c	bl	__RNvNtCs4aqKKZZmviT_4core9panicking16panic_in_cleanup

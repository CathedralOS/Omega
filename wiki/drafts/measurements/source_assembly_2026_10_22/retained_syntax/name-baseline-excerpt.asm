
build/retained-syntax-ownership/syntax-probe-baseline:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100072b60 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier>:
100072b60:     	sub	sp, sp, #0x130
100072b64:     	stp	x28, x27, [sp, #0x100]
100072b68:     	stp	x20, x19, [sp, #0x110]
100072b6c:     	stp	x29, x30, [sp, #0x120]
100072b70:     	add	x29, sp, #0x120
100072b74:     	mov	x20, x0
100072b78:     	mov	x19, x8
100072b7c:     	mov	x8, sp
100072b80:     	bl	0x100071e28 <<tokens_to_syntax_trees::input::token_cursor::Input>::expect_token>
100072b84:     	ldp	x8, x1, [sp]
100072b88:     	ldp	q0, q1, [sp, #0x10]
100072b8c:     	stp	q0, q1, [sp, #0x70]
100072b90:     	ldr	x9, [sp, #0x30]
100072b94:     	str	x9, [sp, #0x90]
100072b98:     	cbz	x8, 0x100072bb8 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x58>
100072b9c:     	ldp	q0, q1, [sp, #0x70]
100072ba0:     	stp	q0, q1, [x19, #0x10]
100072ba4:     	ldr	x8, [sp, #0x90]
100072ba8:     	str	x8, [x19, #0x30]
100072bac:     	mov	x8, #-0x1               ; =-1
100072bb0:     	stp	x8, x1, [x19]
100072bb4:     	b	0x100072d08 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x1a8>
100072bb8:     	ldp	q0, q1, [sp, #0x70]
100072bbc:     	stp	q0, q1, [x29, #-0x60]
100072bc0:     	ldr	x8, [sp, #0x90]
100072bc4:     	stur	x8, [x29, #-0x40]
100072bc8:     	ldrb	w8, [x1, #0x18]
100072bcc:     	sub	w9, w8, #0x2
100072bd0:     	cmp	w8, #0x1
100072bd4:     	csinc	w8, w9, wzr, hi
100072bd8:     	ands	w8, w8, #0xff
100072bdc:     	b.eq	0x100072c28 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0xc8>
100072be0:     	cmp	w8, #0x3
100072be4:     	b.ne	0x100072bf4 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x94>
100072be8:     	ldrb	w8, [x1, #0x19]
100072bec:     	cmp	w8, #0x13
100072bf0:     	b.ne	0x100072c28 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0xc8>
100072bf4:     	ldp	q0, q1, [x20]
100072bf8:     	stp	q0, q1, [sp]
100072bfc:     	ldr	x8, [x20, #0x20]
100072c00:     	str	x8, [sp, #0x20]
100072c04:     	adrp	x2, 0x1001d5000 <_anon.57ec092bcacd346c9a20b8ad5dbadf13.27+0xc80>
100072c08:     	add	x2, x2, #0x393
100072c0c:     	add	x8, x19, #0x8
100072c10:     	mov	x0, sp
100072c14:     	mov	w3, #0xa                ; =10
100072c18:     	bl	0x1000cb874 <tokens_to_syntax_trees::diagnostics::render_diagnostic::expected::<&str>>
100072c1c:     	mov	x8, #-0x1               ; =-1
100072c20:     	str	x8, [x19]
100072c24:     	b	0x100072d08 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x1a8>
100072c28:     	ldp	x9, x10, [x1, #0x8]
100072c2c:     	ldp	x8, x11, [x20, #0x10]
100072c30:     	stp	x10, x11, [x29, #-0x30]
100072c34:     	stur	x9, [x29, #-0x38]
100072c38:     	ldr	x9, [x8]
100072c3c:     	cbz	x9, 0x100072c7c <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x11c>
100072c40:     	ldr	x9, [x8, #0x8]
100072c44:     	mov	w10, #0x1               ; =1
100072c48:     	ldadd	x10, x10, [x9]
100072c4c:     	tbnz	x10, #0x3f, 0x100072d34 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x1d4>
100072c50:     	ldr	x8, [x8, #0x10]
100072c54:     	mov	w10, #0x2               ; =2
100072c58:     	str	x10, [sp, #0x70]
100072c5c:     	ldur	q0, [x29, #-0x38]
100072c60:     	stur	q0, [sp, #0x78]
100072c64:     	ldur	x10, [x29, #-0x28]
100072c68:     	stp	x10, x9, [sp, #0x88]
100072c6c:     	str	x8, [sp, #0x98]
100072c70:     	str	q0, [sp, #0xa0]
100072c74:     	str	x10, [sp, #0xb0]
100072c78:     	b	0x100072cbc <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x15c>
100072c7c:     	add	x8, sp, #0x70
100072c80:     	mov	x0, x20
100072c84:     	bl	0x100074270 <<tokens_to_syntax_trees::input::token_cursor::Input>::text>
100072c88:     	ldp	x8, x0, [sp, #0x70]
100072c8c:     	ldr	x1, [sp, #0x80]
100072c90:     	cbz	x8, 0x100072cac <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x14c>
100072c94:     	mov	x8, sp
100072c98:     	bl	0x100199ea4 <core::str::converts::from_utf8>
100072c9c:     	ldr	w8, [sp]
100072ca0:     	ldp	x9, x1, [sp, #0x8]
100072ca4:     	cmp	w8, #0x0
100072ca8:     	csel	x0, xzr, x9, ne
100072cac:     	cbz	x0, 0x100072d1c <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x1bc>
100072cb0:     	add	x8, sp, #0x70
100072cb4:     	sub	x2, x29, #0x38
100072cb8:     	bl	0x100074d18 <<syntax_trees::syntax_trees::names::identifier::Identifier>::new::<&str>>
100072cbc:     	ldr	x8, [sp, #0xb0]
100072cc0:     	str	x8, [sp, #0x40]
100072cc4:     	ldp	q0, q1, [sp, #0x70]
100072cc8:     	stp	q0, q1, [sp]
100072ccc:     	ldp	q0, q1, [sp, #0x90]
100072cd0:     	stp	q0, q1, [sp, #0x20]
100072cd4:     	ldp	q0, q1, [x29, #-0x60]
100072cd8:     	stur	q0, [sp, #0x48]
100072cdc:     	stur	q1, [sp, #0x58]
100072ce0:     	ldur	x8, [x29, #-0x40]
100072ce4:     	str	x8, [sp, #0x68]
100072ce8:     	ldp	q0, q1, [sp, #0x40]
100072cec:     	stp	q0, q1, [x19, #0x40]
100072cf0:     	ldr	q0, [sp, #0x60]
100072cf4:     	str	q0, [x19, #0x60]
100072cf8:     	ldp	q0, q1, [sp]
100072cfc:     	stp	q0, q1, [x19]
100072d00:     	ldp	q0, q1, [sp, #0x20]
100072d04:     	stp	q0, q1, [x19, #0x20]
100072d08:     	ldp	x29, x30, [sp, #0x120]
100072d0c:     	ldp	x20, x19, [sp, #0x110]
100072d10:     	ldp	x28, x27, [sp, #0x100]
100072d14:     	add	sp, sp, #0x130
100072d18:     	ret
100072d1c:     	adrp	x0, 0x1001d5000 <_anon.57ec092bcacd346c9a20b8ad5dbadf13.27+0xc80>
100072d20:     	add	x0, x0, #0x492
100072d24:     	adrp	x2, 0x100262000 <_anon.a5e568f4ab2c6678820a31b5001fe279.37+0x3420>
100072d28:     	add	x2, x2, #0xfb0
100072d2c:     	mov	w1, #0x3f               ; =63
100072d30:     	bl	0x1001a2d98 <core::option::expect_failed>
100072d34:     	brk	#0x1

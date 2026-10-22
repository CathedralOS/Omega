
build/retained-syntax-ownership/syntax-probe-candidate:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000d3c7c <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier>:
1000d3c7c:     	sub	sp, sp, #0x110
1000d3c80:     	stp	x22, x21, [sp, #0xe0]
1000d3c84:     	stp	x20, x19, [sp, #0xf0]
1000d3c88:     	stp	x29, x30, [sp, #0x100]
1000d3c8c:     	add	x29, sp, #0x100
1000d3c90:     	mov	x20, x0
1000d3c94:     	mov	x19, x8
1000d3c98:     	add	x21, sp, #0x50
1000d3c9c:     	add	x8, sp, #0x50
1000d3ca0:     	bl	0x1000d2cb8 <<tokens_to_syntax_trees::input::token_cursor::Input>::expect_token>
1000d3ca4:     	ldrb	w8, [sp, #0x8a]
1000d3ca8:     	ldr	x1, [sp, #0x50]
1000d3cac:     	cmp	w8, #0x2
1000d3cb0:     	b.ne	0x1000d3d5c <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0xe0>
1000d3cb4:     	ldur	q0, [x21, #0x8]
1000d3cb8:     	ldur	q1, [x21, #0x18]
1000d3cbc:     	stp	q0, q1, [sp, #0x10]
1000d3cc0:     	ldur	x8, [x21, #0x28]
1000d3cc4:     	str	x8, [sp, #0x30]
1000d3cc8:     	mov	w8, #0x2                ; =2
1000d3ccc:     	cmp	w8, #0x2
1000d3cd0:     	b.eq	0x1000d3d90 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x114>
1000d3cd4:     	ldp	q0, q1, [sp, #0x10]
1000d3cd8:     	stp	q0, q1, [x21, #0x50]
1000d3cdc:     	ldr	q0, [sp, #0x30]
1000d3ce0:     	str	q0, [x21, #0x70]
1000d3ce4:     	ldrh	w9, [sp, #0x40]
1000d3ce8:     	sturh	w9, [x29, #-0x30]
1000d3cec:     	ldr	w9, [sp, #0x8]
1000d3cf0:     	stur	w9, [x29, #-0x68]
1000d3cf4:     	ldrb	w9, [sp, #0xc]
1000d3cf8:     	sturb	w9, [x29, #-0x64]
1000d3cfc:     	ldrb	w9, [x1, #0x18]
1000d3d00:     	sub	w10, w9, #0x2
1000d3d04:     	cmp	w9, #0x1
1000d3d08:     	csinc	w9, w10, wzr, hi
1000d3d0c:     	ands	w9, w9, #0xff
1000d3d10:     	b.eq	0x1000d3db0 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x134>
1000d3d14:     	cmp	w9, #0x3
1000d3d18:     	b.ne	0x1000d3d28 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0xac>
1000d3d1c:     	ldrb	w9, [x1, #0x19]
1000d3d20:     	cmp	w9, #0x13
1000d3d24:     	b.ne	0x1000d3db0 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x134>
1000d3d28:     	ldp	q0, q1, [x20]
1000d3d2c:     	stp	q0, q1, [sp, #0x50]
1000d3d30:     	ldr	q0, [x20, #0x20]
1000d3d34:     	str	q0, [sp, #0x70]
1000d3d38:     	ldr	x8, [x20, #0x30]
1000d3d3c:     	str	x8, [sp, #0x80]
1000d3d40:     	adrp	x2, 0x1002ad000 <_anon.181b00417164b387859c57a89b14f098.250+0xec>
1000d3d44:     	add	x2, x2, #0x918
1000d3d48:     	add	x8, x19, #0x8
1000d3d4c:     	add	x0, sp, #0x50
1000d3d50:     	mov	w3, #0xa                ; =10
1000d3d54:     	bl	0x1000c1880 <tokens_to_syntax_trees::diagnostics::render_diagnostic::expected::<&str>>
1000d3d58:     	b	0x1000d3da4 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x128>
1000d3d5c:     	ldur	q0, [x21, #0x8]
1000d3d60:     	ldur	q1, [x21, #0x18]
1000d3d64:     	stp	q0, q1, [sp, #0x10]
1000d3d68:     	ldur	q0, [x21, #0x28]
1000d3d6c:     	str	q0, [sp, #0x30]
1000d3d70:     	ldurh	w9, [x21, #0x38]
1000d3d74:     	strh	w9, [sp, #0x40]
1000d3d78:     	ldur	w9, [x21, #0x3b]
1000d3d7c:     	str	w9, [sp, #0x8]
1000d3d80:     	ldrb	w9, [sp, #0x8f]
1000d3d84:     	strb	w9, [sp, #0xc]
1000d3d88:     	cmp	w8, #0x2
1000d3d8c:     	b.ne	0x1000d3cd4 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x58>
1000d3d90:     	ldp	q0, q1, [sp, #0x10]
1000d3d94:     	stp	q0, q1, [x19, #0x10]
1000d3d98:     	ldr	x8, [sp, #0x30]
1000d3d9c:     	str	x8, [x19, #0x30]
1000d3da0:     	str	x1, [x19, #0x8]
1000d3da4:     	mov	w8, #-0x1               ; =-1
1000d3da8:     	str	w8, [x19]
1000d3dac:     	b	0x1000d3dfc <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier+0x180>
1000d3db0:     	ldr	x9, [x20, #0x20]
1000d3db4:     	ldp	x10, x11, [x1, #0x8]
1000d3db8:     	ldp	w12, w13, [x20, #0x28]
1000d3dbc:     	ldp	q0, q1, [x21, #0x50]
1000d3dc0:     	stp	q0, q1, [x19, #0x30]
1000d3dc4:     	ldr	q0, [x21, #0x70]
1000d3dc8:     	str	q0, [x19, #0x50]
1000d3dcc:     	ldurh	w14, [x29, #-0x30]
1000d3dd0:     	strh	w14, [x19, #0x60]
1000d3dd4:     	ldur	w14, [x29, #-0x68]
1000d3dd8:     	stur	w14, [x19, #0x63]
1000d3ddc:     	ldurb	w14, [x29, #-0x64]
1000d3de0:     	strb	w14, [x19, #0x67]
1000d3de4:     	mov	w14, #0x2               ; =2
1000d3de8:     	stp	w14, w12, [x19]
1000d3dec:     	str	w13, [x19, #0x8]
1000d3df0:     	stp	x10, x11, [x19, #0x18]
1000d3df4:     	str	x9, [x19, #0x28]
1000d3df8:     	strb	w8, [x19, #0x62]
1000d3dfc:     	ldp	x29, x30, [sp, #0x100]
1000d3e00:     	ldp	x20, x19, [sp, #0xf0]
1000d3e04:     	ldp	x22, x21, [sp, #0xe0]
1000d3e08:     	add	sp, sp, #0x110
1000d3e0c:     	ret

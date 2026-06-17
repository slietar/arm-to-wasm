.section .rodata
msg:
	.ascii "Hello, world!\n"

.section .text
.global _start


_start:
    stp x29, x30, [sp, #-16]!

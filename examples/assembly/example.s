.section .rodata
msg:
	.ascii "Hello, world!\n"

.section .text
.global _start

_start:
    mov	x8, #93
	mov x0, #16
    svc	#0
    subs x0, x0, #16
    b.ne bar

foo:
    svc #72

bar:
    svc #82

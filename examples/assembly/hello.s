.section .rodata
msg:
	.ascii "Hello, world!\n"

.section .text
.global _start

exit:
    mov	x8, #93
    svc	#0
    ret

_start:
	// write(1, msg, 14)
	mov x0, #1
	adrp x1, msg
	add x1, x1, :lo12:msg
	mov x2, #14
	mov x8, #64
	svc #0

	// exit(0)
	mov x0, #16
	bl exit

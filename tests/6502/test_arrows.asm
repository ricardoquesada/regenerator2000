* = $1000

l0:
	bne l0
	nop
	nop
	nop
	beq l0
	nop
	nop
	bmi l0
	nop
	jsr l0
	nop
	nop
	jmp l0
	nop
	nop
	nop
	jmp (l0)
	nop
	nop

	bne l1
	nop
	beq l1
	nop
	nop
	bmi l1
	nop
	nop
	nop
	jsr l1
	nop
	nop
	jmp l1
	nop

l1: 	jmp l1
	nop
	jmp (l1)
	nop


partial = *+1
l2:	bne l2
	nop
	bne l2
	nop
	bne l2
	nop
	bne l1
	nop
	bne l0
	nop

l3:	bne l3
	nop

	bne partial
	nop
	bne partial
	nop
	beq partial
	nop

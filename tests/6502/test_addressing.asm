* = $1000

	lda @w $01
	sta $01

	lda @w $01,x
	sta $01,x

	lda @w $01,y
	sta $01,y

	lda #$ea
	lda #$ea
	lda #$ea
	lda #$ea
	lda #$ea
	lda #$ea

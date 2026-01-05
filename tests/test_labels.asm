* = $1000

	; C64 related
	lda #$a0
	jsr $ffe1

	lda #$00
	sta $d020
	sta $d021

	stx $1000
	sty $1001
	
	; C128 related
	jsr $FF8D

	; Plus/4
	jsr $0500

	; PET 4.0
	jsr $B4FB


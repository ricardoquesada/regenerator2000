* = $1000

	; C64 related
	lda #$a0
	jsr $ffe1

	lda #$00
	sta $c000
	sta $c001

	sta $d020
	sta $d021
	jmp $d020

	sta $f000
	sta $f001

	stx $0314
	sty $0315

	stx $0320
	sty $0321

	stx $1000
	sty $1001

	stx $fffe
	sty $ffff
	
	; C128 related
	jsr $FF8D

	; Plus/4
	jsr $0500

	; PET 4.0
	jsr $B4FB

	ldx #$00
	ldy #$c0
	stx $fc
	sty $fd

	lda #$00
	sta $fa
	lda #$d0
	sta $fb

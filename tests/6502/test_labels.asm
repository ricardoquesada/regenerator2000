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

	lda #$00
	sta $07f8

	ldx #$07
l0:
	sta $07f8,x
	dex
	bpl l0

	sta $07f9,y
	lda $07f9

	lda $c0
	sta $c0,x

	sta ($c1,x)
	lda $c1

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

	stx @w $fc
	sty @w $fd

	lda #$00
	sta $fa
	lda #$d0
	sta $fb

	lda #$00
	sta @w $fa
	lda #$d0
	sta @w $fb


*=$1100
	; should auto hi/lo
	lda #$00
	sta $f0
	lda #$c0
	sta $f1

	; should auto hi/lo
	ldx #$00
	ldy #$c0
	stx $c0
	sty $c1

	; should NOT auto hi/lo
	ldx #$00
	ldy #$c0
	sta $c0
	sta $c1

	; should NOT auto hi/lo
	lda #$00
	sta $f8
	lda #$c0
	sta $fa

	; should auto hi/lo
	lda #$00
	sta $0314
	lda #$c0
	sta $0315

	; should auto hi/lo
	ldx #$00
	ldy #$c0
	stx $fffe
	sty $ffff

	ldy #$c0
	ldx #$00
	sty $fffa
	stx $fffb



*=$1200
	lda #00
loop:
lo_addr = * + 1
hi_addr = * + 2
	sta $0400
	inc lo_addr
	inc hi_addr
	jmp loop



lo1 = *+$01
lo2 = *+$02
	lda $4c00
	sta $d000
	bne lo1
	beq lo2

;    64tass Illegal Opcode Demo
;    --------------------------
;    Build with:
;    64tass -i -a illegals.asm -o illegals.prg
;
;    Run with:
;    x64sc illegals.prg

; ==============================================================================
; BASIC HEADER (10 SYS 2064)
; ==============================================================================
* = $0801
        .word (+)
        .word 10
        .null $9e, format("%d", start)
+       .word 0

; ==============================================================================
; CONSTANTS
; ==============================================================================
RASTER  = $d012
BORDER  = $d020
BG_COL  = $d021
DELAY   = $02     ; A zero-page address we will use for our DCP counter

; ==============================================================================
; MAIN PROGRAM
; ==============================================================================
start:
        sei             ; Disable interrupts (stable timing)

        ; ----------------------------------------------------------------------
        ; ILLEGAL: LAX (Load Accumulator and X)
        ; Cycles: 2 | Bytes: 2
        ; Replaces: LDA #$00, TAX
        ; ----------------------------------------------------------------------
        lax #$00        ; Loads #$00 into A *and* X
        stx BORDER      ; Clear border (Black)
        stx BG_COL      ; Clear background (Black)

main_loop:

        ; Wait for a specific raster line to start our effect
wait_frame:
        lda RASTER
        cmp #$60        ; Wait for line $60
        bne wait_frame

        ldy #$00        ; Initialize Y index for our color table

line_loop:
        ; ----------------------------------------------------------------------
        ; ILLEGAL: LAX (Absolute, Y)
        ; Loads a color from the table into A and X simultaneously.
        ; We use X for the store (STX) and A could be used for math.
        ; ----------------------------------------------------------------------
        lax colors,y    ; A = colors[y], X = colors[y]

        stx BORDER      ; Set the border color

        ; ----------------------------------------------------------------------
        ; ILLEGAL: DCP (Decrement and Compare)
        ; Cycles: 5 | Bytes: 2
        ; Replaces: DEC DELAY, CMP #$...
        ; This acts as a delay loop to stretch the color across the screen.
        ; ----------------------------------------------------------------------
        lda #$04        ; We want to loop until DELAY matches this value
        sta DELAY       ; Reset the delay counter
delay_loop:
        dcp DELAY       ; Decrements memory at DELAY, then compares with A
        bne delay_loop  ; Branch if (DELAY - 1) != A

        ; ----------------------------------------------------------------------
        ; ILLEGAL: SAX (Store A AND X)
        ; Cycles: 4 | Bytes: 3
        ; Computes (A & X) and stores it.
        ; Since A and X currently hold the Color, (A & X) == Color.
        ; We store it to a "safe" place just to demonstrate the opcode.
        ; ----------------------------------------------------------------------
        sax $0400       ; Just showing off SAX: Stores color to top-left screen char

        iny
        cpy #40         ; Have we done all lines in the table?
        bne line_loop

        ; ----------------------------------------------------------------------
        ; ILLEGAL: ANC (AND Immediate + updates Carry bit 7)
        ; Cycles: 2 | Bytes: 2
        ; Performs AND, but moves bit 7 of result into Carry.
        ; Great for checking signs without destroying A's value via ASL/ROL.
        ; ----------------------------------------------------------------------
        lda #$80        ; Load a value with bit 7 set
        anc #$FF        ; AND with $FF. Result is $80. Bit 7 is 1, so Carry becomes 1.
        bcc failure     ; If Carry is clear (it shouldn't be), jump to failure.

        jmp main_loop   ; Loop forever

failure:
        inc BORDER      ; Flash border if ANC failed (should not happen)
        jmp failure


; ==============================================================================
; DATA
; ==============================================================================
        .align $100     ; Align to page boundary for clean timing
colors:
        ; A simple gradient pattern
        .byte 0,0,0,11,11,11,12,12,12,15,15,15,1,1,1,1,1,1,1,1
        .byte 15,15,15,12,12,12,11,11,11,0,0,0,0,0,0,0,0,0,0,0

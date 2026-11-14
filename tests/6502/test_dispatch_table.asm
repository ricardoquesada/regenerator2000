; ============================================================================
; 6502 RTS Trampoline (-1 Jump Table) Demo for C64
; Assembler: 64tasm
; Build:     64tasm -o test_dispatch_table.prg test_dispatch_table.asm
; ============================================================================

* = $0801

; BASIC Stub: 10 SYS 2061 ($080D)
.word ss, 10
.null $9e, format("%d", start)
ss .word 0

; ----------------------------------------------------------------------------
; Constants & Zero Page
; ----------------------------------------------------------------------------
BORDER_COLOR = $d020
RASTER_LINE  = $d012

state_idx    = $fb          ; Unused Zero Page address for current state (0..2)

; ----------------------------------------------------------------------------
; Main Program
; ----------------------------------------------------------------------------
start:
        sei                 ; Disable interrupts for clean raster polling
        lda #$00
        sta state_idx       ; Start at State 0

main_loop:
        ; Wait for raster line $ff so we run once per frame (~60Hz / 50Hz)
        lda #$ff
-       cmp RASTER_LINE
        bne -
-       cmp RASTER_LINE
        beq -

        ; Call our jump-table dispatcher as a normal subroutine.
        ; JSR pushes the return address back to 'main_loop' onto the stack.
        ; When the target routine finishes with RTS, it returns right here!
        jsr dispatch_state

        jmp main_loop

; ----------------------------------------------------------------------------
; RTS Trampoline Dispatcher
; ----------------------------------------------------------------------------
dispatch_state:
        ldx state_idx       ; Load current state index (0, 1, or 2)
        lda state_table_hi, x
        pha                 ; Push High Byte of (Target - 1)
        lda state_table_lo, x
        pha                 ; Push Low Byte of (Target - 1)
        rts                 ; Pop address, add 1, and jump to State_X!

; ----------------------------------------------------------------------------
; Jump Table (Target Addresses - 1)
; 64tasm lets us define the list of target addresses once and extract
; the low/high bytes using array notation.
; ----------------------------------------------------------------------------
TARGETS = (state_red - 1, state_green - 1, state_blue - 1)

state_table_lo:
        .byte <TARGETS      ; Expands to: <(state_red - 1), <(state_green - 1), ...

state_table_hi:
        .byte >TARGETS      ; Expands to: >(state_red - 1), >(state_green - 1), ...

; ----------------------------------------------------------------------------
; State Routines
; Notice that each routine ends with a normal RTS, returning directly to the
; caller of 'dispatch_state' (main_loop).
; ----------------------------------------------------------------------------
state_red:
        lda #$02            ; Color 2 = Red
        sta BORDER_COLOR
        lda #$01            ; Next state = 1
        sta state_idx
        rts

state_green:
        lda #$05            ; Color 5 = Green
        sta BORDER_COLOR
        lda #$02            ; Next state = 2
        sta state_idx
        rts

state_blue:
        lda #$06            ; Color 6 = Blue
        sta BORDER_COLOR
        lda #$00            ; Next state = 0 (wrap around)
        sta state_idx
        rts

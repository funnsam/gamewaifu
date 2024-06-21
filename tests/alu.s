SECTION "code", ROM0

MACRO test_alu
    ld a, \2
    push af
    ld b, \3
    \1 b

    ; save results
    push af
    pop bc
    ; test a
    ld a, b
    sub a, \4
    ld [RESULT], a
    ; test f
    ld a, c
    sub a, \5 << 4
    ld [RESULT], a

    ; test with imm
    pop af
    \1 \3

    ; save results
    push af
    pop bc
    ; test a
    ld a, b
    sub a, \4
    ld [RESULT], a
    ; test f
    ld a, c
    sub a, \5 << 4
    ld [RESULT], a
ENDM

MACRO set_c
    ld c, $10
    push bc
    pop af
ENDM

MACRO reset_c
    ld c, $00
    push bc
    pop af
ENDM

    total_tests 72

init:
    ld sp, stack.end

    test_alu add, 127, 178,  49, $3
    test_alu add, 209,  53,   6, $1
    test_alu add,   5,   3,   8, $0

    reset_c
    test_alu adc, 127, 178,  49, $3
    set_c
    test_alu adc, 237,  22,   4, $3
    reset_c
    test_alu add,   5,   3,   8, $0

    test_alu sub, 127, 178, 205, $5
    test_alu sub,  65,  25,  40, $6

    reset_c
    test_alu sbc, 127, 178, 205, $5
    set_c
    test_alu sbc,  20, 191,  84, $7

    test_alu and, 127, 178,  50, $2
    test_alu and, 252, 220, 220, $2

    test_alu xor, 127, 178, 205, $0
    test_alu xor,  25, 102, 127, $0

    test_alu  or, 127, 178, 255, $0
    test_alu  or,  86, 146, 214, $0

    test_alu  cp, 127, 178, 127, $5
    test_alu  cp,  65,  25,  65, $6

    done

stack:
    ds 16, 0
.end:

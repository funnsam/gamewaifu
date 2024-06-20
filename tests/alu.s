SECTION "code", ROM0

test_add:
    ld sp, test_stack
    ld a, 127
    ld b, 178
    add b
    push af
    pop bc
    ld a, c
    sub a, $30
    ld [0], a

    halt

test_stack:
    ds 16, 0

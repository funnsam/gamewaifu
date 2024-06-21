SECTION "code", ROM0

    total_tests 4

entry:
    ld sp, stack.end

.caller:
    call callee1
    done

callee1:
    pop hl
    push hl
    ld a, h
    sub a, entry.caller >> 8
    ld [RESULT], a
    ld a, l
    sub a, entry.caller + 3
    ld [RESULT], a

.caller:
    call callee2
    ret

    ld a, 1
    ld [RESULT], a

callee2:
    pop hl
    push hl
    ld a, h
    sub a, callee1.caller >> 8
    ld [RESULT], a
    ld a, l
    sub a, callee1.caller + 3
    ld [RESULT], a

    ret

    ld a, 1
    ld [RESULT], a

stack:
    ds 16, 0
.end:

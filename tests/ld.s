SECTION "code", ROM0

    total_tests 8

test_imm_r8:
    ld a, [test_data]
    sub a, $ad
    ld [RESULT], a

    ld a, [test_data + 1]
    sub a, $de
    ld [RESULT], a

test_hl_r8:
    ld hl, test_data
    ld a, [hli]
    sub a, $ad
    ld [RESULT], a

    ld a, [hld]
    sub a, $de
    ld [RESULT], a

    ld a, [hl]
    sub a, $ad
    ld [RESULT], a

    ld a, h
    sub a, test_data >> 8
    ld [RESULT], a
    ld a, l
    sub a, test_data
    ld [RESULT], a

test_ldh:
    ldh a, [RESULT]
    sub a, 7
    ld [RESULT], a

    done

    dw $baad, $1dea
test_data:
    dw $dead, $beef

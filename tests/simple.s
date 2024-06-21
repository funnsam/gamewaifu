SECTION "code", ROM0

    total_tests 1

entry:
    xor a, a
    ld [RESULT], a
    done

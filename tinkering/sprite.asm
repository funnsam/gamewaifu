INCLUDE "hardware.inc"

SECTION "VBlank handler", ROM0[$0040]
    jp VBlank

SECTION "Header", ROM0[$0100]
    jp EntryPoint
    ;ds $150 - @, 0 ; Make room for the header

SECTION "Code", ROM0[$0150]
EntryPoint:
    ; Shut down audio circuitry
    ld a, 0
    ld [rNR52], a

    ; Do not turn the LCD off outside of VBlank
WaitVBlank:
    ld a, [rLY]
    cp 144
    jp c, WaitVBlank

    ; Turn the LCD off
    ld a, 0
    ld [rLCDC], a

    ; Copy the tile data
    ld de, Tiles
    ld hl, $8000
    ld bc, TilesEnd - Tiles
CopyTiles:
    ld a, [de]
    ld [hli], a
    inc de
    dec bc
    ld a, b
    or a, c
    jp nz, CopyTiles

    ; Copy the tilemap
    ld hl, $9800
    ld bc, 1024
CopyTilemap:
    xor a, a
    ld [hli], a
    dec bc
    ld a, b
    or a, c
    jp nz, CopyTilemap

    ; Clean OAM
    ld a, 0
    ld b, 160
    ld hl, _OAMRAM
ClearOam:
    ld [hli], a
    dec b
    jp nz, ClearOam

    ; Set up OAM
    ld hl, _OAMRAM
    ld a, 16
    ld [hli], a
    ld a, 8 + 8
    ld [hli], a
    ld a, 1
    ld [hli], a
    ld [hli], a

    ; Set palettes
    ld a, $e4
    ld [rOBP0], a
    ld [rBGP], a

    ; Turn the LCD on
    ld a, LCDCF_ON | LCDCF_BGON | LCDCF_BG8000 | LCDCF_OBJON | LCDCF_OBJ8
    ld [rLCDC], a

    ; Turn on VBlank interrupt
    ld a, IEF_VBLANK
    ld [rIE], a
    ei

:   jp :-

VBlank:
    ; incr sprite x, wrapping to x=-8 at x=160
    ld hl, _OAMRAM + OAMA_X
    ld a, 160 + 8
    cp a, [hl]
    jr z, :+
    inc [hl]
    jr :++
:   ld a, 0
    ld [hl], a
:

    ; incr counter
    ld hl, $9813
    inc [hl]

    reti

SECTION "Tile data", ROM0
Tiles:
    INCBIN "tileset.bin"
TilesEnd:

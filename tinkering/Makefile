%.o: %.asm tileset.bin
	rgbasm -o $@ $<

%.gb: %.o
	rgblink -o $@ $<
	rgbfix -v -p 0xFF $@

tileset.bin: tileset.png palette.txt
	rgbgfx -c hex:palette.txt -o $@ $<

%.bin:
	rgbasm $*.s -o $@
	rgblink $@ -o $@ -l link.ld

clean:
	- rm *.bin

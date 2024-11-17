#!/bin/sh

if [ ! -e blargg-tests ]; then
    git clone git@github.com:retrio/gb-test-roms.git blargg-tests --depth 1
fi

if [ ! -e mooneye-tests ]; then
    MOONEYE_V=mts-20240127-1204-74ae166
    wget "https://gekkio.fi/files/mooneye-test-suite/$MOONEYE_V/$MOONEYE_V.tar.xz" && {
        tar -xvf $MOONEYE_V.tar.xz
        mv $MOONEYE_V mooneye-tests
        rm $MOONEYE_V.tar.xz
    }
fi

if [ ! -e dmg-acid2.gb ]; then
    wget "https://github.com/mattcurrie/dmg-acid2/releases/download/v1.0/dmg-acid2.gb"
fi

if [ ! -e mealybug-tearoom-tests ]; then
    wget "https://raw.githubusercontent.com/mattcurrie/mealybug-tearoom-tests/master/mealybug-tearoom-tests.zip" && {
        unzip mealybug-tearoom-tests.zip -d mealybug-tearoom-tests
        rm mealybug-tearoom-tests.zip
    }
fi

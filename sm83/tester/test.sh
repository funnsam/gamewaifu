#!/bin/bash

set -o pipefail

mkdir -p logs || exit 1
cargo b -r || exit 1
failed=()

for f in "./GameboyCPUTests/v2/$1"*.json; do
    echo -e "     \x1b[34;1mTesting\x1b[0m \`$(basename $f)\`"
    log="./logs/$(basename $f).log"

    timeout 5 ./../../target/release/tester "$f" 2>&1 | head -n 1000 > $log && {
        rm $log &
    } || {
        failed+=("$(basename $f)")
    }
done

fails=${#failed[@]}

rm logs/summary.txt 2> /dev/null
if [[ $fails == 0 ]]; then
    echo -e "\x1b[1;32mAll tests passed 🎉\x1b[0m"
else
    if [[ $fails == 1 ]]; then
        echo -e "\x1b[1;31m$fails failed test\x1b[0m"
    else
        echo -e "\x1b[1;31m$fails failed tests\x1b[0m"
    fi

    for fail in "${failed[@]}"; do
        echo "    $fail"
        echo "$fail" >> logs/summary.txt
    done
fi

exit $fails

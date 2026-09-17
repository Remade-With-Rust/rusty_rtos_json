#!/bin/sh
# Generate the C arm of K7's coreJSON differential, and score the C itself.
#
# `core_json.c` is compiled VERBATIM out of the pinned checkout in the
# umbrella; this script never copies or edits it. Fetch it first with
#
#     cargo run --manifest-path tools/kairos/Cargo.toml -- oracle fetch --lib coreJSON
#
# The corpus is NOT fetched: test_parsing/ is vendored next to this script
# (JSONTestSuite @ 1ef36fa, MIT, licence alongside) so the differential runs in
# CI with no network. The trace it writes is checked in too, so the Rust side
# diffs it with no C toolchain either -- the same arrangement the kernel corpus
# and the heap differentials use.
#
# Two outputs, and the second is the one that made this package's target
# coherent: "agree with the C" and "pass JSONTestSuite" are the same goal only
# because coreJSON already scores 100 % on the suite. That was measured before
# a line was transcribed, and `score` re-checks it whenever the pin moves.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
lib="$here/../../oracle/coreJSON"
src="$lib/source/core_json.c"
corpus="$here/test_parsing"

[ -f "$src" ] || { echo "no core_json.c at $src -- run \`kairos oracle fetch --lib coreJSON\` first" >&2; exit 1; }
[ -d "$corpus" ] || { echo "no vendored corpus at $corpus" >&2; exit 1; }

cc -O2 -g -w -I "$lib/source/include" -o "$here/probe" "$src" "$here/probe.c"

case "${1:-trace}" in
trace)
    for f in "$corpus"/*.json; do
        printf '%s %s\n' "$(basename "$f")" "$("$here/probe" "$f")"
    done > "$here/corejson.trace"
    echo "wrote $(wc -l < "$here/corejson.trace") lines to $here/corejson.trace"
    ;;
score)
    # What does coreJSON score on the suite, by class?
    for cls in y n i; do
        acc=0
        rej=0
        for f in "$corpus"/${cls}_*.json; do
            if [ "$("$here/probe" "$f")" = "0" ]; then acc=$((acc + 1)); else rej=$((rej + 1)); fi
        done
        echo "${cls}_  accepted $acc  rejected $rej"
    done
    echo
    echo "--- y_ files coreJSON REJECTS (it must not) ---"
    for f in "$corpus"/y_*.json; do
        [ "$("$here/probe" "$f")" = "1" ] && basename "$f"
    done
    echo "--- n_ files coreJSON ACCEPTS (it must not) ---"
    for f in "$corpus"/n_*.json; do
        [ "$("$here/probe" "$f")" = "0" ] && basename "$f"
    done
    ;;
search)
    # The query differential's C arm. The file list is generated with LC_ALL=C
    # so the trace's ORDER is a property of the corpus, not of the locale the
    # machine happened to be in.
    cc -O2 -g -w -I "$lib/source/include"        -o "$here/search_driver" "$src" "$here/search_driver.c"
    ( cd "$corpus" && LC_ALL=C ls *.json ) > "$here/corpus.list"
    ( cd "$here" && ./search_driver corpus.list ) > "$here/search.trace"
    rm -f "$here/corpus.list"
    echo "wrote $(wc -l < "$here/search.trace") lines to $here/search.trace"
    ;;
*)
    echo "usage: run.sh [trace|score|search]" >&2
    exit 2
    ;;
esac

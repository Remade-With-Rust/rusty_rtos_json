/* The C arm of K7's coreJSON differential: one verdict per file.
 *
 * `core_json.c` is compiled VERBATIM out of the pinned checkout in the
 * umbrella; nothing here copies or edits it. This probe only answers the one
 * question the Rust side compares against.
 *
 * It exists as a file rather than as a command someone once ran, because the
 * measurement that made this package's target coherent was taken WITH it:
 * "match the C" and "pass JSONTestSuite" are the same goal only because
 * coreJSON already scores 100 % on the suite, and that has to be re-checkable
 * when the pin moves.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "core_json.h"

/* The corpus's largest file is n_structure_100000_opening_arrays.json. A
 * megabyte covers it with room to spare, and a static buffer keeps the probe
 * free of any allocator so it cannot be the thing that fails. */
static char buf[1 << 20];

int main(int argc, char **argv)
{
    if (argc < 2)
    {
        fprintf(stderr, "usage: probe <file.json>\n");
        return 2;
    }

    FILE *f = fopen(argv[1], "rb");
    if (f == NULL)
    {
        fprintf(stderr, "cannot open %s\n", argv[1]);
        return 2;
    }

    size_t n = fread(buf, 1, sizeof buf, f);
    fclose(f);

    /* 0 = accepted, 1 = rejected. The four JSONStatus_t refusals are
     * deliberately collapsed here: the trace answers "did the C accept it",
     * and the Rust side keeps the reasons distinct in its own tests. */
    JSONStatus_t s = JSON_Validate(buf, n);
    printf("%d\n", s == JSONSuccess ? 0 : 1);
    return 0;
}

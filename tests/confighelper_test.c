// Proof-of-plumbing test for the new CTest wiring — not an attempt at
// comprehensive coverage. Exercises strtodevsubsubtype (helper/confighelper.c),
// a pure string->enum mapping with no I/O/hardware, including the R8/R3
// aliasing onto SIMDEVSUBTYPE_MOZAR5 and the fallback for unrecognized input.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "../src/cargopit/helper/confighelper.h"

static int failures = 0;

static void check(const char* input, DeviceSubSubType expected)
{
    DeviceSettings ds = {0};
    strtodevsubsubtype(input, &ds);
    if (ds.dev_subsubtype != expected)
    {
        fprintf(stderr, "FAIL: strtodevsubsubtype(\"%s\") = %d, expected %d\n",
                input, ds.dev_subsubtype, expected);
        failures++;
    }
}

static void check_index(const char* path, int requested, int expected)
{
    int got = resolve_config_index(path, requested);
    if (got != expected)
    {
        fprintf(stderr, "FAIL: resolve_config_index(%s, %d) = %d, expected %d\n",
                path, requested, got, expected);
        failures++;
    }
}

static char* write_two_profiles(void)
{
    char tmpl[] = "/tmp/cargopit-config-index-XXXXXX";
    int fd = mkstemp(tmpl);
    if (fd < 0)
    {
        perror("mkstemp");
        return NULL;
    }
    const char* body =
        "configs = (\n"
        "  { sim = \"ac\"; car = \"one\"; devices = (); },\n"
        "  { sim = \"acc\"; car = \"two\"; devices = (); }\n"
        ");\n";
    if (write(fd, body, strlen(body)) < 0)
    {
        perror("write");
        close(fd);
        return NULL;
    }
    close(fd);
    return strdup(tmpl);
}

int main(void)
{
    check("MozaR5", SIMDEVSUBTYPE_MOZAR5);
    check("MozaR8", SIMDEVSUBTYPE_MOZAR5);   // aliased onto the same protocol as R5
    check("MozaR3", SIMDEVSUBTYPE_MOZAR5);   // same
    check("MozaNew", SIMDEVSUBTYPE_MOZA_NEW);
    check("MozaKSProWheel", SIMDEVSUBTYPE_MOZA_KS_PRO_WHEEL);
    check("CammusC5", SIMDEVSUBTYPE_CAMMUSC5);
    check("CammusC12", SIMDEVSUBTYPE_CAMMUSC12);
    check("LogitechG29", SIMDEVSUBTYPE_LOGITECH_G29);
    check("not-a-real-subtype", SIMDEVSUBTYPE_UNKNOWN);
    check("", SIMDEVSUBTYPE_UNKNOWN);

    char* path = write_two_profiles();
    if (path == NULL)
    {
        fprintf(stderr, "FAIL: could not write fixture\n");
        return 1;
    }
    check_index(path, 1, 1);
    check_index(path, CONFIG_INDEX_UNSET, CONFIG_INDEX_FIRST);
    check_index(path, 99, CONFIG_INDEX_UNSET);
    unlink(path);
    free(path);

    if (failures > 0)
    {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }

    printf("confighelper_test: all checks passed\n");
    return 0;
}

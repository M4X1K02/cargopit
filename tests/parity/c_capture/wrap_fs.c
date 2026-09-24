#define _GNU_SOURCE
#include "capture_log.h"

#include <glob.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern FILE* __real_fopen(const char* path, const char* mode);
extern int __real_glob(const char* pattern, int flags, int (*errfunc)(const char*, int), glob_t* pglob);

static ssize_t rumble_write(void* cookie, const char* buf, size_t size)
{
    (void)cookie;
    capture_log_op("sysfs_write", CAPTURE_FAKE_RUMBLE_PATH);
    capture_log_bytes("sysfs_value", buf, size);
    return (ssize_t)size;
}

static int rumble_close(void* cookie)
{
    (void)cookie;
    capture_log_op("sysfs_close", CAPTURE_FAKE_RUMBLE_PATH);
    return 0;
}

FILE* __wrap_fopen(const char* path, const char* mode)
{
    cookie_io_functions_t hooks = {
        .read = NULL,
        .write = rumble_write,
        .seek = NULL,
        .close = rumble_close,
    };

    if (path != NULL && strstr(path, "rumble") != NULL)
    {
        capture_log_op("fopen", path);
        return fopencookie(NULL, mode != NULL ? mode : "w", hooks);
    }
    return __real_fopen(path, mode);
}

int __wrap_glob(const char* pattern, int flags, int (*errfunc)(const char*, int), glob_t* pglob)
{
    if (pattern != NULL && strstr(pattern, "hid_fanatec") != NULL && pglob != NULL)
    {
        memset(pglob, 0, sizeof(*pglob));
        pglob->gl_pathc = 1;
        pglob->gl_pathv = calloc(2, sizeof(char*));
        if (pglob->gl_pathv == NULL)
        {
            return GLOB_NOSPACE;
        }
        pglob->gl_pathv[0] = strdup(CAPTURE_FAKE_RUMBLE_PATH);
        if (pglob->gl_pathv[0] == NULL)
        {
            free(pglob->gl_pathv);
            return GLOB_NOSPACE;
        }
        capture_log_op("glob", CAPTURE_FAKE_RUMBLE_PATH);
        return 0;
    }
    return __real_glob(pattern, flags, errfunc, pglob);
}

#define _GNU_SOURCE
#include "capture_log.h"

#include <errno.h>
#include <stdio.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

extern int __real_clock_gettime(clockid_t clk_id, struct timespec* tp);
extern int __real_gettimeofday(struct timeval* tv, void* tz);
extern int __real_usleep(useconds_t usec);

static uint64_t wrap_ns = CAPTURE_ORIGIN_NS;

void capture_clock_reset(void);

static void sync_from_log(void)
{
    wrap_ns = capture_clock_ms() * 1000000ULL;
}

int __wrap_clock_gettime(clockid_t clk_id, struct timespec* tp)
{
    char detail[64];

    if (tp == NULL)
    {
        errno = EFAULT;
        return -1;
    }
    if (clk_id != CLOCK_MONOTONIC && clk_id != CLOCK_MONOTONIC_RAW && clk_id != CLOCK_BOOTTIME)
    {
        return __real_clock_gettime(clk_id, tp);
    }
    sync_from_log();
    tp->tv_sec = (time_t)(wrap_ns / 1000000000ULL);
    tp->tv_nsec = (long)(wrap_ns % 1000000000ULL);
    snprintf(detail, sizeof(detail), "clk=%d ns=%llu", (int)clk_id,
             (unsigned long long)wrap_ns);
    capture_log_op("clock_gettime", detail);
    return 0;
}

int __wrap_gettimeofday(struct timeval* tv, void* tz)
{
    (void)tz;
    if (tv == NULL)
    {
        errno = EFAULT;
        return -1;
    }
    sync_from_log();
    tv->tv_sec = (time_t)(wrap_ns / 1000000000ULL);
    tv->tv_usec = (suseconds_t)((wrap_ns % 1000000000ULL) / 1000ULL);
    capture_log_op("gettimeofday", "virtual");
    return 0;
}

int __wrap_usleep(useconds_t usec)
{
    char detail[64];

    snprintf(detail, sizeof(detail), "us=%u", (unsigned)usec);
    capture_log_op("usleep", detail);
    capture_clock_advance_ms(((uint64_t)usec + 999ULL) / 1000ULL);
    return 0;
}

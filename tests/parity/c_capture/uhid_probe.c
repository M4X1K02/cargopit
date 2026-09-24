#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#define UHID_DEVICE_PATH "/dev/uhid"

int main(void)
{
    int fd = open(UHID_DEVICE_PATH, O_RDWR);
    if (fd < 0)
    {
        printf("uhid: unavailable (%s)\n", strerror(errno));
        return 0;
    }
    close(fd);
    printf("uhid: available\n");
    return 0;
}

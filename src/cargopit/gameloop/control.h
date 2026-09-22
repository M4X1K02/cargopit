#ifndef _CONTROL_H
#define _CONTROL_H

#include <stdbool.h>
#include <stddef.h>
#include <uv.h>

/*
 * Local control socket for a running play session. One newline-terminated
 * command per connection, answered with one line of JSON, then closed.
 * The TUI's tui/src/control.rs speaks the same protocol.
 */
#define CONTROL_SOCKET_NAME      "cargopit.sock"
#define CONTROL_SOCKET_FALLBACK  "/tmp/cargopit-%u.sock"
#define CONTROL_RUNTIME_DIR_ENV  "XDG_RUNTIME_DIR"
#define CONTROL_PATH_MAX         108
#define CONTROL_LINE_MAX         64

#define CONTROL_CMD_STATUS       "status"
#define CONTROL_CMD_RELOAD       "reload"
#define CONTROL_CMD_STOP         "stop"

/* Returns a malloc'd reply (without trailing newline) for one command. */
typedef char* (*ControlHandler)(void* ctx, const char* command);

typedef struct
{
    uv_pipe_t server;
    ControlHandler handler;
    void* ctx;
    char path[CONTROL_PATH_MAX];
    bool listening;
}
ControlServer;

int control_socket_path(char* out, size_t len);
/* Fails without touching the socket when another live session already owns it. */
int control_server_start(ControlServer* server, uv_loop_t* loop, ControlHandler handler, void* ctx);
void control_server_stop(ControlServer* server);

#endif

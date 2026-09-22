#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <unistd.h>

#include "control.h"
#include "../slog/slog.h"

#define CONTROL_BACKLOG         4
#define CONTROL_SOCKET_MODE     0600
#define CONTROL_NEWLINE         '\n'
#define CONTROL_REPLY_TOO_LONG  "{\"ok\":false,\"error\":\"command too long\"}"

typedef struct
{
    uv_pipe_t pipe;
    uv_write_t write_req;
    ControlServer* server;
    char line[CONTROL_LINE_MAX];
    size_t len;
    char* reply;
}
ControlClient;

int control_socket_path(char* out, size_t len)
{
    const char* runtime_dir = getenv(CONTROL_RUNTIME_DIR_ENV);
    int written;

    if (runtime_dir != NULL && runtime_dir[0] != '\0')
    {
        written = snprintf(out, len, "%s/%s", runtime_dir, CONTROL_SOCKET_NAME);
    }
    else
    {
        written = snprintf(out, len, CONTROL_SOCKET_FALLBACK, (unsigned) getuid());
    }
    return (written > 0 && (size_t) written < len) ? 0 : -1;
}

static bool socket_has_listener(const char* path)
{
    struct sockaddr_un addr;
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    bool live;

    if (fd < 0)
    {
        return false;
    }
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, path, sizeof(addr.sun_path) - 1);
    live = connect(fd, (struct sockaddr*) &addr, sizeof(addr)) == 0;
    close(fd);
    return live;
}

static void free_client(uv_handle_t* handle)
{
    ControlClient* client = handle->data;
    free(client->reply);
    free(client);
}

static void on_reply_written(uv_write_t* req, int status)
{
    ControlClient* client = req->data;
    (void) status;
    uv_close((uv_handle_t*) &client->pipe, free_client);
}

static void send_reply(ControlClient* client, char* reply)
{
    size_t len = reply != NULL ? strlen(reply) : 0;
    char* line = malloc(len + 2);

    uv_read_stop((uv_stream_t*) &client->pipe);
    if (line == NULL)
    {
        free(reply);
        uv_close((uv_handle_t*) &client->pipe, free_client);
        return;
    }
    memcpy(line, reply, len);
    line[len] = CONTROL_NEWLINE;
    line[len + 1] = '\0';
    free(reply);
    client->reply = line;
    uv_buf_t buf = uv_buf_init(line, (unsigned int) (len + 1));
    client->write_req.data = client;
    uv_write(&client->write_req, (uv_stream_t*) &client->pipe, &buf, 1, on_reply_written);
}

static void on_alloc(uv_handle_t* handle, size_t suggested_size, uv_buf_t* buf)
{
    ControlClient* client = handle->data;
    (void) suggested_size;
    buf->base = client->line + client->len;
    buf->len = sizeof(client->line) - client->len - 1;
}

static void on_read(uv_stream_t* stream, ssize_t nread, const uv_buf_t* buf)
{
    ControlClient* client = stream->data;
    char* newline;

    (void) buf;
    if (nread < 0)
    {
        uv_close((uv_handle_t*) stream, free_client);
        return;
    }
    client->len += (size_t) nread;
    client->line[client->len] = '\0';
    newline = strchr(client->line, CONTROL_NEWLINE);
    if (newline != NULL)
    {
        *newline = '\0';
        send_reply(client, client->server->handler(client->server->ctx, client->line));
        return;
    }
    if (client->len >= sizeof(client->line) - 1)
    {
        send_reply(client, strdup(CONTROL_REPLY_TOO_LONG));
    }
}

static void on_connection(uv_stream_t* server_stream, int status)
{
    ControlServer* server = server_stream->data;
    ControlClient* client;

    if (status < 0)
    {
        return;
    }
    client = calloc(1, sizeof(*client));
    if (client == NULL)
    {
        return;
    }
    client->server = server;
    uv_pipe_init(server_stream->loop, &client->pipe, 0);
    client->pipe.data = client;
    if (uv_accept(server_stream, (uv_stream_t*) &client->pipe) != 0)
    {
        uv_close((uv_handle_t*) &client->pipe, free_client);
        return;
    }
    uv_read_start((uv_stream_t*) &client->pipe, on_alloc, on_read);
}

int control_server_start(ControlServer* server, uv_loop_t* loop, ControlHandler handler, void* ctx)
{
    memset(server, 0, sizeof(*server));
    if (control_socket_path(server->path, sizeof(server->path)) != 0)
    {
        slogw("control socket path is too long; control socket disabled");
        return -1;
    }
    if (socket_has_listener(server->path))
    {
        slogw("another cargopit session owns %s; control socket disabled", server->path);
        return -1;
    }
    unlink(server->path);
    server->handler = handler;
    server->ctx = ctx;
    uv_pipe_init(loop, &server->server, 0);
    server->server.data = server;
    if (uv_pipe_bind(&server->server, server->path) != 0
        || uv_listen((uv_stream_t*) &server->server, CONTROL_BACKLOG, on_connection) != 0)
    {
        slogw("could not listen on %s; control socket disabled", server->path);
        uv_close((uv_handle_t*) &server->server, NULL);
        return -1;
    }
    chmod(server->path, CONTROL_SOCKET_MODE);
    server->listening = true;
    slogi("control socket listening on %s", server->path);
    return 0;
}

void control_server_stop(ControlServer* server)
{
    if (!server->listening)
    {
        return;
    }
    server->listening = false;
    uv_close((uv_handle_t*) &server->server, NULL);
    unlink(server->path);
}

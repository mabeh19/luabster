#include <lua.h>
#include <lauxlib.h>
#include <lualib.h>
#include <unistd.h>
#include <stdlib.h>
#include <string.h>


#define PIPE_READ 0
#define PIPE_WRITE 1

struct Child {
    pid_t pid;
    int stdin[2];
    int stdout[2];
    int stderr[2];
    char *cmd;
    int is_first;
    int is_last;
};

extern void run_lua(void* l, const char *cmd, int cmdlen);

struct Child lua_runner_spawn_command(const char *command, uint32_t len, int is_first, int is_last)
{
    struct Child c = {
        .cmd = strndup(command, len),
        .pid = -1,
        .is_first = is_first,
        .is_last = is_last,
    };

    if (!is_first)
        pipe(c.stdin);

    if (!is_last) {
        pipe(c.stdout);
        pipe(c.stderr);
    }

    return c;
}


struct Child lua_runner_run_command(void* l, struct Child *child)
{
    printf("Running command: %s\n", child->cmd);
    if ((child->pid = fork()) == 0)
    {
        if (!child->is_first)
            dup2(child->stdin[PIPE_READ], STDIN_FILENO);
        if (!child->is_last)
        {
            dup2(child->stdout[PIPE_WRITE], STDOUT_FILENO);
            dup2(child->stderr[PIPE_WRITE], STDERR_FILENO);
        }

        if (!child->is_first)
            close(child->stdin[PIPE_WRITE]);
        if (!child->is_last)
        {
            close(child->stdout[PIPE_READ]);
            close(child->stderr[PIPE_READ]);
        }

        run_lua(l, child->cmd, strlen(child->cmd));

        // ..and exit
        exit(0);
    }

    if (!child->is_first)
        close(child->stdin[PIPE_READ]);
    if (!child->is_last)
    {
        close(child->stdout[PIPE_WRITE]);
        close(child->stderr[PIPE_WRITE]);
    }

    free(child->cmd);

    return *child;
}


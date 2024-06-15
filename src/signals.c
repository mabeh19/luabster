#include <stdio.h>
#include <stdlib.h>
#include <signal.h>
#include <string.h>
#include <errno.h>
#include <sys/wait.h>


extern void parser_kill(void *, int);
extern void parser_stop(void*, int);
extern void parser_child_reaped(void*, int);

static void sig_handler(int sig);
static void sigstop_handler(int sig);
static void sigchld_handler(int sig);

static volatile void *parser;

const int sig_CONT = SIGCONT;
const int sig_STOP = SIGSTOP;
const int PROCESS_EXITED = -1;
const int PROCESS_STOPPED = SIGSTOP;
const int PROCESS_RUNNING = 0;

void signal_setup(void *p)
{
    parser = p;
    struct sigaction act = {
        .sa_handler = sig_handler,
        .sa_flags = 0,
    };
    sigemptyset(&act.sa_mask);

    if (sigaction(SIGINT, &act, NULL)) {
        printf("Failed to bind SIGINT: %s\n", strerror(errno));
    }
    if (sigaction(SIGQUIT, &act, NULL)) {
        printf("Failed to bind SIGQUIT: %s\n", strerror(errno));
    }
    act.sa_handler = sigstop_handler;
    if (sigaction(SIGTSTP, &act, NULL)) {
        printf("Failed to bind SIGTSTP: %s\n", strerror(errno));
    }

    act.sa_handler = sigchld_handler;
    if (sigaction(SIGCHLD, &act, NULL)) {
        printf("Failed to bind SIGCHLD: %s\n", strerror(errno));
    }
}

int try_wait_process(pid_t pid)
{
    int status;
    int id = waitpid(pid, &status, WNOHANG | WUNTRACED);

    if (id == -1) {
        switch (errno) {
        case ECHILD:
            return PROCESS_EXITED;
        default:
            break;
        }
    }
    if (WIFEXITED(status)) {
        return PROCESS_EXITED;
    }
    else if (WIFSTOPPED(status)) {
        return PROCESS_STOPPED;
    }
    else {
        return PROCESS_RUNNING;
    }
}

int signal_is_stopped(pid_t *pids, unsigned len)
{
    int is_stopped = 0;
    for (unsigned i = 0U; i < len; i++) {
        int status;
        waitpid(pids[i], &status, WNOHANG | WUNTRACED);

        is_stopped |= WIFSTOPPED(status);
    }
    return is_stopped;
}

void sig_kill(unsigned pid, int sig)
{
    kill(pid, sig);
}


void enter_critical_section()
{
    sigset_t sigs;
    sigemptyset(&sigs);
    sigaddset(&sigs, SIGCHLD);
    sigaddset(&sigs, SIGINT);

    sigprocmask(SIG_BLOCK, &sigs, NULL);
}


void exit_critical_section()
{
    sigset_t sigs;
    sigemptyset(&sigs);
    sigaddset(&sigs, SIGCHLD);
    sigaddset(&sigs, SIGINT);
    sigprocmask(SIG_UNBLOCK, &sigs, NULL);
}

static void sig_handler(int sig)
{
    if (!parser) return;
    parser_kill((void*)parser, sig);
}


static void sigstop_handler(int sig)
{
    if (!parser) return;
    parser_stop((void*)parser, sig);
}

static void sigchld_handler(int sig)
{
    if (!parser) return;
    
    const int ALL = -1;
    int status;

    // reap all zombies
    for (;;) {
        int res = waitpid(ALL, &status, WNOHANG);
        if (WIFEXITED(status) && res > 0)
            parser_child_reaped((void*)parser, res);
        else 
            break;
    }

}

#include <stdio.h>
#include <stdlib.h>
#include <signal.h>
#include <string.h>
#include <errno.h>


extern void parser_kill(void *, int);
extern void parser_stop(void*, int);

static void sig_handler(int sig);
static void sigstop_handler(int sig);

static volatile void *parser;

const int sig_CONT = SIGCONT;

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
    act.sa_handler = sigstop_handler;
    if (sigaction(SIGTSTP, &act, NULL)) {
        printf("Failed to bind SIGTSTP: %s\n", strerror(errno));
    }
}


void sig_kill(unsigned pid, int sig)
{
    printf("Sending sig %d to %u\n", sig, pid);
    kill(pid, sig);
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

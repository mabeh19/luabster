#include <stdio.h>
#include <stdlib.h>
#include <signal.h>
#include <string.h>
#include <errno.h>


extern void parser_kill(void *, void (*)(unsigned, int), int);
extern void parser_stop(void*);

static void sig_handler(int sig);

static volatile void *parser;


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
    if (sigaction(SIGTSTP, &act, NULL)) {
        printf("Failed to bind SIGTSTP: %s\n", strerror(errno));
    }
}

static void forward_signal(unsigned pid, int sig)
{
    kill(pid, sig);
}

static void sig_handler(int sig)
{
    if (!parser) return;
    parser_kill((void*)parser, forward_signal, sig);
}


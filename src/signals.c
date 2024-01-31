#include <stdlib.h>
#include <signal.h>


extern void parser_kill(void *parser);

static void sigint_handler(int sig);


static void *parser;


void signal_setup(void *p)
{
    parser = p;
    struct sigaction act = {
        .sa_handler = sigint_handler
    };

    sigaction(SIGINT, &act, NULL);
}


static void sigint_handler(int sig)
{
    (void)sig;
    if (!parser) return;
    parser_kill(parser);
}

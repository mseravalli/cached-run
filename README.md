# Cached Run (caru)

## Description

This project allows to use the output of a previously run command instead of running the command itself.

It is useful when the output of the command is idempotent, but it takes a long time to execute e.g. `curl -X GET https://example.org/very_expensive_idempotent_op`.

Usege example:

```shell
$ echo 'sleep 3; echo hello' > test.sh
$ chmod 755 test.sh
$ time caru bash test.sh
hello
caru bash test.sh  0.01s user 0.01s system 0% cpu 3.046 total
$ time caru bash test.sh
hello
caru bash test.sh  0.00s user 0.00s system 38% cpu 0.013 total
```

## Installation

Can be installed using cargo:

```shell
cargo add cached-run
```

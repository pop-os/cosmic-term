#!/usr/bin/env bash

set -e

printf "   "
for background in $(seq 0 15)
do
    printf " %2d " "${background}"
done
printf "\n"

for foreground in $(seq 0 15)
do
    printf "%2d:" "${foreground}"
    printf "\x1B[38;5;${foreground}m"
    for background in $(seq 0 15)
    do
        printf "\x1B[48;5;${background}m"
        printf " %2d " "${background}"
        printf "\x1B[49m"
    done
    printf "\x1B[39m"
    printf "\n"
done

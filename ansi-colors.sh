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
    # Add each dim color right above corresponding normal color for easier visual comparisons
    if ((foreground < 8))
    then
        printf "d%d:" "${foreground}"
        printf "\x1B[2;$((foreground+30))m"

        for background in $(seq 0 15)
        do
            printf "\x1B[48;5;${background}m"
            printf " %2d " "${background}"
            printf "\x1B[49m"
        done
        printf "\x1B[0m"
        printf "\n"
    fi

    printf "%2d:" "${foreground}"
    printf "\x1B[38;5;${foreground}m"

    for background in $(seq 0 15)
    do
        printf "\x1B[48;5;${background}m"
        printf " %2d " "${background}"
        printf "\x1B[49m"
    done
    printf "\x1B[0m"
    printf "\n"
done

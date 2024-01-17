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

# Underlines

CNAMES=("BLK" "RED" "GRN" "YEL" "BLU" "MAG" "CYN" "WHT")

printf "\033[1m" # bold
printf "\nUnderline With FG Colors:\n"

printf "\033[4m" # underline
for foreground in $(seq 0 7)
do
    printf "\033[$((foreground+30))m ${CNAMES[$foreground]} "
done
printf "\x1B[24m\n" # no underline

printf "\nUnderline Styles And Colors:\n"

printf "\nFG:  "
printf "\033[9mStrikeout\033[0m "
printf "\033[4mUnderline\033[0m "
printf "\033[4:2mDoubleUnderline\033[0m "
printf "\033[4:3mCurlyUnderline\033[0m "
printf "\033[4:4mDottedUnderline\033[0m "
printf "\033[4:5mDashedUnderline\033[0m "
printf "\n"

printf "INV: "
printf "\033[7m\033[9mStrikeout\033[0m "
printf "\033[7m\033[4mUnderline\033[0m "
printf "\033[7m\033[4:2mDoubleUnderline\033[0m "
printf "\033[7m\033[4:3mCurlyUnderline\033[0m "
printf "\033[7m\033[4:4mDottedUnderline\033[0m "
printf "\033[7m\033[4:5mDashedUnderline\033[0m "
printf "\n"

for line_color in $(seq 0 7)
do
    printf "${CNAMES[$line_color]}: "
    printf "          "
    printf "\033[58:5:"${line_color}m
    printf "\033[4mUnderline\033[0m "
    printf "\033[58:5:"${line_color}m
    printf "\033[4:2mDoubleUnderline\033[0m "
    printf "\033[58:5:"${line_color}m
    printf "\033[4:3mCurlyUnderline\033[0m "
    printf "\033[58:5:"${line_color}m
    printf "\033[4:4mDottedUnderline\033[0m "
    printf "\033[58:5:"${line_color}m
    printf "\033[4:5mDashedUnderline\033[0m "
    printf "\n"
done

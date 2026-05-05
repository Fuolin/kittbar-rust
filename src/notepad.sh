#!/bin/bash

NOTE_FILE="$NOTE_FILE"

stty -echo -icanon

trap '' 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
stty -isig -icanon -echo >/dev/null 2>&1
tput civis
clear

tput home
cat "$NOTE_FILE"

# 事件监听：文件被修改才刷新
while true; do
  inotifywait -qq -e modify "$NOTE_FILE"
  clear
  tput home
  cat "$NOTE_FILE"
done
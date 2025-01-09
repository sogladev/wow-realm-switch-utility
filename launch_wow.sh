#!/usr/bin/bash
INFO=$1
if [ -z "$1" ]
  then
      echo "Pick a config!"
    exit 0
fi
CONFIG="${HOME}/.secrets/launch_wow.config"
L=$(grep -Fin $INFO $CONFIG | head -1 | cut --delimiter=":" --fields=1)
INFO=$(sed -n ${L}p $CONFIG)
L=$((L+1))
GAME_FOLDER=$(sed -n ${L}p $CONFIG)
L=$((L+1))
REALMLIST_REL_PATH=$(sed -n ${L}p $CONFIG)
L=$((L+1))
LAUNCH_CMD=$(sed -n ${L}p $CONFIG)
L=$((L+1))
REALMLIST=$(sed -n ${L}p $CONFIG)
L=$((L+1))
USERNAME=$(sed -n ${L}p $CONFIG)
L=$((L+1))
PASSWORD=$(sed -n ${L}p $CONFIG)
if [ $# -eq 1 ]; then
    echo Changing Realmlist.wtf to $REALMLIST
    echo $REALMLIST > "${GAME_FOLDER}/${REALMLIST_REL_PATH}"
fi
# Optional xdotool to input username and password
# sleep 12 && xdotool type $USERNAME &
# sleep 13 && xdotool key Tab && xdotool type $PASSWORD &
# sleep 14 && xdotool key KP_Enter &
echo "Launching ${INFO} WoW.exe autologin in 12 seconds"
$LAUNCH_CMD

#!/usr/bin/bash
GAME="/media/${USER}/Data/games/${FOLDER}"
LAUNCH_GAME="env LUTRIS_SKIP_INIT=1 lutris lutris:rungameid/${ID}"
INFO=$1
if [ -z "$1" ]
  then
      $LAUNCH_GAME
    exit 0
fi
CONFIG="/home/${USER}/.secrets/launch_wow.config"
L=$(grep -Fin $INFO $CONFIG | head -1 | cut --delimiter=":" --fields=1)
INFO=$(sed -n ${L}p $CONFIG)
L=$((L+1))
REALMLIST=$(sed -n ${L}p $CONFIG)
L=$((L+1))
USER=$(sed -n ${L}p $CONFIG)
L=$((L+1))
PASSWORD=$(sed -n ${L}p $CONFIG)
echo Changing Realmlist.wtf to $REALMLIST
echo $REALMLIST > "${GAME}/Data/enUS/realmlist.wtf"
sleep 12 && xdotool type $USER & 
sleep 13 && xdotool key Tab && xdotool type $PASSWORD &
sleep 14 && xdotool key KP_Enter &
echo "Launching ${INFO} WoW.exe autologin in 20 seconds"
$LAUNCH_GAME

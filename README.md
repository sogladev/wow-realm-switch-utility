<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://github.com/SoglaHash/wow-autologin">
    <img src="logo192.png" alt="Logo" width="80" height="80">
  </a>

  <h3 align="center">Autologin WoW lutris/wine</h3>
</div>

# Autologin WoW lutris/wine
This project allows to set aliases in `.bash_aliases` to load a config and
launch the chosen version of wow, set realmlist and auto-login

```mermaid
graph TD;
    cmd-->bash_alias-->launch_wow.sh--set realmlist-->wow--sleep-->type_credentials
    launch_wow.config-->launch_wow.sh
    launch_wow.config[(launch_wow.config)]
    cmd[$ WOWC]
```

![GUI Screenshot][gui-screenshot]

## Requirements
A version of WoW installed and able to run a launch command with wine/lutris/..

## Warning
This script does `NOT` send keystrokes to a specific `WoW.exe` window. It types
the credentials as you would on a keyboard with `xdotool` after a set delay to
the window with focus.

## Setup
3 files need to be setup: .bash_alias, a launch_wow.sh script and launch_wow.config script.

Adding or removing requires setting up a new alias and adding the configuration to the config file.

### `launch_wow.config`
Each line respectively contains: description, game_folder, realmlist_rel_path, launch_cmd, realmlist, username, passwd


```
Local <-- string to reference in launch_wow.sh
/media/jelle/Data/games/wow335 <-- game folder containing WoW.exe
Data/enUS/realmlist.wtf  <-- relative path to game folder
env LUTRIS_SKIP_INIT=1 lutris lutris:rungameid/2
set realmlist 127.0.0.1
myusername
mypassword
```

`env LUTRIS_SKIP_INIT=1 lutris lutris:rungameid/2` may be replaced with `wine wow.exe`

Find game ID (for `rungameid/2`) with
 ```
 lutris -l
 ```

### `.bash_aliases`
Set a 2nd argument to skip writing realmlist. `.launch_wow.config` must contain a line for 
```
alias WOWL="/home/${USER}/scripts/launch_wow.sh Local"
alias WOWM="/home/${USER}/scripts/launch_wow.sh Mistblade skip_write_realmlist"
```

### `launch_wow.sh`
Change `sleep` timers as needed


## Usage
Open shell and type an alias
```
$ WOWL
```
then keep focus and wait for autologin

[gui-screenshot]: ./screenshot.png
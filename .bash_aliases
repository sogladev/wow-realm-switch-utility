wow_vs() {
    "$HOME/.local/bin/wowctl" "--config" "~/.config/wowctl/config.toml" "$@"
}
alias WOWC='wow_vs Chromiecraft'
alias WOWCHD='wow_vs ChromiecraftHD'
alias WOWL='wow_vs Local'
alias WOWL2='wow_vs Local2'
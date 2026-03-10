# fish completion for kubens
complete -c kubens -f

# Dynamic namespace names
complete -c kubens -f -n '__fish_use_subcommand' -a '(kubens 2>/dev/null | string replace -r "^[* ]*" "")'

# Flags
complete -c kubens -f -s c -l current -d 'Show current namespace'
complete -c kubens -f -s u -l unset -d 'Reset namespace to default'
complete -c kubens -f -s f -l force -d 'Switch without existence check' -r -a '(kubens 2>/dev/null | string replace -r "^[* ]*" "")'
complete -c kubens -f -l fzf -d 'Use external fzf for selection'
complete -c kubens -f -s h -l help -d 'Show help'
complete -c kubens -f -s V -l version -d 'Show version'

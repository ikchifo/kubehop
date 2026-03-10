# fish completion for kubectx
complete -c kubectx -f

# Dynamic context names
complete -c kubectx -f -n '__fish_use_subcommand' -a '(kubectx 2>/dev/null | string replace -r "^[* ]*" "")'

# Flags
complete -c kubectx -f -s c -l current -d 'Show current context name'
complete -c kubectx -f -s d -l delete -d 'Delete a context' -r -a '(kubectx 2>/dev/null | string replace -r "^[* ]*" "")'
complete -c kubectx -f -s u -l unset -d 'Unset the current context'
complete -c kubectx -f -l fzf -d 'Use external fzf for selection'
complete -c kubectx -f -s h -l help -d 'Show help'
complete -c kubectx -f -s V -l version -d 'Show version'
complete -c kubectx -f -l completion -d 'Output shell completion script' -r -a 'bash zsh fish'

# pick subcommand
complete -c kubectx -f -n '__fish_use_subcommand' -a pick -d 'Interactive picker (k9s plugin)'
complete -c kubectx -f -n '__fish_seen_subcommand_from pick' -l switch -d 'Switch context after picking'
complete -c kubectx -f -n '__fish_seen_subcommand_from pick' -l kubeconfig -d 'Kubeconfig file path' -r
complete -c kubectx -f -n '__fish_seen_subcommand_from pick' -l current -d 'Current context name' -r

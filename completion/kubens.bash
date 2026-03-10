# bash completion for kubens
_kubens() {
    local cur prev
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    local flags="-c --current -u --unset -f --force --fzf -h --help -V --version"

    case "$prev" in
        -f|--force)
            COMPREPLY=( $(compgen -W "$(kubens 2>/dev/null | sed 's/^[* ]*//')" -- "$cur") )
            return
            ;;
    esac

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "$flags" -- "$cur") )
        return
    fi

    local namespaces
    namespaces="$(kubens 2>/dev/null | sed 's/^[* ]*//')"
    COMPREPLY=( $(compgen -W "$namespaces" -- "$cur") )
}

complete -F _kubens kubens

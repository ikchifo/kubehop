# bash completion for kubectx
_kubectx() {
    local cur prev
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    local flags="-c --current -d --delete -u --unset --fzf -h --help -V --version --completion"

    case "$prev" in
        -d|--delete)
            local contexts
            contexts="$(kubectx 2>/dev/null | sed 's/^[* ]*//')"
            COMPREPLY=( $(compgen -W "$contexts" -- "$cur") )
            return
            ;;
        --completion)
            COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
            return
            ;;
    esac

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "$flags" -- "$cur") )
        return
    fi

    if [[ "${COMP_WORDS[1]}" == "pick" ]]; then
        local pick_flags="--switch --kubeconfig --current"
        COMPREPLY=( $(compgen -W "$pick_flags" -- "$cur") )
        return
    fi

    local contexts
    contexts="$(kubectx 2>/dev/null | sed 's/^[* ]*//')"
    COMPREPLY=( $(compgen -W "$contexts pick" -- "$cur") )
}

complete -F _kubectx kubectx

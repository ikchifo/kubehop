#compdef kubectx

_kubectx() {
    local -a contexts flags

    flags=(
        '-c:show current context name'
        '--current:show current context name'
        '-d:delete a context'
        '--delete:delete a context'
        '-u:unset the current context'
        '--unset:unset the current context'
        '--fzf:use external fzf for selection'
        '-h:show help'
        '--help:show help'
        '-V:show version'
        '--version:show version'
        '--completion:output shell completion script'
    )

    if (( CURRENT == 2 )); then
        _alternative \
            'flags:flags:_describe "flag" flags' \
            'contexts:contexts:_kubectx_contexts' \
            'commands:commands:(pick)'
        return
    fi

    case "${words[2]}" in
        -d|--delete)
            _kubectx_contexts
            ;;
        --completion)
            _values 'shell' bash zsh fish
            ;;
        pick)
            local -a pick_flags
            pick_flags=(
                '--switch:switch context after picking'
                '--kubeconfig:kubeconfig file path'
                '--current:current context name'
            )
            _describe 'pick flag' pick_flags
            ;;
    esac
}

_kubectx_contexts() {
    local -a ctx_list
    ctx_list=("${(@f)$(kubectx 2>/dev/null | sed 's/^[* ]*//')}")
    _describe 'context' ctx_list
}

_kubectx "$@"

#compdef kubens

_kubens() {
    local -a namespaces flags

    flags=(
        '-c:show current namespace'
        '--current:show current namespace'
        '-u:reset namespace to default'
        '--unset:reset namespace to default'
        '-f:switch without existence check'
        '--force:switch without existence check'
        '--fzf:use external fzf for selection'
        '-h:show help'
        '--help:show help'
        '-V:show version'
        '--version:show version'
    )

    if (( CURRENT == 2 )); then
        _alternative \
            'flags:flags:_describe "flag" flags' \
            'namespaces:namespaces:_kubens_namespaces'
        return
    fi

    case "${words[2]}" in
        -f|--force)
            _kubens_namespaces
            ;;
    esac
}

_kubens_namespaces() {
    local -a ns_list
    ns_list=("${(@f)$(kubens 2>/dev/null | sed 's/^[* ]*//')}")
    _describe 'namespace' ns_list
}

_kubens "$@"

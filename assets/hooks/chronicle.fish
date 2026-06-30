# Chronicle shell hook — reports command completion to the daemon (UDP 127.0.0.1:9712)
# Sourced from ~/.config/fish/config.fish via `chronicle hook install`

if set -q CHRONICLE_HOOK_LOADED
    return
end
set -gx CHRONICLE_HOOK_LOADED 1

function __chronicle_send -a cmd exit_code dur cwd
    if not command -v python3 >/dev/null
        return
    end
    python3 -c "
import json, socket, sys
payload = json.dumps({
    'cmd': sys.argv[1],
    'exit_code': int(sys.argv[2]),
    'dur': int(sys.argv[3]),
    'cwd': sys.argv[4],
}).encode()
socket.socket(socket.AF_INET, socket.SOCK_DGRAM).sendto(payload, ('127.0.0.1', 9712))
" "$cmd" "$exit_code" "$dur" "$cwd" 2>/dev/null
end

function __chronicle_preexec --on-event fish_preexec
    set -g _CHRONICLE_CMD $argv[1]
    set -g _CHRONICLE_START (date +%s%3N)
end

function __chronicle_postexec --on-event fish_postexec
    if not set -q _CHRONICLE_CMD
        return
    end
    set -l end (date +%s%3N)
    set -l dur_ms (math $end - $_CHRONICLE_START)
    __chronicle_send "$_CHRONICLE_CMD" $status $dur_ms $PWD
    set -e _CHRONICLE_CMD _CHRONICLE_START
end

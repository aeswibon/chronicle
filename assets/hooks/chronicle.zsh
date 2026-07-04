# Chronicle shell hook — reports command completion to the daemon (UDP 127.0.0.1:9712)
# Sourced from ~/.zshrc via `chronicle hook install`

[[ -n "${CHRONICLE_HOOK_LOADED:-}" ]] && return
export CHRONICLE_HOOK_LOADED=1

__chronicle_send() {
  command -v python3 >/dev/null 2>&1 || return 0
  python3 - "$@" <<'PY' 2>/dev/null
import json, socket, sys
cmd, exit_code, dur, cwd, shell, tty, ppid = sys.argv[1:8]
payload = json.dumps({
    "cmd": cmd,
    "exit_code": int(exit_code),
    "dur": int(dur),
    "cwd": cwd,
    "shell": shell or None,
    "tty": tty or None,
    "ppid": int(ppid) if ppid and ppid.isdigit() else None,
}).encode()
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.sendto(payload, ("127.0.0.1", 9712))
PY
}

__chronicle_preexec() {
  _CHRONICLE_CMD="$1"
  _CHRONICLE_START=$EPOCHREALTIME
}

__chronicle_precmd() {
  [[ -z "${_CHRONICLE_CMD:-}" ]] && return
  local exit_code=$?
  local dur_ms
  dur_ms=$(python3 -c "print(int((${EPOCHREALTIME} - ${_CHRONICLE_START}) * 1000))" 2>/dev/null) || dur_ms=0
  __chronicle_send "$_CHRONICLE_CMD" "$exit_code" "$dur_ms" "$PWD" "${ZSH_NAME:-zsh}" "${TTY:-}" "$PPID"
  unset _CHRONICLE_CMD _CHRONICLE_START
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec __chronicle_preexec
add-zsh-hook precmd __chronicle_precmd

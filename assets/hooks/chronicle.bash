# Chronicle shell hook — reports command completion to the daemon (UDP 127.0.0.1:9712)
# Sourced from ~/.bashrc via `chronicle hook install --shell bash`

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

__chronicle_debug_trap() {
  [[ "${BASH_COMMAND}" == __chronicle_* ]] && return 0
  [[ -n "${_CHRONICLE_CMD:-}" ]] && return 0
  _CHRONICLE_CMD="${BASH_COMMAND}"
  _CHRONICLE_START=$(python3 -c 'import time; print(int(time.time()*1000))' 2>/dev/null || date +%s)
}

__chronicle_precmd() {
  [[ -z "${_CHRONICLE_CMD:-}" ]] && return 0
  local exit_code=$?
  local dur_ms=0
  if [[ -n "${_CHRONICLE_START:-}" ]]; then
    dur_ms=$(python3 -c "print(max(0, int(__import__('time').time()*1000) - int(${_CHRONICLE_START})))" 2>/dev/null) || dur_ms=0
  fi
  __chronicle_send "${_CHRONICLE_CMD}" "${exit_code}" "${dur_ms}" "${PWD}"
  unset _CHRONICLE_CMD _CHRONICLE_START
}

trap '__chronicle_debug_trap' DEBUG
if [[ -z "${PROMPT_COMMAND:-}" ]]; then
  PROMPT_COMMAND="__chronicle_precmd"
else
  case "${PROMPT_COMMAND}" in
    *__chronicle_precmd*) ;;
    *) PROMPT_COMMAND="__chronicle_precmd; ${PROMPT_COMMAND}" ;;
  esac
fi

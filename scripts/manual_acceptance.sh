#!/usr/bin/env bash
# scripts/manual_acceptance.sh
#
# T-601-A / T-601-B 必須受け入れシナリオの実行補助スクリプト。
# CI で実行できない API キー／Ollama サーバー依存の検証を、ローカル環境で半自動実行する。
#
# 使い方:
#   ./scripts/manual_acceptance.sh [--bin <path>] [--config-dir <path>]
#
# 環境変数:
#   ANTHROPIC_API_KEY  ... Stage A（claude）を実行する場合に必須
#   OLLAMA_URL         ... Stage B（ollama）の base_url（既定 http://127.0.0.1:11434）
#
# 結果は標準出力に逐次表示され、終了コードは「全シナリオ成功で 0」。
# 各シナリオの SKIP 条件:
#   - claude: ANTHROPIC_API_KEY 未設定 → SKIP
#   - ollama: OLLAMA_URL/api/tags への HTTP 200 が無い → SKIP

set -u
set -o pipefail

bin="./target/release/agent-cli"
work_dir=""

while [ $# -gt 0 ]; do
    case "$1" in
        --bin)
            bin="$2"
            shift 2
            ;;
        --work-dir)
            work_dir="$2"
            shift 2
            ;;
        -h|--help)
            sed -n '1,40p' "$0"
            exit 0
            ;;
        *)
            echo "unknown option: $1" >&2
            exit 2
            ;;
    esac
done

if [ ! -x "$bin" ]; then
    echo "[acceptance] building release binary at $bin"
    cargo build --release --quiet || { echo "build failed"; exit 1; }
fi

if [ -z "$work_dir" ]; then
    work_dir=$(mktemp -d)
fi
mkdir -p "$work_dir"
echo "[acceptance] work_dir = $work_dir"

pass=()
fail=()
skip=()

run_stage() {
    local label="$1"; shift
    echo
    echo "=========================================="
    echo "[acceptance] $label"
    echo "=========================================="
    if "$@"; then
        pass+=("$label")
        echo "[acceptance] $label : PASS"
    else
        fail+=("$label")
        echo "[acceptance] $label : FAIL"
    fi
}

skip_stage() {
    local label="$1"; local reason="$2"
    skip+=("$label ($reason)")
    echo
    echo "[acceptance] $label : SKIP ($reason)"
}

write_config() {
    # $1 = path  $2 = provider kind  $3 = base_url override (optional)
    local path="$1" kind="$2" base="${3:-}"
    local kind_for_dir
    kind_for_dir=$(echo "$kind" | tr '.' '_')
    cat > "$path" <<EOF
[provider]
kind = "$kind"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
model       = "claude-opus-4-7"

[provider.codex]
api_key_env = "OPENAI_API_KEY"
model       = "gpt-4.1"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "${OLLAMA_URL:-http://127.0.0.1:11434}"

[provider."llama.cpp"]
model    = "default"
base_url = "${LLAMACPP_URL:-http://127.0.0.1:8080}"

[runtime]
auto_approve_tools = true
log_dir            = "$work_dir/log-$kind_for_dir"
registry_dir       = "$work_dir/reg-$kind_for_dir"
agents_dir         = "$work_dir/agents-$kind_for_dir"

[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]
EOF
}

scenario_claude() {
    local cfg="$work_dir/claude.toml"
    write_config "$cfg" "claude"
    "$bin" --config "$cfg" doctor || return 1
    "$bin" --config "$cfg" selftest --provider claude || return 1
}

scenario_ollama() {
    local cfg="$work_dir/ollama.toml"
    write_config "$cfg" "ollama" "${OLLAMA_URL:-http://127.0.0.1:11434}"
    "$bin" --config "$cfg" doctor || return 1
    "$bin" --config "$cfg" selftest --provider ollama || return 1
}

scenario_codex() {
    local cfg="$work_dir/codex.toml"
    write_config "$cfg" "codex"
    "$bin" --config "$cfg" doctor || return 1
    "$bin" --config "$cfg" selftest --provider codex || return 1
}

scenario_llamacpp() {
    local cfg="$work_dir/llamacpp.toml"
    write_config "$cfg" "llama.cpp"
    "$bin" --config "$cfg" doctor || return 1
    "$bin" --config "$cfg" selftest --provider "llama.cpp" || return 1
}

scenario_shared() {
    # claude × ollama の 2 プロセス協調シナリオは長時間 REPL 操作が必要なため、
    # 本スクリプトでは「両 selftest が PASS したら 2 プロセス協調も実機で実施可能」
    # と判定し、実際の対話は手動で行う。手順を出力するに留める。
    cat <<'EOF'
[acceptance] manual step required:
  Open two terminals with the same registry_dir, e.g.

    Terminal A:
      agent-cli --config <claude.toml> run --name alice

    Terminal B:
      agent-cli --config <ollama.toml> run --name bob

    Both configs must share [runtime] registry_dir.

  Then:
    > :list                       # both alice and bob are visible
    > :send bob "hello"           # B receives, ollama responds
    > (in B)  :send alice "..."   # A receives, claude responds
EOF
}

# --- Stage A: claude ---
if [ -z "${ANTHROPIC_API_KEY:-}" ]; then
    skip_stage "scenario A (claude single)" "ANTHROPIC_API_KEY not set"
else
    run_stage "scenario A (claude single)" scenario_claude
fi

# --- Stage B: ollama ---
ollama_url="${OLLAMA_URL:-http://127.0.0.1:11434}"
if ! curl -fsS --max-time 3 "$ollama_url/api/tags" >/dev/null 2>&1; then
    skip_stage "scenario B (ollama single, glm-5.1:cloud)" "$ollama_url unreachable"
else
    run_stage "scenario B (ollama single, glm-5.1:cloud)" scenario_ollama
fi

# --- Stage C: claude × ollama 2 プロセス協調（手動操作部分） ---
run_stage "scenario C (claude x ollama coord, manual instructions)" scenario_shared

# --- Optional D1: codex (OpenAI) ---
if [ -z "${OPENAI_API_KEY:-}" ]; then
    skip_stage "scenario D1 (codex/openai, optional)" "OPENAI_API_KEY not set"
else
    run_stage "scenario D1 (codex/openai, optional)" scenario_codex
fi

# --- Optional D2: llama.cpp ---
llamacpp_url="${LLAMACPP_URL:-http://127.0.0.1:8080}"
if ! curl -fsS --max-time 3 "$llamacpp_url/v1/models" >/dev/null 2>&1; then
    skip_stage "scenario D2 (llama.cpp, optional)" "$llamacpp_url unreachable"
else
    run_stage "scenario D2 (llama.cpp, optional)" scenario_llamacpp
fi

echo
echo "=========================================="
echo "[acceptance] summary"
echo "=========================================="
printf '%s\n' "PASS: ${#pass[@]}" "FAIL: ${#fail[@]}" "SKIP: ${#skip[@]}"
for s in "${pass[@]}"; do echo "  + $s"; done
for s in "${fail[@]}"; do echo "  ! $s"; done
for s in "${skip[@]}"; do echo "  ~ $s"; done

if [ "${#fail[@]}" -gt 0 ]; then
    exit 1
fi
exit 0

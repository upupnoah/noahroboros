#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

export PATH="$HOME/.local/bin:$PATH"
AGENT_CMD="${AGENT_CMD:-agent}"

TAG="${TAG:-$(date +%b%d | tr '[:upper:]' '[:lower:]')}"
BRANCH="autoresearch/$TAG"
EXPERIMENTS=0
CLOUD=false
MODEL=""
TIMEOUT=300  # 5 min per agent invocation

usage() {
    cat <<EOF
Usage: ./run.sh [OPTIONS]

Modes:
  (no flags)              Interactive: launches Cursor CLI agent session
  --experiments N, -n N   Non-interactive: run N experiments via agent -p
  --cloud, -c             Cloud: push to Cursor Cloud Agent

Options:
  --tag TAG               Git branch tag (default: current date, e.g. mar21)
  --model MODEL           Model to use (e.g. claude-4-opus, gpt-5.2)
  --timeout SECS          Timeout per experiment in seconds (default: 300)
  -h, --help              Show this help

Examples:
  ./run.sh                          # interactive session
  ./run.sh -n 50                    # run 50 experiments non-interactively
  ./run.sh -n 100 --model gpt-5.2  # 100 experiments with specific model
  ./run.sh --cloud                  # push to cloud agent
  TAG=experiment1 ./run.sh -n 20    # custom branch tag
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case $1 in
        -n|--experiments) EXPERIMENTS="$2"; shift 2 ;;
        -c|--cloud) CLOUD=true; shift ;;
        --tag) TAG="$2"; BRANCH="autoresearch/$TAG"; shift 2 ;;
        --model) MODEL="$2"; shift 2 ;;
        --timeout) TIMEOUT="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

check_prerequisites() {
    local missing=false

    if ! command -v cargo &>/dev/null; then
        echo "ERROR: cargo not found. Install Rust: https://rustup.rs"
        missing=true
    fi

    if ! command -v "$AGENT_CMD" &>/dev/null; then
        echo "ERROR: Cursor CLI ($AGENT_CMD) not found."
        echo "Install: curl https://cursor.com/install -fsS | bash"
        missing=true
    fi

    if ! find data -name "*.csv" -type f 2>/dev/null | grep -q .; then
        echo "WARNING: No CSV files found in data/. Backtest needs historical data."
        echo "Run: cargo run --release -- download"
    fi

    if $missing; then
        exit 1
    fi
}

setup_branch() {
    if git rev-parse --verify "$BRANCH" &>/dev/null 2>&1; then
        echo "Branch $BRANCH already exists. Checking out..."
        git checkout "$BRANCH"
    else
        echo "Creating branch $BRANCH..."
        git checkout -b "$BRANCH"
    fi
}

build_project() {
    echo "Building project..."
    if ! cargo build --release 2>&1; then
        echo "ERROR: cargo build failed. Fix compilation errors before running experiments."
        exit 1
    fi
    echo "Build successful."
}

init_results() {
    mkdir -p experiments
    if [[ ! -f experiments/results.tsv ]]; then
        printf "commit\tscore\tsharpe\tmax_dd\tstatus\tdescription\n" > experiments/results.tsv
        echo "Initialized experiments/results.tsv"
    fi
}

DATA_DIR="${DATA_DIR:-data/1h}"

PROMPT_TEMPLATE='Read AGENTS.md for full context. You are in the middle of an autoresearch experiment loop on branch %s.

Read experiments/results.tsv to see past results. Read src/strategy/baseline.rs for the current strategy.

The strategy has 6 TOGGLEABLE Nunchi mechanisms (USE_DYNAMIC_THRESHOLD, USE_MACD,
USE_BB, USE_ATR_STOP, USE_COOLDOWN, USE_SIGNAL_FLIP). All start disabled.
Current baseline score: +1.851 with 4 active signals + RSI exit.

PRIORITY: Try enabling toggles ONE AT A TIME first, then parameter tuning, then
structural changes. Read the Strategy Ideas section in AGENTS.md for prioritized list.

Run the NEXT experiment:
1. Review what has been tried. Pick ONE change.
2. Edit src/strategy/baseline.rs with the change.
3. git add -A && git commit -m "experiment: <description>"
4. cargo build --release 2>&1 | tail -n 20
5. cargo run --release -- backtest -d %s > run.log 2>&1
6. grep "^score:\|^sharpe:\|^max_drawdown_pct:" run.log
7. If score improved (higher): keep. If worse: git reset --hard HEAD~1. Log to experiments/results.tsv.

Do exactly ONE experiment, then stop.'

run_interactive() {
    echo "=== Autoresearch Interactive Mode ==="
    echo "Branch: $BRANCH"
    echo ""
    echo "Launching Cursor CLI agent..."
    echo "The agent will read AGENTS.md and begin the experiment loop."
    echo ""

    local cmd="$AGENT_CMD --trust --yolo --sandbox disabled"
    [[ -n "$MODEL" ]] && cmd="$cmd --model $MODEL"
    $cmd "Read AGENTS.md and let's kick off a new autoresearch experiment run. Do the setup first."
}

run_noninteractive() {
    local n=$1
    echo "=== Autoresearch Non-Interactive Mode ==="
    echo "Branch: $BRANCH"
    echo "Experiments: $n"
    echo "Timeout: ${TIMEOUT}s per experiment"
    [[ -n "$MODEL" ]] && echo "Model: $MODEL"
    echo ""

    local prompt
    prompt=$(printf "$PROMPT_TEMPLATE" "$BRANCH" "$DATA_DIR")

    local model_flag=""
    [[ -n "$MODEL" ]] && model_flag="--model $MODEL"

    for i in $(seq 1 "$n"); do
        echo "--- Experiment $i/$n ($(date '+%H:%M:%S')) ---"

        $AGENT_CMD -p "$prompt" $model_flag --output-format text --trust --yolo --sandbox disabled > "experiments/agent_${i}.log" 2>&1 &
        local agent_pid=$!

        local waited=0
        local timed_out=false
        while kill -0 "$agent_pid" 2>/dev/null; do
            if [[ $waited -ge $TIMEOUT ]]; then
                kill "$agent_pid" 2>/dev/null
                wait "$agent_pid" 2>/dev/null
                echo "Experiment $i timed out after ${TIMEOUT}s."
                timed_out=true
                break
            fi
            sleep 5
            waited=$((waited + 5))
        done

        if [[ "$timed_out" = false ]]; then
            wait "$agent_pid" 2>/dev/null
            echo "Experiment $i completed."
        fi

        if [[ -f experiments/results.tsv ]]; then
            local count
            count=$(tail -n +2 experiments/results.tsv | wc -l | tr -d ' ')
            echo "Total experiments logged: $count"
        fi
        echo ""
    done

    echo "=== Run Complete ==="
    echo ""
    if [[ -f experiments/results.tsv ]]; then
        echo "Results summary:"
        cat experiments/results.tsv
    fi
}

run_cloud() {
    echo "=== Autoresearch Cloud Mode ==="
    echo "Branch: $BRANCH"
    echo "Pushing to Cursor Cloud Agent..."
    echo ""

    local cmd="$AGENT_CMD -c --trust --yolo --sandbox disabled"
    [[ -n "$MODEL" ]] && cmd="$cmd --model $MODEL"
    $cmd "Read AGENTS.md for full instructions. You are on branch $BRANCH. Do the setup, then run the experiment loop FOREVER. Do not stop. Do not ask for confirmation. Be autonomous."

    echo ""
    echo "Task pushed to cloud. Monitor at: https://cursor.com/agents"
}

# --- Main ---

check_prerequisites
setup_branch
build_project
init_results

if $CLOUD; then
    run_cloud
elif [[ $EXPERIMENTS -gt 0 ]]; then
    run_noninteractive "$EXPERIMENTS"
else
    run_interactive
fi

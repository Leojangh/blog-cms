#!/bin/bash
set -e

BASE_URL="http://127.0.0.1:3000"
DURATION=10        # seconds per test
CONCURRENCY=50     # concurrent connections
SERVER_PID=""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

cleanup() {
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

check_deps() {
    for cmd in wrk sqlite3 cargo; do
        if ! command -v "$cmd" &>/dev/null; then
            echo "Error: $cmd is required. Install it first."
            exit 1
        fi
    done
}

get_rss_kb() {
    # macOS: ps shows RSS in KB
    ps -o rss= -p "$1" 2>/dev/null | tr -d ' '
}

print_header() {
    echo ""
    echo -e "${BOLD}${CYAN}========================================${NC}"
    echo -e "${BOLD}${CYAN}  Blog CMS Benchmark Report${NC}"
    echo -e "${BOLD}${CYAN}  $(date '+%Y-%m-%d %H:%M:%S')${NC}"
    echo -e "${BOLD}${CYAN}========================================${NC}"
    echo ""
}

run_bench() {
    local name="$1"
    local path="$2"
    local method="${3:-GET}"
    local extra_args="${4:-}"

    echo -e "${BOLD}--- ${name} ---${NC}"
    echo -e "  URL: ${BASE_URL}${path}"
    echo -e "  Concurrency: ${CONCURRENCY}, Duration: ${DURATION}s"
    echo ""

    # Memory before
    local mem_before=$(get_rss_kb "$SERVER_PID")

    # Run wrk
    local wrk_output
    wrk_output=$(wrk -t4 -c${CONCURRENCY} -d${DURATION}s ${extra_args} "${BASE_URL}${path}" 2>&1)

    # Memory after
    local mem_after=$(get_rss_kb "$SERVER_PID")

    echo "$wrk_output"
    echo ""
    echo -e "  ${GREEN}Memory: before=${mem_before}KB after=${mem_after}KB delta=$((mem_after - mem_before))KB${NC}"
    echo ""
}

# ---- Main ----

check_deps
print_header

echo -e "${BOLD}[1/5] Building release binary...${NC}"
cd "$(dirname "$0")/.."
cargo build --release 2>&1 | tail -1

echo -e "${BOLD}[2/5] Seeding test data...${NC}"
bash bench/seed.sh

echo -e "${BOLD}[3/5] Starting server...${NC}"
ADMIN_PASS_HASH='$argon2id$v=19$m=19456,t=2,p=1$j9BBVCPtAeaS1IPSsG+knA$RdND3nLhrd5f8wbXamuf651fZOOzLlvuA9WNAOkZzZ0' \
SESSION_SECRET=bench-secret \
./target/release/blog-cms &
SERVER_PID=$!
sleep 1

# Verify server is up
if ! curl -sf "$BASE_URL" > /dev/null; then
    echo "Server failed to start"
    exit 1
fi

INITIAL_MEM=$(get_rss_kb "$SERVER_PID")
echo -e "  Server PID: ${SERVER_PID}"
echo -e "  ${GREEN}Initial RSS: ${INITIAL_MEM}KB ($(echo "scale=1; $INITIAL_MEM/1024" | bc)MB)${NC}"
echo ""

echo -e "${BOLD}[4/5] Running benchmarks...${NC}"
echo ""

# Test 1: Homepage (list all posts)
run_bench "GET / (post list, 100 posts)" "/"

# Test 2: Single post (markdown rendering)
run_bench "GET /post/test-post-50 (single post, markdown render)" "/post/test-post-50"

# Test 3: Login page (static template)
run_bench "GET /admin/login (login page)" "/admin/login"

# Test 4: 404 page
run_bench "GET /post/nonexistent (404)" "/post/nonexistent"

echo -e "${BOLD}[5/5] Final stats${NC}"
FINAL_MEM=$(get_rss_kb "$SERVER_PID")
echo -e "  ${GREEN}Final RSS: ${FINAL_MEM}KB ($(echo "scale=1; $FINAL_MEM/1024" | bc)MB)${NC}"
echo -e "  ${GREEN}Memory growth: $((FINAL_MEM - INITIAL_MEM))KB${NC}"
echo ""
echo -e "${BOLD}${CYAN}========================================${NC}"
echo -e "${BOLD}${CYAN}  Benchmark complete${NC}"
echo -e "${BOLD}${CYAN}========================================${NC}"

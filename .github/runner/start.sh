#!/bin/bash
# GitHub Actions Runner entrypoint
# 
# Environment variables:
#   GITHUB_URL     - GitHub instance URL (default: https://github.com/astra-hq)
#   GITHUB_TOKEN   - Runner registration token (required)
#   RUNNER_NAME    - Name for this runner (default: poly-runner-$(hostname))
#   RUNNER_LABELS  - Comma-separated labels (default: self-hosted,linux,ubuntu-22.04,x86_64)
#   RUNNER_GROUP   - Runner group name (optional)
#   RUNNER_WORKDIR - Work directory (default: /_work)

set -e

GITHUB_URL="${GITHUB_URL:-https://github.com/astra-hq}"
RUNNER_NAME="${RUNNER_NAME:-poly-runner-$(hostname)}"
RUNNER_LABELS="${RUNNER_LABELS:-self-hosted,linux,ubuntu-22.04,x86_64}"
RUNNER_WORKDIR="${RUNNER_WORKDIR:-/_work}"

# Check for required token
if [ -z "$GITHUB_TOKEN" ]; then
    echo "ERROR: GITHUB_TOKEN environment variable is required"
    echo ""
    echo "Get a runner token from:"
    echo "  https://github.com/organizations/astra-hq/settings/actions/runners/new"
    echo ""
    echo "Then run with:"
    echo "  docker run -e GITHUB_TOKEN=your_token_here ..."
    exit 1
fi

# Configure the runner if not already configured
if [ ! -f .runner ]; then
    echo "=== Configuring GitHub Actions Runner ==="
    echo "URL:         $GITHUB_URL"
    echo "Name:        $RUNNER_NAME"
    echo "Labels:      $RUNNER_LABELS"
    echo "Work dir:    $RUNNER_WORKDIR"
    echo ""

    ./config.sh \
        --url "$GITHUB_URL" \
        --token "$GITHUB_TOKEN" \
        --name "$RUNNER_NAME" \
        --labels "$RUNNER_LABELS" \
        --work "$RUNNER_WORKDIR" \
        --unattended \
        --replace

    echo "=== Runner configured ==="
fi

# Trap SIGINT and SIGTERM for clean shutdown
cleanup() {
    echo "Shutting down runner..."
    ./config.sh remove --token "$GITHUB_TOKEN" 2>/dev/null || true
    exit 0
}
trap cleanup SIGINT SIGTERM

# Start the runner
echo "=== Starting runner: $RUNNER_NAME ==="
./run.sh

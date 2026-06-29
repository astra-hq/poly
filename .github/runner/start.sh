#!/bin/bash
# GitHub Actions Runner entrypoint
# 
# Environment variables:
#   GITHUB_URL     - GitHub instance URL (default: https://github.com/astra-hq)
#   GITHUB_TOKEN   - Runner registration token (only needed for first registration)
#   RUNNER_NAME    - Name for this runner (default: poly-runner-$(hostname))
#   RUNNER_LABELS  - Comma-separated labels (default: self-hosted,linux,ubuntu-latest,arm64)
#   RUNNER_GROUP   - Runner group name (optional)
#   RUNNER_WORKDIR - Work directory (default: /_work)

set -e

GITHUB_URL="${GITHUB_URL:-https://github.com/astra-hq}"
RUNNER_NAME="${RUNNER_NAME:-poly-runner-$(hostname)}"
RUNNER_LABELS="${RUNNER_LABELS:-self-hosted,linux,ubuntu-latest,arm64}"
RUNNER_WORKDIR="${RUNNER_WORKDIR:-/_work}"

# Configure the runner if not already configured
if [ ! -f .runner ]; then
    if [ -z "$GITHUB_TOKEN" ]; then
        echo "ERROR: Runner not configured and GITHUB_TOKEN is not set."
        echo ""
        echo "First-time setup requires a token. Get one from:"
        echo "  https://github.com/organizations/astra-hq/settings/actions/runners/new"
        echo ""
        echo "Then run with:"
        echo "  docker run -e GITHUB_TOKEN=your_token_here ..."
        echo ""
        echo "On subsequent restarts, the token is not needed."
        exit 1
    fi

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
else
    echo "=== Runner already configured, starting existing runner ==="
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

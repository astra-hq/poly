#!/usr/bin/env bash
# Verification script for supported-app documentation
# Fails if any disallowed phrases remain in supported-app docs.

set -euo pipefail

ERRORS=0

# Files to check (supported-app docs only, NOT legacy backend)
FILES=(
  README.md
  frontend/README.md
  docs/guides/architecture.md
  docs/guides/GETTING_STARTED.md
  docs/kg/LOCAL_SETUP.md
  CLAUDE.md
)

echo "=== Checking for disallowed phrases in supported-app docs ==="

for file in "${FILES[@]}"; do
  if [[ ! -f "$file" ]]; then
    echo "SKIP: $file (not found)"
    continue
  fi

  # Disallowed: "Whisper or Parakeet" (implying both are supported for transcription)
  if grep -in "Whisper or Parakeet" "$file"; then
    echo "FAIL: $file contains 'Whisper or Parakeet'"
    ERRORS=$((ERRORS + 1))
  fi

  # Disallowed: "Ollama recommended" (Ollama is now advanced/external only)
  # We check for Ollama explicitly presented as the recommended provider.
  # "Local is recommended... Ollama is available as an advanced external option" is ALLOWED.
  if grep -Ein "Ollama\s*\(local\)\s*is recommended|Ollama is recommended" "$file"; then
    echo "FAIL: $file contains Ollama as recommended"
    ERRORS=$((ERRORS + 1))
  fi

done

if [[ $ERRORS -eq 0 ]]; then
  echo "PASS: No disallowed phrases found."
  exit 0
else
  echo "FAIL: $ERRORS disallowed phrase(s) found."
  exit 1
fi

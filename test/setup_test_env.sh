#!/bin/bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
FIXTURES_DIR="$SCRIPT_DIR/fixtures"
WORK_DIR="$SCRIPT_DIR/work"

echo "Setting up test environment in: $WORK_DIR"

if [ -d "$WORK_DIR" ]; then
    echo "  -> Removing old work directory."
    rm -rf "$WORK_DIR"
fi

echo "  -> Creating new work directory."
mkdir -p "$WORK_DIR"

echo "  -> Copying fixtures to work directory."
cp -r "$FIXTURES_DIR/original/." "$WORK_DIR/"
cp -r "$FIXTURES_DIR/diffs/." "$WORK_DIR/"

echo "-----------------------------------"
echo "Test environment is ready."
echo "You can now run mend against the files in '$WORK_DIR'"
echo "Example:"
echo "  cargo run -- '$WORK_DIR/Personne.java' '$WORK_DIR/chatgpt.diff'"
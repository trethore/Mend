#!/bin/bash

echo "Building project..."
cargo build --release

if [ $? -eq 0 ]; then
  echo "Build successful."
  echo "Installing mend to /usr/local/bin. Sudo privileges are required."
  sudo mv target/release/mend /usr/local/bin/
  if [ $? -eq 0 ]; then
    echo "Installation successful."
  else
    echo "Installation failed. Could not move the binary."
    exit 1
  fi
else
  echo "Build failed, not installing."
  exit 1
fi
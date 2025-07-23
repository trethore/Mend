#!/bin/bash

echo "Building project in release mode..."
cargo build --release

if [ $? -eq 0 ]; then
  echo "Build successful."
  echo "Installing 'mend' to /usr/local/bin/. Sudo privileges are required."
  sudo install -m 755 target/release/mend /usr/local/bin/
  if [ $? -eq 0 ]; then
    echo "Installation successful. You can now use 'mend' system-wide."
  else
    echo "Installation failed. Could not move the binary."
    exit 1
  fi
else
  echo "Build failed, not installing."
  exit 1
fi
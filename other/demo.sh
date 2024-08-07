#!/usr/bin/env bash

# Custom script for starting a tmux 2-pane asciinema recording.
# Intended for recording gday demos.
# Requires: asciinema, tmux
# Use ctrl+b <arrow key> to switch between panes.
# Use ctrl+d to end the recording.

# Create the demo folders if they don't exist yet.
mkdir tmp
cd tmp
mkdir peer_1
mkdir peer_2

# Start a new session detached
tmux new-session -d -s demo_session  
tmux split-window -h

# Set both panes to a custom prompt
tmux send-keys -t demo_session:0.0 'export PS1="\033[1;92mpeer 1: \033[0m"' C-m
tmux send-keys -t demo_session:0.0 'cd peer_1' C-m
tmux send-keys -t demo_session:0.0 'clear' C-m

tmux send-keys -t demo_session:0.1 'export PS1="\033[1;92mpeer 2: \033[0m"' C-m
tmux send-keys -t demo_session:0.1 'cd peer_2' C-m
tmux send-keys -t demo_session:0.1 'clear' C-m

# Select the left pane
tmux select-pane -t demo_session:0.0

# Start recording
asciinema rec -c "tmux attach -t demo_session" --overwrite demo.cast 

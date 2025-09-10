#!/bin/bash

# Some VPS providers charge per GB of network egress.
# The Gday Server only exchanges contacts, so it doesn't
# transmit lots of data to network.
#
# However, as always, there's the remote possibility of a
# DDOS causing excessive network usage on our VPS.
# 
# We can protect ourselves against this
# by limiting the VPS's data rate.
#
# This bash script limits data rate to 1 mbit/s with bursts allowed.
#
# To make this script run on every startup of the VPS, put
# the following into /etc/systemd/system/rate_limiter.service:

##########################################
# [Unit]
# Description=rate_limiter
# After=network.target
#
# [Service]
# Type=oneshot
# ExecStart=/root/rate_limiter.sh
# RemainAfterExit=yes
#
# [Install]
# WantedBy=multi-user.target
##########################################

# Then reload the dameon:
# 'sudo systemctl daemon-reload'
# 
# Enable the service so that it starts on boot:
# 'sudo systemctl enable rate_limiter'
#
# Start the service right now:
# 'sudo systemctl start rate_limiter'
#
# Verify the status of the service:
# 'sudo systemctl status rate_limiter'
#
# View service logs (-u specifies service, -f follows log in real time)
# 'sudo journalctl -u rate_limiter -f'


# Clear existing rules
tc qdisc del dev eth0 root 2>/dev/null

# Limit eth0 rate to 1 mbit/s.
# This should ensure monthly egress doesn't exceed ~330 GB.
tc qdisc add dev eth0 root tbf rate 1mbit burst 1mbit latency 100ms

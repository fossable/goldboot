#!/bin/bash

set -e
set -x

exec 1>&2

# Don't block forever if we don't have enough entropy
ln -sf /dev/urandom /dev/random

# Synchronize time
timedatectl set-ntp true

# Wait for time sync
while [ "$(timedatectl show --property=NTPSynchronized --value)" != "yes" ]; do
	sleep 5
done

# Wait for pacman-init to complete
while ! systemctl show pacman-init.service | grep SubState=exited; do
	sleep 5
done

# Use a wrapper to run the install
pacman -Sy --noconfirm --noprogressbar archinstall curl
archinstall --debug --silent --config <(curl -s "http://${GB_HTTP_HOST:?}:${GB_HTTP_PORT:?}/config.json") --creds <(curl -s "http://${GB_HTTP_HOST:?}:${GB_HTTP_PORT:?}/creds.json")

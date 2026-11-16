#!/bin/sh
set -eu

backing_mount=/run/ferrum-workspace
backing_file=$backing_mount/isolate.ext4
workspace=/var/lib/isolate

if [ "$(id -u)" -ne 0 ] || [ "$(uname -sm)" != "Linux aarch64" ]; then
    printf '%s\n' 'Run as root inside the dedicated ARM64 Linux worker.' >&2
    exit 1
fi
if mountpoint -q "$workspace" || mountpoint -q "$backing_mount"; then
    printf '%s\n' 'A Ferrum workspace mount already exists.' >&2
    exit 1
fi
if [ ! -d "$workspace" ] || [ -L "$workspace" ] || [ -n "$(ls -A "$workspace")" ]; then
    printf '%s\n' 'The Isolate workspace must be an empty directory.' >&2
    exit 1
fi

umask 077
modprobe loop
install -d -m 0755 "$backing_mount"
mount -t tmpfs -o size=256m,nr_inodes=64,mode=0755,nodev,nosuid,noexec tmpfs "$backing_mount"
trap 'umount "$backing_mount"' EXIT
install -m 0600 /dev/null "$backing_file"
truncate -s 256M "$backing_file"
mkfs.ext4 -q -F -O quota -E quotatype=usrquota -N 4096 "$backing_file"
mount -t ext4 -o loop,nodev,nosuid,usrquota "$backing_file" "$workspace"
trap - EXIT

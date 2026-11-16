# Local Ferrum worker on Apple Silicon

Run the web app, API, and PostgreSQL on macOS. Run the execution worker in a dedicated native ARM64 Debian 13 VM using Lima's Apple virtualization backend. The VM owns the Isolate sandbox and has no host-folder mounts. It reaches the same PostgreSQL database through a loopback SSH tunnel.

## Create and qualify the guest

Install Lima from the [official instructions](https://lima-vm.io/docs/installation/). From your Mac, start a native guest with at least two vCPUs and 2 GiB of RAM.

```sh
limactl start --name=ferrum-arm64 --vm-type=vz --arch=aarch64 \
  --cpus=2 --memory=3 --disk=12 --mount-none --containerd=none template:debian-13
```

Copy the Ferrum source into the guest without copying `.env`. Run the [Phase 0 host setup](phase0-linux-worker.md#run-on-a-fresh-vm) to install Isolate 2.7 and the language runtimes. Install the repo's pinned Rust toolchain through Rustup and install `build-essential` inside the guest.

Debian's ARM64 cloud kernel can lack `CONFIG_TMPFS_QUOTA`. In that case, use the guest setup below in place of the direct quota-enabled tmpfs mount from the Phase 0 guide. It creates a capped 256 MiB tmpfs with a root-owned ext4 image inside. Isolate applies per-user block and inode quotas on ext4. The kernel discards the image at shutdown. Install the systemd unit to recreate it at boot.

```sh
sudo install -m 0755 deployment/isolate-arm64-workspace.sh /usr/local/sbin/ferrum-isolate-workspace
sudo install -m 0644 deployment/ferrum-isolate-workspace.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now ferrum-isolate-workspace.service
```

Run the [Isolate real-host checks](phase3-isolate.md#qualify-the-adapter) and the [language conformance tests](phase5-languages.md#qualify-the-linux-worker) before letting the worker claim queued user submissions.

## Start or reconnect the worker

If Lima is installed under the local Ferrum application support directory, select that installation in each Mac terminal you use for the worker.

```sh
ferrum_lima_root="$HOME/Library/Application Support/Ferrum"
export PATH="$ferrum_lima_root/lima-tools/bin:$PATH"
export LIMA_HOME="$ferrum_lima_root/lima"
```

Start the submission-enabled API on the Mac. Leave PostgreSQL listening on loopback. Start the VM if necessary, then forward the Mac's database port to the guest using Lima's pinned SSH configuration. Replace `5432` with the port from the Mac's `FERRUM_DATABASE_URL` if it differs.

```sh
limactl start ferrum-arm64
ssh -F "${LIMA_HOME:-$HOME/.lima}/ferrum-arm64/ssh.config" \
  -o ControlMaster=no -o ControlPath=none -o ExitOnForwardFailure=yes \
  -N -T -R 127.0.0.1:15432:127.0.0.1:5432 lima-ferrum-arm64
```

Leave the SSH tunnel running. In the guest, set `FERRUM_DATABASE_URL` to the Mac API's database URL with the address changed to `127.0.0.1:15432`. Apply the worker migrations and start the worker as the ordinary guest user.

```sh
cargo run --locked --release -p ferrum-worker -- migrate
cargo run --locked --release -p ferrum-worker
```

For a background worker, install the built binary at `~/.local/bin/ferrum-worker` and copy `deployment/ferrum-worker-local.service` into `~/.config/systemd/user/` inside the guest. Store the guest database URL in `~/.config/ferrum/worker.env` with mode 0600. Enable the worker account's user manager with `sudo loginctl enable-linger "$(id -un)"` in the guest, then run `systemctl --user daemon-reload`. After establishing the tunnel, start the installed worker from a second Mac terminal.

```sh
limactl shell --workdir=/tmp ferrum-arm64 systemctl --user start ferrum-worker-local
limactl shell --workdir=/tmp ferrum-arm64 journalctl --user -u ferrum-worker-local -n 20 --no-pager
```

From the Mac, `curl -fsS http://127.0.0.1:4173/api/execution/status` reports `{"worker":"available"}` once the worker registers with Apalis.

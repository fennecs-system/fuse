# experiments with codex

A first implementation of a personal coding room: NixOS microVM + dependency-free
Rust HTTP server + mobile web UI. The harness, server, credentials and workspace
are inside the VM. The desktop runs QEMU, firewall, Caddy and Tailscale.

**Status: scaffold, not yet compiled or boot-tested.** This authoring environment
has no Nix, Rust, or KVM. Inputs are deliberately not fake-pinned: generate and
commit `flake.lock` on your desktop before building. Cargo has no dependencies.
The first milestone is the isolation foundation; do the acceptance checks below
before adding important credentials or code.

## 1. Configure and build

Requires an x86_64 NixOS desktop with hardware virtualization and `/dev/kvm`.
For ARM, change the flake system and check your hypervisor support.

1. Extract this repo, `git init`, `git add .`, then `nix flake lock`.
2. Put your desktop SSH **public** key into `nix/guest.nix` under
   `users.users.agent.openssh.authorizedKeys.keys`.
3. Check that `10.77.0.0/30` is unused, or change it consistently in both modules.
4. If you use the phone URL, set `AGENT_ORIGIN` to its exact HTTPS origin.
5. Run `nix flake check`, `nix develop -c cargo test`, and
   `nix build .#nixosConfigurations.agent.config.microvm.declaredRunner`.
6. Commit both lock files and the configuration.

Add the repo as a flake input to your **existing** desktop flake:

```nix
inputs.agent-room.url = "path:/absolute/path/to/agent-vm";
inputs.agent-room.inputs.nixpkgs.follows = "nixpkgs";
# Inside your desktop modules list:
modules = [
  # your existing hardware/configuration modules ...
  inputs.agent-room.nixosModules.default
  { services.agentVM.uplink = "enp5s0"; } # replace with your real uplink
];
```

Your desktop needs networkd enabled by this module; review compatibility with
NetworkManager, existing firewall, NAT and Caddy configuration. Do not replace
your desktop configuration with the guest configuration. Build the desktop
configuration first, review it, then `sudo nixos-rebuild switch --flake .#YOUR-HOST`.
The VM autostarts via `microvm@agent` and persists disks under its host service
working directory (normally `/var/lib/microvms/agent`).

## 2. Authenticate the harness and open the UI

```sh
ssh agent@10.77.0.2
cd /persist/workspace
# Codex needs a git repository for its normal repository checks.
git init
codex login --device-auth
cat /persist/home/control-token
```

Use `http://localhost:8787` on the desktop and paste that token. It is held in
browser memory, not local storage. Clone a real project into the workspace, or
ask Codex to create one there. The server always starts tasks in that directory;
no request-controlled executable or shell interpolation is used.

You can use desktop editors over SSH (no SSH agent forwarding), or copy changes
with `scp`. Give the guest its own narrowly scoped Git credential; do not copy
personal SSH private keys, your main Git token, or your home directory into it.
Codex authentication remains in the persistent guest home. Its credential and
control token are accessible to guest root, which is intentionally allowed.

The agent can install tools with `nix shell`, `nix profile install`, Python venvs,
npm, or guest sudo. Guest OS settings change at the next reviewed rebuild by
editing `nix/guest.nix`; ad hoc guest root filesystem changes are ephemeral.
The writable Nix overlay, home and workspace survive VM restarts.

## 3. GrapheneOS / Tailscale

On your desktop, join your tailnet with `sudo tailscale up`, then:

```sh
sudo tailscale serve --bg http://127.0.0.1:8787
tailscale serve status
```

Set guest `AGENT_ORIGIN` to the HTTPS origin reported by Serve, rebuild, and
restart the VM. Connect the GrapheneOS Tailscale app to the same tailnet and open
that HTTPS URL in Vanadium. Tailnet policy should grant only your intended user
or phone identity access to desktop TCP 443; test that another tailnet identity
is denied. Keep the app token as a second access check. No Funnel, public port
forward, Tailscale subnet route, or tailnet credentials in the VM are needed.

Telegram is deferred: a future host-side notification relay can send only a
completion notice and private UI link. Do not put Telegram command execution
or detailed coding logs into the control path.

## Trust boundary

* No host file shares, Nix daemon socket, desktop session sockets or SSH agent.
  The Nix store is a guest image plus private writable overlay.
* Guest root is in scope. An agent can compromise its VM, its own server and
  its own credentials. Recreate the VM after suspicious behavior.
* Host input blocks guest-initiated connections; established replies to host
  connections are allowed. Forwarding drops spoofed IPv4, IPv6, private LAN,
  link-local, multicast and Tailscale destinations, and non-uplink routes.
* Public internet egress is intentionally allowed for tools and model calls.
  A malicious agent can exfiltrate anything you put in its VM. Publicly addressed
  LAN hosts reachable via the uplink need additional destination deny rules.
* A hypervisor/kernel escape remains possible. Keep the host patched. Persistent
  disks are untrusted data; do not automatically run exported code on the host.
* Logs are rendered with `textContent` and a restrictive CSP. The UI is still
  served by the untrusted guest: use it in a browser profile with no valuable
  cookies/passwords, and never enter host secrets into it.
* No host admin endpoint exists. Agent-proposed flake edits require full human
  review including lock files and host modules before host builds/deployment.
* Cancellation kills the harness process group. Guest-root processes can escape
  it via a new session; stopping `microvm@agent` is the reliable emergency stop.
* Jobs are not resumable; after a server restart the UI reports idle. systemd
  cleans its service cgroup. Logs overwrite each run and are capped by a monitor,
  not a hard filesystem quota. VM disks and resource limits bound the larger risk.

## Acceptance checks on the actual desktop

```sh
nix flake check
nix develop -c cargo fmt
nix develop -c cargo fmt --check
nix develop -c cargo clippy -- -D warnings
nix develop -c cargo test
sudo systemctl status nftables microvm@agent caddy
sudo nft list table inet agent-guard
curl -i http://localhost:8787/api/status   # must be 401
```

From inside the VM, check that HTTPS to a public tool registry succeeds, while
new TCP connections to `10.77.0.1`, your desktop LAN address, your router,
`169.254.169.254`, and a `100.64.0.0/10` tailnet address fail. Repeat those tests
with guest sudo and its firewall disabled. Check IPv6 is denied as well.
Verify the UI works from desktop and phone, bad tokens and foreign Origins fail,
a second job returns 409, cancel works, and the VM restart preserves installed
Nix packages and workspace. Run an actual Codex task before relying on the adapter.

Inspect host forwarding rules too: deployments with additional forward chains
may need explicit acceptance of permitted agent uplink traffic; do not remove
the isolation drops to make it work. Verify the rules persist across host reboot.

See [docs/ROADMAP.md](docs/ROADMAP.md) for subsequent milestones.

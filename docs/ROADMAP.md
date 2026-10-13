# Roadmap

1. **Isolation foundation (this scaffold)**: flake, guest-owned writable Nix store,
   persistent private volumes, TAP network, host firewall, localhost proxy.
   Lock inputs and verify on the actual NixOS desktop before trusting the boundary.
2. **Coding loop (this scaffold)**: Codex stdin adapter, one bounded job at a time,
   authenticated responsive UI, logs, cancel, process-group cleanup.
3. **Phone (integration instructions)**: Tailscale Serve on desktop, exact origin,
   phone-only grants/ACLs, no Funnel, no tailnet credentials in guest.
4. **Reviewable self-modification**: clone this repository into the guest; agent
   commits proposed guest changes. Human fetches into a separate host checkout,
   reviews the complete flake and lock diff, builds, then deploys deliberately.
   No host rebuild endpoint or guest sudo on the host.
5. **Hardening before daily use**: boot/network integration tests, snapshots,
   per-project VMs and scoped credentials, output disk quotas, egress proxy,
   secret broker, signed artifact export, durable job DB and crash recovery.
6. **Optional Pi/Telegram**: Pi adapter after checking its pinned CLI contract;
   Telegram notifications only, with no shell/control commands or secret/log content.
   Telegram is not the private control channel.

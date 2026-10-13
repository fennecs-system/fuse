{ config, lib, pkgs, ... }: let
  cfg = config.services.agentVM;
in {
  options.services.agentVM.uplink = lib.mkOption {
    type = lib.types.str;
    description = "Desktop internet interface, e.g. enp5s0. Required; never guessed.";
  };
  config = {
    microvm.autostart = [ "agent" ];
    networking.useNetworkd = true;
    systemd.network.networks."30-agent" = {
      matchConfig.Name = "agent-tap";
      address = [ "10.77.0.1/30" ];
      networkConfig = { ConfigureWithoutCarrier = true; LinkLocalAddressing = "no"; IPv6AcceptRA = false; };
    };
    boot.kernel.sysctl."net.ipv4.ip_forward" = 1;
    networking.nftables.enable = true;
    networking.nat = {
      enable = true; internalInterfaces = [ "agent-tap" ]; externalInterface = cfg.uplink;
    };
    # Drops are host-enforced, even if guest root disables its own firewall.
    networking.nftables.tables.agent-guard = {
      family = "inet";
      content = ''
        chain input {
          type filter hook input priority -20; policy accept;
          iifname "agent-tap" meta nfproto ipv6 drop
          iifname "agent-tap" ip saddr != 10.77.0.2 drop
          iifname "agent-tap" ct state established,related accept
          iifname "agent-tap" drop
        }
        chain forward {
          type filter hook forward priority -20; policy accept;
          iifname "agent-tap" meta nfproto ipv6 drop
          iifname "agent-tap" ip saddr != 10.77.0.2 drop
          iifname "agent-tap" ip daddr { 0.0.0.0/8, 10.0.0.0/8, 100.64.0.0/10, 127.0.0.0/8, 169.254.0.0/16, 172.16.0.0/12, 192.168.0.0/16, 224.0.0.0/4, 240.0.0.0/4 } drop
          iifname "agent-tap" oifname != "${cfg.uplink}" drop
          oifname "agent-tap" ct state established,related accept
          oifname "agent-tap" drop
        }
      '';
    };
    services.caddy.enable = true;
    services.caddy.virtualHosts."http://:8787".extraConfig = ''
      bind 127.0.0.1
      reverse_proxy 10.77.0.2:8080
    '';
    services.tailscale.enable = true;
    # VM must not start if the host firewall fails to load.
    systemd.services."microvm@agent" = {
      requires = [ "nftables.service" ]; after = [ "nftables.service" "systemd-networkd.service" ];
    };
  };
}

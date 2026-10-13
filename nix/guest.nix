{ pkgs, ... }: {
  networking.hostName = "agent";
  system.stateVersion = "26.05";
  microvm = {
    hypervisor = "qemu";
    vcpu = 4; mem = 8192;
    shares = []; # No desktop paths or host store shared with the guest.
    interfaces = [{ type = "tap"; id = "agent-tap"; mac = "02:00:00:00:77:02"; }];
    writableStoreOverlay = "/nix/.rw-store";
    volumes = [
      { image = "agent-state.img"; mountPoint = "/persist"; size = 32768; }
      { image = "agent-nix.img"; mountPoint = "/nix/.rw-store"; size = 32768; }
    ];
  };
  networking.useDHCP = false;
  networking.interfaces.eth0.ipv4.addresses = [{ address = "10.77.0.2"; prefixLength = 30; }];
  networking.defaultGateway = "10.77.0.1";
  networking.nameservers = [ "1.1.1.1" "9.9.9.9" ];
  networking.enableIPv6 = false;
  networking.firewall.allowedTCPPorts = [ 22 8080 ];
  services.openssh = {
    enable = true;
    settings = { PasswordAuthentication = false; KbdInteractiveAuthentication = false; PermitRootLogin = "no"; };
  };
  nix.settings.experimental-features = [ "nix-command" "flakes" ];
  # The guest owns its own Nix daemon and writable store.
  nix.settings.trusted-users = [ "root" "agent" ];
  users.users.agent = {
    isNormalUser = true; home = "/persist/home"; createHome = true;
    extraGroups = [ "wheel" ];
    openssh.authorizedKeys.keys = [
      # Paste your DESKTOP SSH public key here (never its private key).
    ];
  };
  security.sudo.wheelNeedsPassword = false; # Guest root is explicitly in scope.
  environment.systemPackages = with pkgs; [ git curl jq bash coreutils nodejs python3 gcc gnumake codex ];
  systemd.tmpfiles.rules = [ "d /persist/workspace 0700 agent users -" ];
  systemd.services.agent-token = {
    wantedBy = [ "multi-user.target" ];
    before = [ "agent-server.service" ];
    serviceConfig = { Type = "oneshot"; User = "agent"; Group = "users"; UMask = "0077"; };
    script = ''
      if [ ! -s /persist/home/control-token ]; then
        ${pkgs.openssl}/bin/openssl rand -hex 32 > /persist/home/control-token
      fi
      chmod 600 /persist/home/control-token
    '';
  };
  systemd.services.agent-server = {
    wantedBy = [ "multi-user.target" ]; after = [ "network-online.target" ];
    wants = [ "network-online.target" ];
    requires = [ "agent-token.service" ];
    path = with pkgs; [ codex bash coreutils git nix nodejs python3 gcc gnumake sudo ];
    environment = {
      HOME = "/persist/home";
      AGENT_BIND = "0.0.0.0:8080";
      AGENT_WORKSPACE = "/persist/workspace";
      AGENT_TOKEN_FILE = "/persist/home/control-token";
      # Set to your exact https://desktop.<tailnet>.ts.net URL for phone POSTs.
      AGENT_ORIGIN = "http://localhost:8787";
    };
    serviceConfig = {
      User = "agent"; Group = "users";
      ExecStart = "/run/current-system/sw/bin/agent-vm-server";
      Restart = "on-failure"; UMask = "0077";
      KillMode = "control-group";
      MemoryMax = "7G"; TasksMax = 512;
    };
  };
}

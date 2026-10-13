{
  description = "Personal coding agent inside a NixOS microVM";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.microvm.url = "github:microvm-nix/microvm.nix";
  inputs.microvm.inputs.nixpkgs.follows = "nixpkgs";
  outputs = { self, nixpkgs, microvm }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs { inherit system; };
    server = pkgs.rustPlatform.buildRustPackage {
      pname = "agent-vm-server"; version = "0.1.0";
      src = self; cargoLock.lockFile = ./Cargo.lock;
    };
  in {
    packages.${system} = {
      default = server;
      inherit server;
    };
    devShells.${system}.default = pkgs.mkShell {
      packages = with pkgs; [ cargo rustc rustfmt clippy python3 ];
    };
    nixosConfigurations.agent = nixpkgs.lib.nixosSystem {
      inherit system;
      modules = [ microvm.nixosModules.microvm ./nix/guest.nix
        { environment.systemPackages = [ server ]; } ];
    };
    nixosModules.default = { ... }: {
      imports = [ microvm.nixosModules.host ./nix/host.nix ];
      microvm.vms.agent.config = {
        imports = [ ./nix/guest.nix ];
        environment.systemPackages = [ server ];
      };
    };
    checks.${system}.server = server;
  };
}

{ pkgs, ... }:

{
  packages = with pkgs; [
    bacon
    cargo-expand
    protobuf
  ];

  languages.rust.enable = true;
}

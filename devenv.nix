{ pkgs, ... }:

{
  packages = with pkgs; [
    bacon
    protobuf
  ];

  languages.rust.enable = true;
}

{ pkgs, ... }: {
  languages.rust = {
    enable = true;
    channel = "stable";
    components = [
      "rustc"
      "cargo"
      "clippy"
      "rustfmt"
      "rust-analyzer"
    ];
  };

  packages = with pkgs; [
    bacon
    cargo-expand
    cargo-nextest
    cargo-deny
    taplo
    nixd
    nixfmt
  ];

  git-hooks.hooks = {
    rustfmt.enable = true;
    clippy-strict = {
      enable = true;
      entry = "cargo clippy --workspace --all-targets -- -D warnings";
      language = "system";
      pass_filenames = false;
      files = "\\.rs$";
      stages = [ "pre-commit" ];
    };
    taplo.enable = true;
    nixfmt.enable = true;
    trim-trailing-whitespace.enable = true;
  };

  tasks."schema:generate" = {
    exec = "cargo run -p clepsydra-core --example generate-schema";
  };
}

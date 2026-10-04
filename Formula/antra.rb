class Antra < Formula
  desc "Stable HTTPS domains for local development — one command, no ports, no /etc/hosts"
  homepage "https://github.com/ifelse-codes/antra"
  version "0.6.6"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-aarch64-apple-darwin"
      sha256 "40d1d5fe3c58c8e7eea21916149eefce075009bb9a481f086bd4355ff2d597cc"
    else
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-x86_64-apple-darwin"
      sha256 "e35b71a2048a0606a302133098c363b9d71dcc2821be562b624ea13d0a8da788"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-aarch64-linux"
      sha256 "8ed6e300357844628ba1f834dea30653b0f4f8dead3728627d1d1472a3a7811d"
    else
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-x86_64-linux"
      sha256 "230a0b0ae0782322ebdc5a64de9be90e7cefcff735b2e58897b7c511cdaa9324"
    end
  end

  def install
    bin.install Dir["antra*"].first => "antra"
  end

  def caveats
    <<~EOS
      To trust the local CA for HTTPS (one-time setup, no sudo on macOS):

        antra trust --user-level

      This installs a local root CA into your login keychain.
      Linux/Windows: run 'sudo antra trust' instead.

      Quick start:

        antra run --domain myapp.localhost -- pnpm dev
        # Then open https://myapp.localhost

      Run 'antra doctor' to verify your setup.
    EOS
  end

  test do
    assert_match "antra", shell_output("#{bin}/antra --version")
  end
end

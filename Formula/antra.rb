class Antra < Formula
  desc "Stable HTTPS domains for local development — one command, no ports, no /etc/hosts"
  homepage "https://github.com/ifelse-codes/antra"
  version "0.6.4"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-aarch64-apple-darwin"
      sha256 "3f1b64e13f46a1a600e02a7820e695de6d4f58ddda9e06bc070142f472695a55"
    else
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-x86_64-apple-darwin"
      sha256 "fa4f06be5830f3e5962a53ef82d8ec52459c6710bd5bd505694b415cf3d1bd84"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-aarch64-linux"
      sha256 "87ef5d82c81a8e096f6e9220651231c6465fd62e0ee6aaf239745f30163e311c"
    else
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-x86_64-linux"
      sha256 "f97a5e5c0cd6fb33decad5d955160b17eb0e1f92724da27b195b2f5908f8e803"
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

class Antra < Formula
  desc "Stable HTTPS domains for local development — one command, no ports, no /etc/hosts"
  homepage "https://github.com/ifelse-codes/antra"
  version "0.6.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-aarch64-apple-darwin"
      sha256 "90fc8179aee91f4b6067f6408fd95b44df50d4684168ea62100f7ed59c32dce8"
    else
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-x86_64-apple-darwin"
      sha256 "b389d8d51a844c1b477b580899e709040cb2be3ad06a20b90de70019c549154b"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-aarch64-linux"
      sha256 "c106c882775d5d532a59065fe6d54215d86ef93b963cf8db7254dfaa277ddd36"
    else
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-x86_64-linux"
      sha256 "e88a0d0ba89a4a22fc5e2e1d390e6e68bfca017707267e85ef29bbf9db0cffb5"
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

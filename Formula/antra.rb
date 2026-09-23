class Antra < Formula
  desc "Stable HTTPS domains for local development — one command, no ports, no /etc/hosts"
  homepage "https://github.com/ifelse-codes/antra"
  version "0.4.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-aarch64-apple-darwin"
      sha256 "b7e47450a6b12b207f6983ce30eb62de6938891e36eda44633b5dc0abda41642"
    else
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-x86_64-apple-darwin"
      sha256 "4a201bcf8222b3a363e09693676632c79821b207c594683d68e918fe20821493"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-aarch64-linux"
      sha256 "4c5072d9e587c79c8c4ef7f6baab721bad04d47b40ec79e2b6dc46f232f3ecb2"
    else
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-x86_64-linux"
      sha256 "5c7bdfbaf3efe86a4f67e94e0e07ed390ed43b48f791dab18e30eaee9440a0a8"
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

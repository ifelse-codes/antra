class Antra < Formula
  desc "Stable HTTPS domains for local development — one command, no ports, no /etc/hosts"
  homepage "https://github.com/ifelse-codes/antra"
  version "0.2.4"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-aarch64-apple-darwin"
      sha256 "2a3e703c7eea53f036a215b7a363eb732ac667e8908ed5b6f209fb0ce1907b49"
    else
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-x86_64-apple-darwin"
      sha256 "005db4276481f4b917463e8669e05190634f77985df4bab7d7da884212c372f6"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-aarch64-linux"
      sha256 "4484095774a1cdeef1b7eff15f9f16827b9b466c5c3994c28c07b3bd8999cd00"
    else
      url "https://github.com/ifelse-codes/antra/releases/download/v#{version}/antra-x86_64-linux"
      sha256 "e99c15fda77018abd78d5efffc0f79068cb2b2bb3aa4f9b0de52caff3330ff49"
    end
  end

  def install
    bin.install Dir["antra*"].first => "antra"
  end

  def caveats
    <<~EOS
      To trust the local CA for HTTPS (one-time setup):

        antra trust

      This installs a local root CA into your system trust store.
      It requires admin privileges and prompts before making changes.

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

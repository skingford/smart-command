class SmartCommand < Formula
  desc "An intelligent shell with context-aware command completion"
  homepage "https://github.com/kingford/smart-command"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/kingford/smart-command/releases/download/v#{version}/smart-command-x86_64-apple-darwin.tar.gz"
      # sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
    on_arm do
      url "https://github.com/kingford/smart-command/releases/download/v#{version}/smart-command-aarch64-apple-darwin.tar.gz"
      # sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/kingford/smart-command/releases/download/v#{version}/smart-command-x86_64-unknown-linux-gnu.tar.gz"
      # sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
    on_arm do
      url "https://github.com/kingford/smart-command/releases/download/v#{version}/smart-command-aarch64-unknown-linux-gnu.tar.gz"
      # sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
  end

  def install
    bin.install "smart-command"
    (share/"smart-command").install Dir["definitions/*"]
  end

  def caveats
    <<~EOS
      Command definitions have been installed to:
        #{share}/smart-command/definitions

      To use these definitions, smart-command will automatically detect them.
      You can also copy them to your config directory:
        mkdir -p ~/.config/smart-command
        cp -r #{share}/smart-command/definitions ~/.config/smart-command/
    EOS
  end

  test do
    assert_match "smart-command", shell_output("#{bin}/smart-command --version")
  end
end

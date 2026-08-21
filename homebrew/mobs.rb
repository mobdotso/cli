# Formula for the mobdotso/homebrew-tap repository. Copy this file there as
# Formula/mobs.rb when cutting a release, filling in the sha256 of each
# release asset (shasum -a 256 <asset>).
class Mobs < Formula
  desc "Command line client for the mob.so API"
  homepage "https://github.com/mobdotso/cli"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/mobdotso/cli/releases/download/v#{version}/mobs-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_APPLE_DARWIN_SHA256"
    end
    on_intel do
      url "https://github.com/mobdotso/cli/releases/download/v#{version}/mobs-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_APPLE_DARWIN_SHA256"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/mobdotso/cli/releases/download/v#{version}/mobs-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_LINUX_MUSL_SHA256"
    end
    on_intel do
      url "https://github.com/mobdotso/cli/releases/download/v#{version}/mobs-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_X86_64_LINUX_MUSL_SHA256"
    end
  end

  def install
    bin.install "mobs"
  end

  test do
    assert_match "mobs", shell_output("#{bin}/mobs --version")
  end
end

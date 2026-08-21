# Source-build formula for a future homebrew-core submission. Core rejects
# binary-download formulas (the tap's Formula/mobs.rb) and audits new
# formulas for upstream notability, roughly 75 stars or 30 forks on the
# repository. When mobdotso/cli clears that bar:
#
#   1. Update url/sha256 to the latest release source tarball
#      (curl -L <url> | shasum -a 256).
#   2. Copy this file to homebrew-core as Formula/m/mobs.rb.
#   3. brew audit --new-formula mobs && brew install --build-from-source mobs
#   4. Open the PR against Homebrew/homebrew-core.
class Mobs < Formula
  desc "Command line client for the mob.so API"
  homepage "https://github.com/mobdotso/cli"
  url "https://github.com/mobdotso/cli/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "abefe864c5f911e58f2043a42256926da0c54c7cf251245e9570d47c8ba5b3b7"
  license "MIT"
  head "https://github.com/mobdotso/cli.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mobs --version")
    # Without a stored login the CLI refuses with a clear message.
    output = shell_output("#{bin}/mobs mobs list 2>&1", 1)
    assert_match "Not logged in", output
  end
end

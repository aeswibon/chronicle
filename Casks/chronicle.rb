cask "chronicle" do
  version "0.1.0"
  sha256 arm:   "7877f6fdf6b7f85327cd4f530a94cea15e0cf439acd32b095f595711ac2cebbb",
         intel: "dbc8a5e931b5640aa6f2da96b65409d5ce4861d7dc2a93b534617ff19e5a640b"

  url "https://github.com/aeswibon/chronicle/releases/download/v#{version}/chronicle-#{arch == :arm64 ? "arm64" : "x64"}.dmg"
  name "Chronicle"
  desc "Local-first developer observability for macOS"
  homepage "https://github.com/aeswibon/chronicle"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: :monterey

  app "Chronicle.app"

  zap trash: [
    "~/.chronicle",
    "~/Library/LaunchAgents/com.chronicle.daemon.plist",
    "~/Library/Logs/chronicle.log",
    "~/Library/Logs/chronicle.err",
    "~/Library/Application Support/chronicle",
    "~/Library/Caches/com.chronicle.app",
    "~/Library/Preferences/com.chronicle.app.plist",
  ]
end

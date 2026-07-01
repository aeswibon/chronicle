cask "chronicle" do
  version "0.1.0"
  sha256 arm:   "b9079e5414946817c62d36474645d430a777780ec0f3e2bccc1b699894a99f46",
         intel: "b5cd8e8fb4ee1c2ffd7683000233253738caad5b5cf96d9d07a4c2c97d4a5cc6"

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

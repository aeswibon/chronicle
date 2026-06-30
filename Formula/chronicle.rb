cask "chronicle" do
  version "0.1.0"
  sha256 arm:   "",
         intel: ""

  url "https://github.com/aeswibon/chronicle/releases/download/v#{version}/chronicle-#{arch == :arm64 ? "arm64" : "x64"}.dmg"
  name "Chronicle"
  desc "Developer observability platform"
  homepage "https://github.com/aeswibon/chronicle"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true

  app "Chronicle.app"

  zap trash: [
    "~/Library/Application Support/chronicle",
    "~/Library/Caches/com.chronicle.app",
    "~/Library/Preferences/com.chronicle.app.plist",
  ]
end

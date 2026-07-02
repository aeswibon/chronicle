cask "chronicle" do
  version "0.1.2"

  on_arm do
    sha256 "028d98d678e9eba7353213fad10231eab56bb33572eeef6a0769bd92ab5c75b9"
    url "https://github.com/aeswibon/chronicle/releases/download/v#{version}/chronicle-arm64.dmg"
  end

  on_intel do
    sha256 "162b51586814cbb54aafaa9678d5c08e296d5c7776ddd8e39971de0b4f7d686f"
    url "https://github.com/aeswibon/chronicle/releases/download/v#{version}/chronicle-x64.dmg"
  end

  name "Chronicle"
  desc "Local-first developer observability for macOS"
  homepage "https://github.com/aeswibon/chronicle"

  livecheck do
    url :homepage
    strategy :github_latest
  end

  depends_on macos: :monterey

  preflight do
    command.run(
      "/usr/bin/osascript",
      args:         ["-e", 'tell application "Chronicle" to if it is running then quit'],
      print_stderr: false,
    )

    [
      Pathname("#{appdir}/Chronicle.app"),
      Pathname("/Applications/Chronicle.app"),
      Pathname("#{ENV.fetch("HOME", Dir.home)}/Applications/Chronicle.app"),
    ].uniq.each do |path|
      next unless path.directory?

      opoo "Replacing existing Chronicle.app at #{path}"
      if path.parent.writable? || path.writable?
        FileUtils.rm_rf(path)
      else
        command.run!("/bin/rm", args: ["-rf", path], sudo: true)
      end
    end
  end

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

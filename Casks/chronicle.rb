cask "chronicle" do
  version "0.1.2"

  on_arm do
    sha256 "82263f135762f0297b04b10fa97118450a867614aa8cb0918d0d28d168f5b4d5"
    url "https://github.com/aeswibon/chronicle/releases/download/v#{version}/chronicle-arm64.dmg"
  end

  on_intel do
    sha256 "5ea67f6894710bbc1bebe3893fcd1a08db52e2915b6caa2a9fd6d8469841b05b"
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

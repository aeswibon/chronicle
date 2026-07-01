import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

struct FocusPayload: Codable {
    let event: String
    let name: String
    let bundle_id: String
    let pid: Int32
    let window_title: String?
    let title_source: String
    let timestamp_ms: Int64
    let accessibility_trusted: Bool
    let screen_capture_granted: Bool
}

struct PermissionsPayload: Codable {
    let accessibility_trusted: Bool
    let screen_capture_granted: Bool
    let can_read_window_titles: Bool
}

private func nowMs() -> Int64 {
    Int64(Date().timeIntervalSince1970 * 1000)
}

private func axTrusted() -> Bool {
    AXIsProcessTrusted()
}

private func screenCaptureGranted() -> Bool {
    if #available(macOS 10.15, *) {
        return CGPreflightScreenCaptureAccess()
    }
    return true
}

private func cgWindowTitle(pid: Int32) -> String? {
    let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
    guard let list = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
        return nil
    }
    for window in list {
        guard let owner = window[kCGWindowOwnerPID as String] as? Int32, owner == pid else { continue }
        let layer = window[kCGWindowLayer as String] as? Int ?? 99
        if layer != 0 { continue }
        if let name = window[kCGWindowName as String] as? String {
            let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty { return trimmed }
        }
    }
    return nil
}

private func axWindowTitle(pid: Int32) -> String? {
    guard axTrusted() else { return nil }
    let appEl = AXUIElementCreateApplication(pid)
    var focused: CFTypeRef?
    guard AXUIElementCopyAttributeValue(appEl, kAXFocusedWindowAttribute as CFString, &focused) == .success,
          let windowEl = focused
    else { return nil }
    var title: CFTypeRef?
    guard AXUIElementCopyAttributeValue(windowEl as! AXUIElement, kAXTitleAttribute as CFString, &title) == .success,
          let t = title as? String
    else { return nil }
    let trimmed = t.trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
}

private func normalizeTabTitle(_ title: String) -> String {
    var t = title.trimmingCharacters(in: .whitespacesAndNewlines)
    if t.isEmpty { return t }

    for marker in [" - ⏳", " — ⏳", " – ⏳", " ⏳"] {
        if let range = t.range(of: marker) {
            t = String(t[..<range.lowerBound])
            break
        }
    }

    let leadingMarkers: [Character] = ["●", "•", "◦", "*", "○"]
    while let first = t.first, leadingMarkers.contains(first) {
        t.removeFirst()
        t = t.trimmingCharacters(in: .whitespaces)
    }

    let trailingMarkers: [Character] = ["●", "•", "◦", "*", "○"]
    while let last = t.last, trailingMarkers.contains(last) {
        t.removeLast()
        t = t.trimmingCharacters(in: .whitespaces)
    }

    let lower = t.lowercased()
    for suffix in [
        " (unsaved)", " — unsaved", " - unsaved",
        " (modified)", " — modified", " - modified",
        " — loading", " - loading", " …",
    ] {
        if lower.hasSuffix(suffix) {
            t = String(t.dropLast(suffix.count)).trimmingCharacters(in: .whitespaces)
            break
        }
    }

    while t.hasSuffix(".") || t.hasSuffix("·") || t.hasSuffix(" ") || t.hasSuffix("…") {
        t.removeLast()
    }
    return t.trimmingCharacters(in: .whitespacesAndNewlines)
}

/// Matches Rust `tab_session_key`: app|bundle|normalized_title
private func tabSessionKey(name: String, bundleId: String, title: String?) -> String {
    let app = name.lowercased()
    let bundle = bundleId.lowercased()
    let tab: String
    if let title = title {
        let normalized = normalizeTabTitle(title)
        tab = normalized.isEmpty ? "_default" : normalized
    } else {
        tab = "_default"
    }
    return "\(app)|\(bundle)|\(tab)"
}

private func resolveTitle(pid: Int32) -> (String?, String) {
    if let t = axWindowTitle(pid: pid) { return (t, "accessibility") }
    if let t = cgWindowTitle(pid: pid) { return (t, "cgwindow") }
    return (nil, "none")
}

private func payload(from app: NSRunningApplication, event: String, title: String?, titleSource: String) -> FocusPayload {
    FocusPayload(
        event: event,
        name: app.localizedName ?? app.bundleIdentifier ?? "Unknown",
        bundle_id: app.bundleIdentifier ?? "",
        pid: app.processIdentifier,
        window_title: title,
        title_source: titleSource,
        timestamp_ms: nowMs(),
        accessibility_trusted: axTrusted(),
        screen_capture_granted: screenCaptureGranted()
    )
}

private func emit(_ p: FocusPayload) {
    let enc = JSONEncoder()
    guard let data = try? enc.encode(p), let line = String(data: data, encoding: .utf8) else { return }
    FileHandle.standardOutput.write((line + "\n").data(using: .utf8)!)
}

private func snapshotFromFrontmost(event: String) {
    guard let app = NSWorkspace.shared.frontmostApplication else { return }
    let (title, source) = resolveTitle(pid: app.processIdentifier)
    emit(payload(from: app, event: event, title: title, titleSource: source))
}

private func runMonitor() {
    var lastPid: pid_t = -1
    var lastTabKey: String?

    func publish(_ app: NSRunningApplication, event: String) {
        let (title, source) = resolveTitle(pid: app.processIdentifier)
        let pid = app.processIdentifier
        let name = app.localizedName ?? app.bundleIdentifier ?? "Unknown"
        let bundleId = app.bundleIdentifier ?? ""
        let tabKey = tabSessionKey(name: name, bundleId: bundleId, title: title)
        if event == "activation" || pid != lastPid || tabKey != lastTabKey {
            lastPid = pid
            lastTabKey = tabKey
            emit(payload(from: app, event: event, title: title, titleSource: source))
        }
    }

    snapshotFromFrontmost(event: "snapshot")

    let center = NSWorkspace.shared.notificationCenter
    center.addObserver(
        forName: NSWorkspace.didActivateApplicationNotification,
        object: nil,
        queue: nil
    ) { note in
        guard let app = note.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication else { return }
        publish(app, event: "activation")
    }

    Timer.scheduledTimer(withTimeInterval: 3.0, repeats: true) { _ in
        guard let app = NSWorkspace.shared.frontmostApplication else { return }
        publish(app, event: "window")
    }

    RunLoop.current.run()
}

let args = CommandLine.arguments
guard args.count >= 2 else {
    fputs("usage: chronicle-focus-monitor <snapshot|monitor|permissions|request-accessibility>\n", stderr)
    exit(1)
}

switch args[1] {
case "snapshot":
    snapshotFromFrontmost(event: "snapshot")
case "monitor":
    runMonitor()
case "permissions":
    let p = PermissionsPayload(
        accessibility_trusted: axTrusted(),
        screen_capture_granted: screenCaptureGranted(),
        can_read_window_titles: axTrusted() || screenCaptureGranted()
    )
    if let data = try? JSONEncoder().encode(p), let json = String(data: data, encoding: .utf8) {
        print(json)
    }
case "request-accessibility":
    let key = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
    let opts = [key: true] as CFDictionary
    _ = AXIsProcessTrustedWithOptions(opts)
    exit(0)
default:
    exit(1)
}

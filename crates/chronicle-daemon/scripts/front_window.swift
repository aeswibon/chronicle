import CoreGraphics
import Foundation

guard CommandLine.arguments.count > 1,
      let pid = Int32(CommandLine.arguments[1]),
      pid > 0
else {
    exit(1)
}

let options = CGWindowListOption(arrayLiteral: .optionOnScreenOnly, .excludeDesktopElements)
guard let info = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
    exit(0)
}

for window in info {
    guard let owner = window["kCGWindowOwnerPID"] as? Int32, owner == pid else { continue }
    let layer = window["kCGWindowLayer"] as? Int ?? 99
    if layer != 0 { continue }
    if let name = window["kCGWindowName"] as? String {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            print(trimmed)
            exit(0)
        }
    }
}

exit(0)

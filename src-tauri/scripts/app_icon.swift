import AppKit
import Foundation

let args = CommandLine.arguments
guard args.count >= 3 else {
    fputs("usage: app_icon.swift <app-path> <size>\n", stderr)
    exit(1)
}

let appPath = args[1]
let size = Double(args[2]) ?? 32
let pixels = Int(size)

guard !appPath.isEmpty, FileManager.default.fileExists(atPath: appPath) else {
    exit(2)
}

let icon = NSWorkspace.shared.icon(forFile: appPath)
icon.size = NSSize(width: size, height: size)

guard let rep = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: pixels,
    pixelsHigh: pixels,
    bitsPerSample: 8,
    samplesPerPixel: 4,
    hasAlpha: true,
    isPlanar: false,
    colorSpaceName: .deviceRGB,
    bytesPerRow: 0,
    bitsPerPixel: 0
) else {
    exit(3)
}

rep.size = NSSize(width: size, height: size)
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
icon.draw(
    in: NSRect(x: 0, y: 0, width: size, height: size),
    from: .zero,
    operation: .copy,
    fraction: 1.0,
    respectFlipped: true,
    hints: nil
)
NSGraphicsContext.restoreGraphicsState()

guard let png = rep.representation(
    using: .png,
    properties: [.compressionFactor: NSNumber(value: 0.85)]
) else {
    exit(4)
}

FileHandle.standardOutput.write(png)

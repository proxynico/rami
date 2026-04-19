#!/usr/bin/env swift

import AppKit
import Foundation

struct IconSpec {
    let pointSize: Int
    let filename: String
}

let specs = [
    IconSpec(pointSize: 16, filename: "icon_16x16.png"),
    IconSpec(pointSize: 32, filename: "icon_16x16@2x.png"),
    IconSpec(pointSize: 32, filename: "icon_32x32.png"),
    IconSpec(pointSize: 64, filename: "icon_32x32@2x.png"),
    IconSpec(pointSize: 128, filename: "icon_128x128.png"),
    IconSpec(pointSize: 256, filename: "icon_128x128@2x.png"),
    IconSpec(pointSize: 256, filename: "icon_256x256.png"),
    IconSpec(pointSize: 512, filename: "icon_256x256@2x.png"),
    IconSpec(pointSize: 512, filename: "icon_512x512.png"),
    IconSpec(pointSize: 1024, filename: "icon_512x512@2x.png"),
]

func writePng(image: NSImage, to url: URL) throws {
    guard
        let tiff = image.tiffRepresentation,
        let rep = NSBitmapImageRep(data: tiff),
        let png = rep.representation(using: .png, properties: [:])
    else {
        throw NSError(domain: "rami.icon", code: 1, userInfo: [
            NSLocalizedDescriptionKey: "failed to encode PNG"
        ])
    }

    try png.write(to: url)
}

func drawIcon(size: CGFloat) -> NSImage {
    let image = NSImage(size: NSSize(width: size, height: size))
    image.lockFocus()
    defer { image.unlockFocus() }

    let rect = NSRect(x: 0, y: 0, width: size, height: size)
    NSColor.clear.setFill()
    rect.fill()

    let background = NSBezierPath(
        roundedRect: rect.insetBy(dx: size * 0.04, dy: size * 0.04),
        xRadius: size * 0.24,
        yRadius: size * 0.24
    )
    let gradient = NSGradient(colors: [
        NSColor(calibratedRed: 0.08, green: 0.18, blue: 0.22, alpha: 1),
        NSColor(calibratedRed: 0.02, green: 0.07, blue: 0.10, alpha: 1),
    ])!
    gradient.draw(in: background, angle: -90)

    NSColor(calibratedWhite: 1, alpha: 0.08).setStroke()
    background.lineWidth = max(2, size * 0.018)
    background.stroke()

    let glowRect = rect.insetBy(dx: size * 0.12, dy: size * 0.18)
    let glow = NSBezierPath(
        roundedRect: glowRect,
        xRadius: size * 0.18,
        yRadius: size * 0.18
    )
    NSColor(calibratedRed: 0.17, green: 0.80, blue: 0.82, alpha: 0.16).setFill()
    glow.fill()

    let chipRect = rect.insetBy(dx: size * 0.20, dy: size * 0.20)
    let chip = NSBezierPath(
        roundedRect: chipRect,
        xRadius: size * 0.14,
        yRadius: size * 0.14
    )
    NSColor(calibratedRed: 0.62, green: 0.95, blue: 0.92, alpha: 0.95).setStroke()
    chip.lineWidth = max(3, size * 0.026)
    chip.stroke()

    let coreRect = chipRect.insetBy(dx: chipRect.width * 0.24, dy: chipRect.height * 0.24)
    let core = NSBezierPath(
        roundedRect: coreRect,
        xRadius: size * 0.08,
        yRadius: size * 0.08
    )
    let coreGradient = NSGradient(colors: [
        NSColor(calibratedRed: 0.46, green: 0.92, blue: 0.90, alpha: 0.98),
        NSColor(calibratedRed: 0.28, green: 0.78, blue: 0.80, alpha: 0.92),
    ])!
    coreGradient.draw(in: core, angle: -90)

    NSColor(calibratedWhite: 1, alpha: 0.16).setStroke()
    core.lineWidth = max(2, size * 0.014)
    core.stroke()

    let traceInsetX = chipRect.width * 0.15
    let traceInsetY = chipRect.height * 0.15
    let leftTraceX = chipRect.minX + traceInsetX
    let rightTraceX = chipRect.maxX - traceInsetX
    let bottomTraceY = chipRect.minY + traceInsetY
    let topTraceY = chipRect.maxY - traceInsetY

    let traceColor = NSColor(calibratedRed: 0.54, green: 0.92, blue: 0.90, alpha: 0.90)
    traceColor.setStroke()

    let traceWidth = max(3, size * 0.020)
    let socketRadius = max(3, size * 0.026)
    let coreMidX = coreRect.midX
    let coreMidY = coreRect.midY
    let coreMinX = coreRect.minX
    let coreMaxX = coreRect.maxX
    let coreMinY = coreRect.minY
    let coreMaxY = coreRect.maxY

    let segments = [
        (NSPoint(x: leftTraceX, y: coreMidY), NSPoint(x: coreMinX, y: coreMidY)),
        (NSPoint(x: coreMaxX, y: coreMidY), NSPoint(x: rightTraceX, y: coreMidY)),
        (NSPoint(x: coreMidX, y: topTraceY), NSPoint(x: coreMidX, y: coreMaxY)),
        (NSPoint(x: coreMidX, y: bottomTraceY), NSPoint(x: coreMidX, y: coreMinY)),
    ]

    for (start, end) in segments {
        let path = NSBezierPath()
        path.move(to: start)
        path.line(to: end)
        path.lineWidth = traceWidth
        path.lineCapStyle = .round
        path.stroke()

        let socket = NSBezierPath(ovalIn: NSRect(
            x: start.x - socketRadius / 2,
            y: start.y - socketRadius / 2,
            width: socketRadius,
            height: socketRadius
        ))
        traceColor.setFill()
        socket.fill()
    }

    return image
}

guard CommandLine.arguments.count == 2 else {
    fputs("usage: generate-icon.swift /absolute/path/to/rami.icns\n", stderr)
    exit(1)
}

let outputURL = URL(fileURLWithPath: CommandLine.arguments[1])
let fileManager = FileManager.default
let tempRoot = URL(fileURLWithPath: NSTemporaryDirectory(), isDirectory: true)
let iconsetURL = tempRoot.appendingPathComponent("rami.iconset", isDirectory: true)

try? fileManager.removeItem(at: iconsetURL)
try fileManager.createDirectory(at: iconsetURL, withIntermediateDirectories: true)

for spec in specs {
    let image = drawIcon(size: CGFloat(spec.pointSize))
    try writePng(image: image, to: iconsetURL.appendingPathComponent(spec.filename))
}

if fileManager.fileExists(atPath: outputURL.path) {
    try fileManager.removeItem(at: outputURL)
}

let process = Process()
process.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
process.arguments = ["-c", "icns", iconsetURL.path, "-o", outputURL.path]
try process.run()
process.waitUntilExit()

guard process.terminationStatus == 0 else {
    throw NSError(domain: "rami.icon", code: 2, userInfo: [
        NSLocalizedDescriptionKey: "iconutil failed"
    ])
}

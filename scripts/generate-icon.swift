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

    let cx = rect.midX
    let cy = rect.midY - size * 0.04
    let radius = size * 0.30
    let arcStroke = max(4, size * 0.042)

    let startDeg: CGFloat = 210
    let endDeg: CGFloat = -30

    let arc = NSBezierPath()
    arc.appendArc(
        withCenter: NSPoint(x: cx, y: cy),
        radius: radius,
        startAngle: startDeg,
        endAngle: endDeg,
        clockwise: true
    )
    NSColor(calibratedRed: 0.62, green: 0.95, blue: 0.92, alpha: 0.95).setStroke()
    arc.lineWidth = arcStroke
    arc.lineCapStyle = .round
    arc.stroke()

    let dotRadius = size * 0.032
    for i in 0..<5 {
        let t = CGFloat(i) / 4.0
        let deg = startDeg + (endDeg - startDeg) * t
        let rad = deg * .pi / 180
        let x = cx + cos(rad) * radius
        let y = cy + sin(rad) * radius
        let dot = NSBezierPath(ovalIn: NSRect(
            x: x - dotRadius, y: y - dotRadius,
            width: dotRadius * 2, height: dotRadius * 2
        ))
        NSColor(calibratedRed: 0.46, green: 0.92, blue: 0.90, alpha: 1).setFill()
        dot.fill()
    }

    let needleT: CGFloat = 0.75
    let needleDeg = startDeg + (endDeg - startDeg) * needleT
    let needleRad = needleDeg * .pi / 180
    let needleLen = radius * 0.92
    let needle = NSBezierPath()
    needle.move(to: NSPoint(x: cx, y: cy))
    needle.line(to: NSPoint(
        x: cx + cos(needleRad) * needleLen,
        y: cy + sin(needleRad) * needleLen
    ))
    needle.lineWidth = max(4, size * 0.046)
    needle.lineCapStyle = .round
    NSColor.white.setStroke()
    needle.stroke()

    let hubRadius = size * 0.04
    let hub = NSBezierPath(ovalIn: NSRect(
        x: cx - hubRadius, y: cy - hubRadius,
        width: hubRadius * 2, height: hubRadius * 2
    ))
    NSColor.white.setFill()
    hub.fill()

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

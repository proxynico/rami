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

func rgb(_ hex: UInt32, _ alpha: CGFloat = 1) -> NSColor {
    NSColor(
        srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
        green: CGFloat((hex >> 8) & 0xFF) / 255,
        blue: CGFloat(hex & 0xFF) / 255,
        alpha: alpha
    )
}

func gradient(_ stops: [(UInt32, CGFloat)]) -> NSGradient {
    var colors: [NSColor] = []
    var locations: [CGFloat] = []
    for (hex, location) in stops {
        colors.append(rgb(hex))
        locations.append(location)
    }
    return NSGradient(colors: colors, atLocations: &locations, colorSpace: .sRGB)!
}

/// A RAM stick on a graphite board: four chips, three lit amber to show the
/// fill, over a gold contact strip split by the key notch. Drawn on a
/// 1024-point grid, top-down.
func drawIcon(size: CGFloat) -> NSImage {
    let image = NSImage(size: NSSize(width: size, height: size))
    image.lockFocus()
    defer { image.unlockFocus() }

    let scale = size / 1024
    let topDown = NSAffineTransform()
    topDown.translateX(by: 0, yBy: size)
    topDown.scaleX(by: scale, yBy: -scale)
    topDown.concat()

    let body = NSBezierPath(
        roundedRect: NSRect(x: 100, y: 100, width: 824, height: 824),
        xRadius: 185,
        yRadius: 185
    )
    gradient([(0x26272B, 0), (0x08080A, 1)]).draw(in: body, angle: 90)

    let board = NSRect(x: 152, y: 360, width: 720, height: 290)
    let notch = NSRect(x: 470, y: board.maxY - 44, width: 36, height: 44)
    let depth: CGFloat = 12
    func stickPath(_ rect: NSRect) -> NSBezierPath {
        let path = NSBezierPath(roundedRect: rect, xRadius: 30, yRadius: 30)
        let cut = NSRect(x: notch.minX, y: notch.minY, width: notch.width, height: rect.maxY - notch.minY)
        path.append(NSBezierPath(rect: cut))
        path.windingRule = .evenOdd
        return path
    }

    NSGraphicsContext.saveGraphicsState()
    let dropShadow = NSShadow()
    dropShadow.shadowColor = NSColor.black.withAlphaComponent(0.6)
    dropShadow.shadowBlurRadius = 36 * scale
    dropShadow.shadowOffset = NSSize(width: 0, height: -24 * scale)
    dropShadow.set()
    rgb(0x0E0F11).setFill()
    stickPath(board.offsetBy(dx: 0, dy: depth)).fill()
    NSGraphicsContext.restoreGraphicsState()

    let stick = stickPath(board)
    gradient([(0x34373D, 0), (0x1B1D21, 1)]).draw(in: stick, angle: 90)
    rgb(0xFFFFFF, 0.14).setStroke()
    stick.lineWidth = 3
    stick.stroke()

    let gold = gradient([(0xFCEAA8, 0), (0xE0AA44, 0.5), (0xB27820, 1)])
    let stripY = board.maxY - 62
    for strip in [
        NSRect(x: board.minX + 44, y: stripY, width: notch.minX - 4 - board.minX - 44, height: 38),
        NSRect(x: notch.maxX + 4, y: stripY, width: board.maxX - 44 - notch.maxX - 4, height: 38),
    ] {
        gold.draw(in: NSBezierPath(roundedRect: strip, xRadius: 9, yRadius: 9), angle: 90)
    }

    let amber = gradient([(0xFFD57C, 0), (0xF7A933, 0.55), (0xE4841A, 1)])
    let dark = gradient([(0x2E2F33, 0), (0x0B0B0C, 1)])
    let chipWidth: CGFloat = 132
    let chipGap: CGFloat = 44
    let chipsLeft = board.minX + (board.width - (4 * chipWidth + 3 * chipGap)) / 2
    for index in 0..<4 {
        let lit = index < 3
        let chip = NSRect(
            x: chipsLeft + CGFloat(index) * (chipWidth + chipGap),
            y: board.minY + 42,
            width: chipWidth,
            height: 150
        )
        let path = NSBezierPath(roundedRect: chip, xRadius: 16, yRadius: 16)
        if lit {
            NSGraphicsContext.saveGraphicsState()
            let glow = NSShadow()
            glow.shadowColor = rgb(0xF7A933, 0.55)
            glow.shadowBlurRadius = 30 * scale
            glow.set()
            rgb(0xF7A933).setFill()
            path.fill()
            NSGraphicsContext.restoreGraphicsState()
        }
        (lit ? amber : dark).draw(in: path, angle: 90)
        rgb(0x000000, 0.3).setStroke()
        path.lineWidth = 3
        path.stroke()
        rgb(0xFFFFFF, lit ? 0.5 : 0.12).setFill()
        NSBezierPath(
            roundedRect: NSRect(x: chip.minX + 10, y: chip.minY + 6, width: chip.width - 20, height: 5),
            xRadius: 2.5,
            yRadius: 2.5
        ).fill()
    }

    let gloss = NSGradient(
        colors: [rgb(0xFFFFFF, 0.16), rgb(0xFFFFFF, 0)],
        atLocations: [0, 0.48],
        colorSpace: .sRGB
    )!
    gloss.draw(in: body, angle: 90)
    rgb(0xFFFFFF, 0.1).setStroke()
    body.lineWidth = 3
    body.stroke()

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

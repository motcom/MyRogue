// macOS上で実行: swift -module-cache-path /private/tmp/rogue-swift-cache
//   assets/generate_font.swift <terminal8x8.png> <srcディレクトリ> <dataディレクトリ>
// 先頭256字はbracket-libのCP437。以降はソースとLuaの非ASCII文字を収録。
import AppKit
import CoreText

let args = CommandLine.arguments
guard args.count == 4 else { fatalError("引数: CP437画像 src data") }
let root = URL(fileURLWithPath: args[2])
var characters = Set<Unicode.Scalar>()
for directory in [root, URL(fileURLWithPath: args[3])] {
    let files = FileManager.default.enumerator(at: directory, includingPropertiesForKeys: nil)!
    for case let file as URL in files where ["rs", "lua"].contains(file.pathExtension) {
        let source = try String(contentsOf: file, encoding: .utf8)
        for scalar in source.unicodeScalars where scalar.value > 127 {
            characters.insert(scalar)
        }
    }
}
let sorted = characters.sorted { $0.value < $1.value }
let charset = String(String.UnicodeScalarView(sorted))
try charset.write(to: root.appendingPathComponent("assets/japanese_chars.txt"), atomically: true, encoding: .utf8)
let columns = 32
let rows = (256 + sorted.count + columns - 1) / columns
let width = columns * 16
let height = rows * 16
let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                        bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
context.interpolationQuality = .none
let original = NSImage(contentsOfFile: args[1])!.cgImage(forProposedRect: nil, context: nil, hints: nil)!
for i in 0..<256 {
    let tile = original.cropping(to: CGRect(x: (i % 16) * 8, y: (i / 16) * 8, width: 8, height: 8))!
    context.draw(tile, in: CGRect(x: (i % columns) * 16, y: height - (i / columns + 1) * 16, width: 16, height: 16))
}
// フォント本体は配布せず、描画済みの文字画像だけをゲームへ同梱。
let font = CTFontCreateWithName("HiraginoSans-W3" as CFString, 14, nil)
for (offset, scalar) in sorted.enumerated() {
    let index = 256 + offset
    let text = NSAttributedString(string: String(scalar), attributes: [
        .font: font, .foregroundColor: NSColor.white
    ])
    let line = CTLineCreateWithAttributedString(text)
    let advance = CTLineGetTypographicBounds(line, nil, nil, nil)
    context.saveGState()
    let x = CGFloat((index % columns) * 16)
    let y = CGFloat(height - (index / columns + 1) * 16)
    context.clip(to: CGRect(x: x, y: y, width: 16, height: 16))
    context.textPosition = CGPoint(x: x + (16 - advance) / 2,
                                   y: y + (16 - CTFontGetAscent(font) - CTFontGetDescent(font)) / 2 + CTFontGetDescent(font))
    CTLineDraw(line, context)
    context.restoreGState()
}
let image = NSBitmapImageRep(cgImage: context.makeImage()!)
try image.representation(using: .png, properties: [:])!.write(to: root.appendingPathComponent("assets/japanese16x16.png"))
print("CP437: 256 / 日本語等: \(sorted.count) / 画像: \(width)x\(height)")

import CoreGraphics
import ImageIO
import Foundation
import UniformTypeIdentifiers

@main
enum MakeIcon {
    static func main() throws {
        let directory = URL(fileURLWithPath: CommandLine.arguments[1])
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        for size in [16, 32, 128, 256, 512] {
            for scale in [1, 2] {
                let pixels = size * scale
                let context = CGContext(data: nil, width: pixels, height: pixels, bitsPerComponent: 8,
                    bytesPerRow: pixels * 4, space: CGColorSpaceCreateDeviceRGB(),
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
                context.translateBy(x: 0, y: CGFloat(pixels))
                context.scaleBy(x: 1, y: -1)
                SensesBrand.drawIcon(in: context, size: CGFloat(pixels), compact: size <= 32)
                let suffix = scale == 2 ? "@2x" : ""
                let url = directory.appendingPathComponent("icon_\(size)x\(size)\(suffix).png")
                let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
                CGImageDestinationAddImage(destination, context.makeImage()!, nil)
                precondition(CGImageDestinationFinalize(destination))
            }
        }
        if CommandLine.arguments.count > 2 {
            try exportLogo(to: URL(fileURLWithPath: CommandLine.arguments[2]))
        }
    }

    static func exportLogo(to directory: URL) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let path = SensesBrand.mark(in: CGRect(x: 64, y: 64, width: 896, height: 896))
        var commands: [String] = []
        func point(_ p: CGPoint) -> String {
            String(format: "%.3f %.3f", locale: Locale(identifier: "en_US_POSIX"), Double(p.x), Double(p.y))
        }
        path.applyWithBlock { pointer in
            let element = pointer.pointee
            switch element.type {
            case .moveToPoint: commands.append("M " + point(element.points[0]))
            case .addLineToPoint: commands.append("L " + point(element.points[0]))
            case .addQuadCurveToPoint: commands.append("Q " + point(element.points[0]) + " " + point(element.points[1]))
            case .addCurveToPoint: commands.append("C " + point(element.points[0]) + " " + point(element.points[1]) + " " + point(element.points[2]))
            case .closeSubpath: commands.append("Z")
            @unknown default: break
            }
        }
        let svg = """
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" role="img" aria-labelledby="title">
          <title id="title">Senses — S waveform</title>
          <path fill="#1f8c7a" d="\(commands.joined(separator: " "))"/>
        </svg>
        """
        try svg.write(to: directory.appendingPathComponent("Senses-Logo.svg"), atomically: true, encoding: .utf8)
        let context = CGContext(data: nil, width: 1024, height: 1024, bitsPerComponent: 8,
            bytesPerRow: 1024 * 4, space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.translateBy(x: 0, y: 1024)
        context.scaleBy(x: 1, y: -1)
        context.addPath(path)
        context.setFillColor(SensesBrand.accent)
        context.fillPath()
        let destination = CGImageDestinationCreateWithURL(directory.appendingPathComponent("Senses-Logo.png") as CFURL,
            UTType.png.identifier as CFString, 1, nil)!
        CGImageDestinationAddImage(destination, context.makeImage()!, nil)
        precondition(CGImageDestinationFinalize(destination))
    }
}

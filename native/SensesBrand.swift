import CoreGraphics

/// One vector master for the window, menu bar, and exported app icons.
enum SensesBrand {
    static let accent = CGColor(red: 0.12, green: 0.55, blue: 0.48, alpha: 1)

    static func mark(in rect: CGRect, compact: Bool = false) -> CGPath {
        let signal = CGMutablePath()
        signal.move(to: CGPoint(x: 702, y: 284))
        signal.addCurve(to: CGPoint(x: 338, y: 326),
                        control1: CGPoint(x: 626, y: 206), control2: CGPoint(x: 381, y: 214))
        signal.addCurve(to: CGPoint(x: 512, y: 512),
                        control1: CGPoint(x: 295, y: 438), control2: CGPoint(x: 405, y: 466))
        signal.addCurve(to: CGPoint(x: 686, y: 698),
                        control1: CGPoint(x: 619, y: 558), control2: CGPoint(x: 729, y: 586))
        signal.addCurve(to: CGPoint(x: 322, y: 740),
                        control1: CGPoint(x: 643, y: 810), control2: CGPoint(x: 398, y: 818))
        let silhouette = CGMutablePath()
        silhouette.addPath(signal.copy(strokingWithWidth: compact ? 92 : 80, lineCap: .round,
                                       lineJoin: .round, miterLimit: 1))
        let bars = CGMutablePath()
        for x in [CGFloat(226), 798] {
            bars.move(to: CGPoint(x: x, y: 436))
            bars.addLine(to: CGPoint(x: x, y: 588))
        }
        if !compact {
            for x in [CGFloat(150), 874] {
                bars.move(to: CGPoint(x: x, y: 492))
                bars.addLine(to: CGPoint(x: x, y: 532))
            }
        }
        silhouette.addPath(bars.copy(strokingWithWidth: compact ? 58 : 48, lineCap: .round,
                                     lineJoin: .round, miterLimit: 1))
        let bounds = silhouette.boundingBoxOfPath
        let scale = min(rect.width / bounds.width, rect.height / bounds.height)
        var transform = CGAffineTransform(a: scale, b: 0, c: 0, d: scale,
            tx: rect.midX - bounds.midX * scale, ty: rect.midY - bounds.midY * scale)
        return silhouette.copy(using: &transform)!
    }

    /// The drawing coordinate system has its origin at the top left.
    static func drawIcon(in context: CGContext, size: CGFloat, compact: Bool = false) {
        let space = CGColorSpaceCreateDeviceRGB()
        context.saveGState()
        context.scaleBy(x: size / 1024, y: size / 1024)
        let tile = CGPath(roundedRect: CGRect(x: 72, y: 72, width: 880, height: 880),
                          cornerWidth: 198, cornerHeight: 198, transform: nil)
        context.setShadow(offset: CGSize(width: 0, height: 12), blur: 24,
                          color: CGColor(gray: 0, alpha: 0.20))
        context.addPath(tile)
        context.setFillColor(CGColor(red: 0.04, green: 0.10, blue: 0.09, alpha: 1))
        context.fillPath()
        context.setShadow(offset: .zero, blur: 0, color: nil)
        context.saveGState()
        context.addPath(tile)
        context.clip()
        let background = CGGradient(colorsSpace: space, colors: [
            CGColor(red: 0.10, green: 0.23, blue: 0.20, alpha: 1),
            CGColor(red: 0.035, green: 0.09, blue: 0.08, alpha: 1)
        ] as CFArray, locations: [0, 1])!
        context.drawLinearGradient(background, start: CGPoint(x: 200, y: 100),
                                   end: CGPoint(x: 810, y: 940),
                                   options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
        context.restoreGState()
        context.addPath(tile)
        context.setStrokeColor(CGColor(red: 0.65, green: 0.92, blue: 0.83, alpha: 0.17))
        context.setLineWidth(2)
        context.strokePath()

        context.addPath(mark(in: CGRect(x: 181, y: 219, width: 662, height: 586), compact: compact))
        context.clip()
        let foreground = CGGradient(colorsSpace: space, colors: [
            CGColor(red: 0.55, green: 0.96, blue: 0.83, alpha: 1),
            CGColor(red: 0.23, green: 0.75, blue: 0.63, alpha: 1)
        ] as CFArray, locations: [0, 1])!
        context.drawLinearGradient(foreground, start: CGPoint(x: 330, y: 210),
                                   end: CGPoint(x: 670, y: 820),
                                   options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
        context.restoreGState()
    }
}

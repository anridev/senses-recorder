import SwiftUI

/// Linear channel peaks in the range 0...1, displayed on a −60 to 0 dB scale.
public struct SensesStereoMeter: View {
    let left: Float
    let right: Float

    public init(left: Float, right: Float) {
        self.left = left
        self.right = right
    }
    private let channels = ["L", "R"]

    public var body: some View {
        VStack(spacing: 4) {
            ForEach(channels, id: \.self) { channel in
                let input = channel == "L" ? left : right
                let peak = input.isFinite ? min(1, max(0, input)) : 0
                HStack(spacing: 7) {
                    Text(channel).foregroundStyle(.white.opacity(0.76)).frame(width: 10)
                    Canvas { context, size in
                        let count = 60
                        let step = size.width / CGFloat(count)
                        let level = peak > 0 ? max(0, min(1, (20 * log10(Double(peak)) + 60) / 60)) : 0
                        for index in 0..<count {
                            let rect = CGRect(x: CGFloat(index) * step, y: 0,
                                              width: max(1, step - 2), height: size.height)
                            let lit = Double(index) + 0.5 <= level * Double(count)
                            let color: Color
                            switch index {
                            case 58...: color = .red
                            case 48..<58: color = .yellow
                            default: color = .mint
                            }
                            context.fill(Path(roundedRect: rect, cornerRadius: 1),
                                         with: .color(lit ? color : .white.opacity(0.15)))
                        }
                    }.frame(height: 7)
                }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(channel == "L" ? "Left channel" : "Right channel")
                .accessibilityValue(peak > 0 ? "\(Int(20 * log10(Double(peak)))) decibels" : "Silent")
            }
            HStack {
                Text("−60")
                Spacer()
                Text("−48")
                Spacer()
                Text("−36")
                Spacer()
                Text("−24")
                Spacer()
                Text("−12")
                Spacer()
                Text("0 dB")
            }
            .padding(.leading, 17)
            .foregroundStyle(.white.opacity(0.66))
            .accessibilityHidden(true)
        }
        .font(SensesStyle.meterLabel)
    }
}

import SwiftUI

struct SessionMonitor: View {
    @ObservedObject var model: RecorderModel
    private let mint = Color(red: 0.55, green: 0.93, blue: 0.8)

    private var statusLabel: String {
        if model.busy { return "Working…" }
        if model.status.isActive { return model.status.state.capitalized }
        if !model.allAudio && model.selected.isEmpty { return "Choose Apps" }
        return model.status.files.isEmpty ? "Ready" : "Session Finished"
    }

    private var indicatorColor: Color {
        switch model.status.state {
        case "recording": return .red
        case "paused": return .orange
        default: return mint
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 19) {
                HStack {
                    HStack(spacing: 7) {
                        Circle().fill(indicatorColor).frame(width: 7, height: 7)
                        Text(statusLabel).font(.callout.weight(.medium))
                    }
                    Spacer()
                    Text("Stereo · 48 kHz").font(.callout).foregroundStyle(.white.opacity(0.72))
                }
                Text(model.elapsed)
                    .font(.system(size: 65, weight: .light, design: .rounded))
                    .monospacedDigit()
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                    .accessibilityLabel("Elapsed time")
                    .accessibilityValue(model.elapsed)
                StereoMeter(left: model.status.left, right: model.status.right)
                    .frame(maxWidth: 550)
            }
            .foregroundStyle(.white)
            .padding(.horizontal, 25)
            .padding(.top, 22)
            .padding(.bottom, 21)
            .background {
                LinearGradient(colors: [Color(red: 0.11, green: 0.21, blue: 0.2),
                                        Color(red: 0.055, green: 0.09, blue: 0.105)],
                               startPoint: .topLeading, endPoint: .bottomTrailing)
            }
            HStack(spacing: 10) {
                if model.status.isActive {
                    Button(action: model.stop) {
                        Label("Stop & Save", systemImage: "stop.fill")
                    }
                    .buttonStyle(.borderedProminent)
                    Button(action: model.togglePause) {
                        Label(model.status.state == "paused" ? "Resume" : "Pause",
                              systemImage: model.status.state == "paused" ? "play.fill" : "pause.fill")
                    }
                    .buttonStyle(.bordered)
                } else {
                    Button(action: model.record) {
                        Label("Start Recording", systemImage: "record.circle")
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(Color(red: 0.78, green: 0.15, blue: 0.18))
                    .disabled(!model.canRecord)
                }
                Spacer(minLength: 4)
                Group {
                    if !model.allAudio, model.selectedApps.count == 1, let app = model.selectedApps.first {
                        SourceAppLabel(app: app, iconSize: 20)
                    } else {
                        Text(model.sourceSummary)
                    }
                }
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                    .multilineTextAlignment(.trailing)
            }
            .controlSize(.large)
            .padding(.horizontal, 19)
            .padding(.vertical, 14)
            .disabled(model.busy)
            .background(.background.secondary)
        }
        .clipShape(RoundedRectangle(cornerRadius: 18))
        .overlay { RoundedRectangle(cornerRadius: 18).strokeBorder(.primary.opacity(0.1)) }
    }
}

private struct StereoMeter: View {
    var left: Float
    var right: Float
    private let channels = ["L", "R"]

    var body: some View {
        VStack(spacing: 4) {
            ForEach(channels, id: \.self) { channel in
                let peak = channel == "L" ? left : right
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
        .font(.system(size: 10, weight: .medium, design: .monospaced))
    }
}

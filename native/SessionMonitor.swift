import SwiftUI

struct SessionMonitor: View {
    @ObservedObject var model: RecorderModel

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
        default: return SensesStyle.ready
        }
    }

    var body: some View {
        SensesMonitorCard(status: statusLabel, statusColor: indicatorColor, metadata: "Stereo · 48 kHz",
                          value: model.elapsed, valueLabel: "Elapsed time") {
            SensesStereoMeter(left: model.status.left, right: model.status.right)
                .frame(maxWidth: 550)
        } actions: {
            Group {
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
                    .tint(SensesStyle.record)
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
            .disabled(model.busy)
        }
    }
}

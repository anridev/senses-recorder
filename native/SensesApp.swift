import SwiftUI
import AppKit

private enum BrandImages {
    static let menuBar: NSImage = {
        let image = NSImage(size: NSSize(width: 22, height: 18), flipped: true) { rect in
            guard let context = NSGraphicsContext.current?.cgContext else { return false }
            context.addPath(SensesBrand.mark(in: rect.insetBy(dx: 1, dy: 1), compact: true))
            context.setFillColor(CGColor(gray: 0, alpha: 1))
            context.fillPath()
            return true
        }
        image.isTemplate = true
        return image
    }()
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        RecorderModel.shared.finish()
        return .terminateNow
    }
}

@main
struct SensesApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var delegate
    @StateObject private var model = RecorderModel.shared
    var body: some Scene {
        Window("Senses", id: "main") {
            RecorderView(model: model)
        }
        .defaultSize(width: 940, height: 650)
        .windowResizability(.contentMinSize)
        .commands {
            CommandGroup(replacing: .newItem) { }
            CommandMenu("Recording") {
                Button("Record", action: model.record).keyboardShortcut("r", modifiers: [.command])
                    .disabled(!model.canRecord)
                Button(model.status.state == "paused" ? "Resume" : "Pause", action: model.togglePause)
                    .keyboardShortcut("p", modifiers: [.command, .shift])
                    .disabled(!model.status.isActive || model.busy)
                Button("Stop and Save", action: model.stop).keyboardShortcut(".", modifiers: [.command])
                    .disabled(!model.status.isActive || model.busy)
            }
        }
        MenuBarExtra {
            MenuControls(model: model)
        } label: {
            Label {
                Text(model.status.isActive ? "Senses · \(model.elapsed)" : "Senses")
            } icon: {
                if model.status.state == "recording" {
                    Image(systemName: "record.circle.fill")
                } else {
                    Image(nsImage: BrandImages.menuBar)
                }
            }
        }
    }
}

struct MenuControls: View {
    @ObservedObject var model: RecorderModel
    @Environment(\.openWindow) private var openWindow
    var body: some View {
        Text(model.status.isActive ? "\(model.status.state.capitalized) · \(model.elapsed)" : "Ready to record")
        Text(model.sourceSummary)
        Divider()
        if model.status.isActive {
            Button(model.status.state == "paused" ? "Resume" : "Pause", action: model.togglePause)
                .disabled(model.busy)
            Button("Stop and Save", action: model.stop).disabled(model.busy)
        } else {
            Button("Record", action: model.record).disabled(!model.canRecord)
        }
        Divider()
        Button("Open Senses") { openWindow(id: "main"); NSApp.activate(ignoringOtherApps: true) }
        Button("Quit Senses") { NSApp.terminate(nil) }.keyboardShortcut("q")
    }
}

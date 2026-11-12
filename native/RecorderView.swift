import AppKit
import SwiftUI

private enum Destination: String, CaseIterable {
    case recorder = "Recorder"
    case recordings = "Recent Recordings"

    var symbol: String {
        self == .recorder ? "waveform" : "tray.full"
    }
}

struct RecorderView: View {
    @ObservedObject var model: RecorderModel
    @Environment(\.scenePhase) private var scenePhase
    @State private var destination: Destination? = .recorder
    @State private var choosingApps = false
    @State private var folderError: String?

    var body: some View {
        NavigationSplitView {
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 9) {
                    Image(nsImage: NSApp.applicationIconImage)
                        .resizable().frame(width: 36, height: 36)
                        .accessibilityHidden(true)
                    Text("Senses").font(.title2.weight(.semibold))
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 18)

                List(Destination.allCases, id: \.self, selection: $destination) { page in
                    Label(page.rawValue, systemImage: page.symbol)
                        .padding(.vertical, 3)
                        .tag(page)
                }
                .listStyle(.sidebar)

                Divider().padding(.horizontal, 15)
                HStack(spacing: 9) {
                    Image(systemName: "hifispeaker").foregroundStyle(.secondary)
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Audio Output").font(.caption).foregroundStyle(.secondary)
                        Text(model.status.output_name).font(.callout).lineLimit(2)
                    }
                }
                .padding(17)
                .accessibilityElement(children: .combine)
            }
            .navigationSplitViewColumnWidth(min: 182, ideal: 210, max: 255)
        } detail: {
            Group {
                if destination == .recordings {
                    RecordingLibrary(model: model)
                } else {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 22) {
                            SessionMonitor(model: model)
                            if let error = model.error {
                                Label(error, systemImage: "exclamationmark.triangle")
                                    .foregroundStyle(.primary)
                                    .textSelection(.enabled)
                            }
                            if let notice = model.status.notice {
                                Label(notice, systemImage: "info.circle")
                                    .foregroundStyle(.secondary)
                            }
                            RecordingOptions(model: model, choosingApps: $choosingApps)
                        }
                        .frame(maxWidth: 760)
                        .padding(24)
                        .frame(maxWidth: .infinity)
                    }
                }
            }
            .navigationTitle((destination ?? .recorder).rawValue)
            .toolbar {
                if destination == .recordings {
                    Button("Open Folder", systemImage: "folder") {
                        do { try model.openFolder() }
                        catch { folderError = error.localizedDescription }
                    }
                        .help("Open the recordings folder in Finder")
                } else {
                    Button("Audio Permission", systemImage: "lock.shield", action: model.openPermissions)
                        .help("Open system audio recording permissions")
                }
            }
        }
        .frame(minWidth: 710, minHeight: 555)
        .sheet(isPresented: $choosingApps) { AppSelection(model: model) }
        .alert("Couldn’t Open Folder", isPresented: Binding(
            get: { folderError != nil }, set: { if !$0 { folderError = nil } }
        )) {
            Button("OK", role: .cancel) { }
        } message: {
            Text(folderError ?? "")
        }
        .onChange(of: destination) { _, page in
            if page == .recordings { model.refreshRecent() }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .active { model.refreshRecent() }
        }
    }
}

private struct RecordingLibrary: View {
    @ObservedObject var model: RecorderModel

    var body: some View {
        Group {
            if model.recent.isEmpty {
                ContentUnavailableView {
                    Label("No Recordings Yet", systemImage: "waveform")
                } description: {
                    Text("Recordings saved in \(model.directory.lastPathComponent) appear here.")
                }
            } else {
                List(model.recent) { file in
                    HStack(spacing: 12) {
                        Image(systemName: "waveform")
                            .font(.title3)
                            .foregroundStyle(.teal)
                            .frame(width: 39, height: 39)
                            .background(.teal.opacity(0.1), in: RoundedRectangle(cornerRadius: 10))
                            .accessibilityHidden(true)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(file.url.lastPathComponent)
                                .font(.body.weight(.medium))
                                .lineLimit(1).truncationMode(.middle)
                            if let modified = file.modified {
                                Text(modified.formatted(date: .abbreviated, time: .shortened))
                                    .font(.caption).foregroundStyle(.secondary)
                            }
                        }
                        Spacer()
                        Button("Reveal in Finder", systemImage: "magnifyingglass") { model.reveal(file.url) }
                            .labelStyle(.iconOnly)
                            .buttonStyle(.borderless)
                            .help("Reveal \(file.url.lastPathComponent) in Finder")
                    }
                    .padding(.vertical, 5)
                    .contextMenu {
                        Button("Reveal in Finder") { model.reveal(file.url) }
                    }
                }
                .listStyle(.inset)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            HStack(spacing: 7) {
                Image(systemName: "folder").accessibilityHidden(true)
                Text(model.displayDirectory)
                    .lineLimit(1).truncationMode(.middle).help(model.directory.path)
                Spacer()
            }
            .font(.caption).foregroundStyle(.secondary)
            .padding(.horizontal, 20).padding(.vertical, 12)
            .background(.bar)
        }
    }
}

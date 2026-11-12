import SwiftUI

struct RecordingOptions: View {
    @ObservedObject var model: RecorderModel
    @Binding var choosingApps: Bool

    var body: some View {
        SensesSettingsGroup("Recording Options") {
            LabeledContent {
                Picker("Audio Source", selection: $model.allAudio) {
                    Text("All System Audio").tag(true)
                    Text("Selected Apps").tag(false)
                }
                .labelsHidden().pickerStyle(.segmented).fixedSize()
            } label: {
                Label("Audio Source", systemImage: "waveform")
            }
            if !model.allAudio {
                HStack(alignment: .top, spacing: 10) {
                    if model.selectedApps.isEmpty {
                        Text("Choose at least one app.")
                            .font(.callout).foregroundStyle(.secondary)
                        Spacer()
                    } else {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 135), alignment: .leading)],
                                  alignment: .leading, spacing: 7) {
                            ForEach(model.selectedApps) { app in
                                SourceAppLabel(app: app, iconSize: 22)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    Button("Choose Apps…") { choosingApps = true }.fixedSize()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.leading, 28).padding(.bottom, 12)
            }
            Divider()
            LabeledContent {
                Picker("File Format", selection: $model.format) {
                    Text("M4A · AAC").tag("m4a")
                    Text("WAV · 24-bit PCM").tag("wav")
                }
                .labelsHidden().fixedSize()
            } label: {
                Label("File Format", systemImage: "music.note")
            }
            Divider()
            LabeledContent {
                HStack(spacing: 9) {
                    Text(model.displayDirectory)
                        .foregroundStyle(.secondary)
                        .lineLimit(1).truncationMode(.middle)
                        .help(model.directory.path)
                    Button("Change…", action: model.chooseDirectory)
                }
            } label: {
                Label("Save To", systemImage: "folder")
            }
        }
        .disabled(model.status.isActive || model.busy)
    }
}

struct SourceAppLabel: View {
    let app: SourceApp
    let iconSize: CGFloat

    var body: some View {
        SensesIconLabel(app.name, icon: app.icon.map { Image(nsImage: $0) }, size: iconSize)
    }
}

struct AppSelection: View {
    @ObservedObject var model: RecorderModel
    @Environment(\.dismiss) private var dismiss
    @State private var search = ""

    private var matches: [SourceApp] {
        model.apps.filter { search.isEmpty || $0.name.localizedCaseInsensitiveContains(search) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 7) {
                Text("Choose Apps").font(.title2.weight(.semibold))
                Text("Open an app to see it here. Selecting a browser includes audio from all its tabs.")
                    .font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                TextField("Find an app", text: $search)
                    .textFieldStyle(.roundedBorder).padding(.top, 5)
            }.padding(20)
            Divider()
            List(matches) { app in
                Toggle(isOn: Binding(get: { model.selected.contains(app.id) }, set: { enabled in
                    if enabled { model.selected.insert(app.id) } else { model.selected.remove(app.id) }
                })) {
                    SourceAppLabel(app: app, iconSize: 25)
                }
                .toggleStyle(.checkbox).padding(.vertical, 2)
                .disabled(model.status.isActive || model.busy)
            }
            .listStyle(.inset)
            .overlay {
                if matches.isEmpty {
                    ContentUnavailableView(search.isEmpty ? "No Apps Available" : "No Matching Apps",
                                           systemImage: "magnifyingglass")
                }
            }
            Divider()
            HStack {
                Text(model.sourceSummary).font(.callout).foregroundStyle(.secondary)
                Spacer()
                Button("Done") { dismiss() }.keyboardShortcut(.defaultAction)
            }.padding(16)
        }
        .frame(width: 378, height: 434)
    }
}

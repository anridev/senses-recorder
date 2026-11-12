import AppKit
import SwiftUI
import Darwin
import IOKit.pwr_mgt

struct SourceApp: Identifiable {
    let id: String
    let name: String
    let icon: NSImage?
    var pids: Set<Int32>
    var bundles: Set<String>
    var producingAudio: Bool
}

struct RecentRecording: Identifiable {
    let url: URL
    let modified: Date?
    var id: URL { url }
}

final class RecorderModel: ObservableObject {
    static let shared = RecorderModel()
    @Published var status = RecorderStatus()
    @Published var apps: [SourceApp] = []
    @Published var selected: Set<String> {
        didSet { UserDefaults.standard.set(selected.sorted(), forKey: "selectedApps") }
    }
    @Published var allAudio: Bool { didSet { UserDefaults.standard.set(allAudio, forKey: "allAudio") } }
    @Published var format: String { didSet { UserDefaults.standard.set(format, forKey: "format") } }
    @Published var directory: URL { didSet { UserDefaults.standard.set(directory.path, forKey: "directory"); refreshRecent() } }
    @Published var recent: [RecentRecording] = []
    @Published var commandError: String?
    @Published var busy = false
    private let bridge = EngineBridge()
    private var timer: Timer?
    private var tick = 0
    private var polling = false
    private var lastMembership = ""
    private var assertion: IOPMAssertionID = 0
    private var hasAssertion = false
    private var sleepObserver: NSObjectProtocol?
    private var wasActive = false
    private var finishing = false

    private init() {
        let defaults = UserDefaults.standard
        selected = Set(defaults.stringArray(forKey: "selectedApps") ?? [])
        allAudio = defaults.object(forKey: "allAudio") as? Bool ?? true
        format = defaults.string(forKey: "format") ?? "m4a"
        directory = URL(fileURLWithPath: defaults.string(forKey: "directory") ??
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Music/Senses").path)
        updateApps([])
        refreshRecent()
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in self?.poll() }
        if let timer { RunLoop.main.add(timer, forMode: .common) }
        sleepObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.willSleepNotification, object: nil, queue: .main
        ) { [weak self] _ in
            guard let self, self.status.isActive else { return }
            self.send(["command": "interrupt", "reason": "Recording stopped because the Mac is going to sleep."])
        }
        poll()
    }

    var canRecord: Bool { !busy && !status.isActive && (allAudio || !selected.isEmpty) }
    var elapsed: String {
        let n = Int(status.seconds)
        return String(format: "%02d:%02d:%02d", n / 3600, (n / 60) % 60, n % 60)
    }
    var error: String? { commandError ?? status.error }
    var selectedApps: [SourceApp] { apps.filter { selected.contains($0.id) } }
    var displayDirectory: String { (directory.path as NSString).abbreviatingWithTildeInPath }
    var sourceSummary: String {
        allAudio ? "All system audio" : "\(selected.count) selected app\(selected.count == 1 ? "" : "s")"
    }
    func record() {
        guard canRecord else { return }
        let date = DateFormatter()
        date.dateFormat = "yyyy-MM-dd_HH-mm-ss"
        var command = membership()
        command["command"] = "start"
        command["directory"] = directory.path
        command["format"] = format
        command["stem"] = "Senses_" + date.string(from: Date())
        commandError = nil
        send(command)
    }
    func togglePause() { send(["command": status.state == "paused" ? "resume" : "pause"]) }
    func stop() { send(["command": "stop"]) }
    func send(_ command: [String: Any]) {
        busy = true
        bridge.command(command) { [weak self] error in
            guard let self, !self.finishing else { return }
            self.commandError = error
            self.busy = false
            self.poll()
        }
    }
    func chooseDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.prompt = "Save Recordings Here"
        panel.directoryURL = directory
        if panel.runModal() == .OK, let url = panel.url { directory = url }
    }
    func openPermissions() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
            NSWorkspace.shared.open(url)
        }
    }
    func reveal(_ url: URL) { NSWorkspace.shared.activateFileViewerSelecting([url]) }
    func openFolder() throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        if !NSWorkspace.shared.open(directory) { throw CocoaError(.fileReadUnknown) }
    }
    func finish() {
        finishing = true
        timer?.invalidate()
        bridge.finish()
        releaseSleepAssertion()
    }
    private func poll() {
        guard !polling, !finishing else { return }
        polling = true
        let includeSources = tick % 10 == 0
        tick += 1
        bridge.poll(includeSources: includeSources) { [weak self] status, processes in
            guard let self, !self.finishing else { return }
            self.polling = false
            if let status {
                self.status = status
                if status.isActive && !self.hasAssertion {
                    self.hasAssertion = IOPMAssertionCreateWithName(
                        kIOPMAssertionTypePreventUserIdleSystemSleep as CFString,
                        IOPMAssertionLevel(kIOPMAssertionLevelOn),
                        "Senses is recording audio" as CFString, &self.assertion) == kIOReturnSuccess
                } else if !status.isActive { self.releaseSleepAssertion() }
                if self.wasActive && !status.isActive { self.refreshRecent() }
                self.wasActive = status.isActive
            }
            if let processes {
                self.updateApps(processes)
                if self.status.isActive && !self.allAudio {
                    let member = self.membership()
                    let key = (member["pids"] as? [Int32] ?? []).map(String.init).joined(separator: ",") +
                        (member["bundles"] as? [String] ?? []).joined(separator: ",")
                    if key != self.lastMembership {
                        self.lastMembership = key
                        var command = member
                        command["command"] = "update_sources"
                        self.bridge.command(command) { [weak self] error in
                            if let error { self?.commandError = error }
                        }
                    }
                }
            }
        }
    }
    private func releaseSleepAssertion() {
        if hasAssertion { IOPMAssertionRelease(assertion); hasAssertion = false }
    }
    private func membership() -> [String: Any] {
        let available = Dictionary(uniqueKeysWithValues: apps.map {
            ($0.id, SourceMembership.Members(pids: $0.pids, bundles: $0.bundles))
        })
        let chosen = SourceMembership.selected(selected, from: available)
        return ["all": allAudio, "pids": chosen.pids.sorted(), "bundles": chosen.bundles.sorted()]
    }
    private func updateApps(_ processes: [AudioProcess]) {
        let own = Bundle.main.bundleIdentifier ?? "local.senses.recorder"
        let running = NSWorkspace.shared.runningApplications.filter {
            let path = $0.bundleURL?.path ?? ""
            let userApp = path.hasSuffix(".app") && !path.contains(".app/") &&
                !path.hasPrefix("/System/") && !path.hasPrefix("/usr/")
            return ($0.activationPolicy == .regular || userApp) && $0.bundleIdentifier != own
        }
        var grouped: [String: SourceApp] = [:]
        for app in running {
            guard let id = app.bundleIdentifier else { continue }
            grouped[id] = SourceApp(id: id, name: app.localizedName ?? id, icon: app.icon,
                                    pids: [app.processIdentifier], bundles: [id], producingAudio: false)
        }
        let identities = running.compactMap { app -> SourceMembership.Application? in
            guard let id = app.bundleIdentifier else { return nil }
            return SourceMembership.Application(id: id, pid: app.processIdentifier, path: app.bundleURL?.path ?? "")
        }
        for process in processes where process.pid != ProcessInfo.processInfo.processIdentifier {
            let native = NSRunningApplication(processIdentifier: process.pid)
            let path = executablePath(process.pid)
            let owner = SourceMembership.owner(pid: process.pid, bundle: process.bundle_id, path: path,
                                               ancestors: ancestors(process.pid), apps: identities)
            let id = owner ?? (process.bundle_id.isEmpty ? "pid:\(process.pid)" : process.bundle_id)
            if id == own || id.hasPrefix(own + ".") { continue }
            // Background macOS services are covered by All audio. The app
            // picker shows user applications, including their audio helpers.
            if owner == nil && native?.activationPolicy != .regular { continue }
            if grouped[id] == nil {
                grouped[id] = SourceApp(id: id, name: native?.localizedName ?? id,
                                        icon: native?.icon, pids: [], bundles: [], producingAudio: true)
            }
            grouped[id]?.pids.insert(process.pid)
            if !process.bundle_id.isEmpty { grouped[id]?.bundles.insert(process.bundle_id) }
            grouped[id]?.producingAudio = true
        }
        // Keep selected apps visible after exit so they can be deselected or relaunched.
        for previous in apps where selected.contains(previous.id) && grouped[previous.id] == nil {
            var dormant = previous
            dormant.pids = []
            dormant.producingAudio = false
            grouped[previous.id] = dormant
        }
        for id in selected where grouped[id] == nil {
            let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: id)
            let bundle = url.flatMap { Bundle(url: $0) }
            let name = bundle?.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ??
                bundle?.object(forInfoDictionaryKey: "CFBundleName") as? String ?? id
            grouped[id] = SourceApp(id: id, name: name, icon: url.map { NSWorkspace.shared.icon(forFile: $0.path) },
                                    pids: [], bundles: [id], producingAudio: false)
        }
        apps = grouped.values.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }
    private func executablePath(_ pid: Int32) -> String {
        var buffer = [CChar](repeating: 0, count: 4 * Int(MAXPATHLEN))
        let size = proc_pidpath(pid, &buffer, UInt32(buffer.count))
        return size > 0 ? String(cString: buffer) : ""
    }
    private func ancestors(_ pid: Int32) -> [Int32] {
        var current = pid
        var result: [Int32] = []
        for _ in 0..<12 {
            var info = proc_bsdinfo()
            let size = Int32(MemoryLayout<proc_bsdinfo>.size)
            guard proc_pidinfo(current, PROC_PIDTBSDINFO, 0, &info, size) == size else { break }
            current = Int32(info.pbi_ppid)
            if current <= 1 || result.contains(current) { break }
            result.append(current)
        }
        return result
    }
    func refreshRecent() {
        let urls = (try? FileManager.default.contentsOfDirectory(at: directory,
            includingPropertiesForKeys: [.contentModificationDateKey, .isRegularFileKey],
            options: .skipsHiddenFiles)) ?? []
        let recordings = urls.compactMap { url -> RecentRecording? in
            guard ["wav", "m4a"].contains(url.pathExtension.lowercased()),
                  let values = try? url.resourceValues(forKeys: [.contentModificationDateKey, .isRegularFileKey]),
                  values.isRegularFile == true else { return nil }
            return RecentRecording(url: url, modified: values.contentModificationDate)
        }
        recent = Array(recordings.sorted {
            ($0.modified ?? .distantPast) > ($1.modified ?? .distantPast)
        }.prefix(12))
    }
}

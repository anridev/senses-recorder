import Foundation

/// Pure source resolution, independent of AppKit and the audio device.
enum SourceMembership {
    struct Application {
        let id: String
        let pid: Int32
        let path: String
    }
    struct Members {
        var pids: Set<Int32>
        var bundles: Set<String>
    }

    static func owner(pid: Int32, bundle: String, path: String,
                      ancestors: [Int32], apps: [Application]) -> String? {
        if let exact = apps.first(where: { $0.pid == pid }) { return exact.id }
        if !bundle.isEmpty, let exact = apps.first(where: { $0.id == bundle }) { return exact.id }
        // A path boundary prevents Chrome.app from also matching Chrome Beta.app.
        if let enclosing = apps.filter({ !$0.path.isEmpty && path.hasPrefix($0.path + "/") })
            .max(by: { $0.path.count < $1.path.count }) { return enclosing.id }
        // XPC helpers can live outside their app. Prefer the closest known parent.
        for ancestor in ancestors {
            if let parent = apps.first(where: { $0.pid == ancestor }) { return parent.id }
        }
        return nil
    }

    static func selected(_ ids: Set<String>, from apps: [String: Members]) -> Members {
        var result = Members(pids: [], bundles: ids)
        for id in ids {
            guard let app = apps[id] else { continue }
            result.pids.formUnion(app.pids)
            // Shared helpers (for example WebKit) can use the same bundle ID
            // for unrelated apps. Capture those by their attributed PID only.
            result.bundles.formUnion(app.bundles.filter { $0 == id || $0.hasPrefix(id + ".") })
        }
        return result
    }
}

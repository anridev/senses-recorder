import Foundation

@main
enum SourceMembershipTests {
    static func expect(_ condition: @autoclosure () -> Bool, _ message: String) {
        if !condition() { fatalError(message) }
    }

    static func main() {
        typealias App = SourceMembership.Application
        typealias Members = SourceMembership.Members
        let browser = App(id: "com.example.browser", pid: 100, path: "/Applications/Browser.app")
        let beta = App(id: "com.example.browser.beta", pid: 200, path: "/Applications/Browser Beta.app")
        let music = App(id: "com.example.music", pid: 300, path: "/Applications/Music.app")
        let apps = [browser, beta, music]
        func owner(_ pid: Int32, _ bundle: String = "", _ path: String = "", _ parents: [Int32] = []) -> String? {
            SourceMembership.owner(pid: pid, bundle: bundle, path: path, ancestors: parents, apps: apps)
        }
        expect(owner(100) == browser.id, "Main process must resolve by PID")
        expect(owner(900, music.id, "", [100]) == music.id, "Exact bundle must win over an unrelated ancestor")
        expect(owner(101, "helper", browser.path + "/Contents/Frameworks/Helper.app/Contents/MacOS/Helper") == browser.id,
               "Nested browser helper must resolve by executable path")
        expect(owner(201, "helper", beta.path + "/Contents/MacOS/Helper") == beta.id,
               "Similarly named browser installations must stay separate")
        expect(owner(202, "helper", browser.path + "-fake/Contents/MacOS/Helper") == nil,
               "A string prefix without a path boundary must not attach a process")
        expect(owner(102, "com.apple.WebKit.WebContent", "/System/Library/WebKit/Helper", [999, 200, 100]) == beta.id,
               "External helper must attach to its closest known ancestor")
        expect(owner(999, "com.apple.WebKit.WebContent") == nil,
               "A shared helper without ownership evidence must not attach to an arbitrary browser")

        let available: [String: Members] = [
            browser.id: Members(pids: [100, 101, 102], bundles: [browser.id, browser.id + ".helper", "com.apple.WebKit.WebContent"]),
            music.id: Members(pids: [300, 301], bundles: [music.id])
        ]
        let single = SourceMembership.selected([browser.id], from: available)
        expect(single.pids == [100, 101, 102], "Browser selection must include its helpers and exclude the music app")
        expect(single.bundles == [browser.id, browser.id + ".helper"],
               "Shared helper bundle IDs must not broaden capture to unrelated apps")
        let multiple = SourceMembership.selected([browser.id, music.id], from: available)
        expect(multiple.pids == [100, 101, 102, 300, 301], "Multiple apps must combine without losing helpers")

        let closed = SourceMembership.selected([browser.id], from: [:])
        expect(closed.pids.isEmpty && closed.bundles == [browser.id], "Exited apps must retain bundle identity without stale PIDs")
        let restarted = SourceMembership.selected([browser.id], from: [
            browser.id: Members(pids: [400, 401], bundles: [browser.id, browser.id + ".helper"])
        ])
        expect(restarted.pids == [400, 401] && restarted.bundles.contains(browser.id),
               "Relaunch must replace process membership while retaining the selected app")
        print("Source membership: 12 checks passed")
    }
}

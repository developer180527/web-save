// WebSave Menubar — native AppKit quick access over the shared vault.
//
// Deliberately AppKit (not SwiftUI): an NSPopover + NSTableView idles at a
// fraction of the SwiftUI runtime's memory footprint, which matters for an
// always-running menubar utility. Vault access is direct via UniFFI
// bindings over websave-core; the desktop engine is nudged over localhost
// after writes so its UI stays in sync.

import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private let popover = NSPopover()

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let button = statusItem.button {
            button.image = NSImage(
                systemSymbolName: "bookmark.fill",
                accessibilityDescription: "WebSave"
            )
            button.target = self
            button.action = #selector(togglePopover(_:))
        }

        let panel = PanelViewController()
        panel.popover = popover
        popover.behavior = .transient
        popover.contentViewController = panel
        popover.contentSize = NSSize(width: 340, height: 460)
    }

    @objc private func togglePopover(_ sender: Any?) {
        guard let button = statusItem.button else { return }
        if popover.isShown {
            popover.performClose(sender)
        } else {
            NSApp.activate(ignoringOtherApps: true)
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
            popover.contentViewController?.view.window?.makeKey()
        }
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()

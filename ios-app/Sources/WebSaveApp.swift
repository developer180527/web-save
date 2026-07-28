// WebSave for iPad — a native SwiftUI front end over the same websave-core
// engine that powers the desktop app, linked via UniFFI. Storage, search,
// tagging and link classification are all reused; only this UI is new.
//
// iOS has no background daemon, so health checking runs in the FOREGROUND:
// the app sweeps stale links when it launches and when it returns to the
// foreground. Statuses are therefore as fresh as your last visit — the
// desktop app remains the place for round-the-clock monitoring.

import SwiftUI

@main
struct WebSaveApp: App {
    @StateObject private var store = VaultStore()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            LibraryView()
                .environmentObject(store)
                .task { await store.sweepHealth() }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .active {
                Task { await store.reload(); await store.sweepHealth() }
            }
        }
    }
}

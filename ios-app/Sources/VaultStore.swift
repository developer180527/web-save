import Foundation

/// Bridges SwiftUI to the Rust core. Opens the vault, exposes the current
/// list, and runs foreground health sweeps off the main actor.
@MainActor
final class VaultStore: ObservableObject {
    @Published var saves: [SaveSummary] = []
    @Published var query = "" { didSet { scheduleReload() } }
    @Published var checking = false
    @Published var errorMessage: String?

    private let vault: VaultHandle?
    private var reloadTask: Task<Void, Never>?

    init() {
        do {
            // Documents dir today; a shared App Group container once the
            // Share Extension lands (both link the same core over one vault).
            let dir = FileManager.default
                .urls(for: .documentDirectory, in: .userDomainMask)[0]
                .appendingPathComponent("vault")
            vault = try VaultHandle(path: dir.path)
        } catch {
            vault = nil
            errorMessage = "Could not open vault: \(error.localizedDescription)"
        }
        reload()
    }

    func reload() {
        guard let vault else { return }
        do {
            let q = query.trimmingCharacters(in: .whitespaces)
            saves = q.isEmpty
                ? try vault.recent(limit: 500)
                : try vault.search(query: q, limit: 200)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func scheduleReload() {
        reloadTask?.cancel()
        reloadTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(150))
            guard !Task.isCancelled else { return }
            self?.reload()
        }
    }

    func add(_ rawUrl: String) {
        guard let vault else { return }
        var url = rawUrl.trimmingCharacters(in: .whitespaces)
        guard !url.isEmpty else { return }
        if !url.lowercased().hasPrefix("http") { url = "https://" + url }
        do {
            _ = try vault.add(url: url, title: "")
            reload()
            Task { await sweepHealth() }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func toggleFavorite(_ save: SaveSummary) {
        guard let vault else { return }
        try? vault.setFavorite(id: save.id, favorite: !save.favorite)
        reload()
    }

    func delete(_ save: SaveSummary) {
        guard let vault else { return }
        try? vault.delete(id: save.id)
        reload()
    }

    /// Check whichever links the core's schedule says are due — network work
    /// happens off the main actor so the UI never blocks.
    ///
    /// Bounded by both a count and 20 seconds of wall time: the app may be
    /// backgrounded at any moment, and anything the sweep doesn't reach
    /// simply stays due for next launch.
    func sweepHealth() async {
        guard let vault, !checking else { return }
        checking = true
        defer { checking = false }
        let checked = await Task.detached(priority: .utility) {
            (try? vault.checkDue(limit: 40, budgetSecs: 20)) ?? 0
        }.value
        if checked > 0 { reload() }
    }
}

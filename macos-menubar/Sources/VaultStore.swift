import AppKit

let engineBase = "http://127.0.0.1:38917"
let engineBundleId = "com.venugopal.web-save"

/// Thin synchronous facade over the UniFFI vault handle. Queries are local
/// SQLite reads — fast enough to run on the main thread for panel-sized
/// result sets.
final class VaultStore {
    static let shared = VaultStore()

    private(set) var vault: VaultHandle?
    private(set) var openError: String?

    private init() {
        do {
            let dir = FileManager.default
                .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
                .appendingPathComponent(engineBundleId)
                .appendingPathComponent("vault")
            vault = try VaultHandle(path: dir.path)
        } catch {
            openError = "Could not open vault: \(error.localizedDescription)"
        }
    }

    func starred(limit: Int64 = 12) -> [SaveSummary] {
        (try? vault?.starred(limit: limit)) ?? []
    }

    func recent(limit: Int64 = 20) -> [SaveSummary] {
        (try? vault?.recent(limit: limit)) ?? []
    }

    func search(_ query: String, limit: Int64 = 20) -> [SaveSummary] {
        (try? vault?.search(query: query, limit: limit)) ?? []
    }

    func toggleStar(_ save: SaveSummary) {
        try? vault?.setFavorite(id: save.id, favorite: !save.favorite)
        ping(path: "/reload")
    }

    func open(_ save: SaveSummary) {
        if let url = URL(string: save.url) {
            NSWorkspace.shared.open(url)
        }
    }

    func openMainApp() {
        // A running engine raises its own window (works for dev builds too);
        // otherwise launch the installed app.
        request(path: "/show") { ok in
            guard !ok else { return }
            DispatchQueue.main.async {
                if let appURL = NSWorkspace.shared
                    .urlForApplication(withBundleIdentifier: engineBundleId)
                {
                    NSWorkspace.shared.openApplication(
                        at: appURL,
                        configuration: NSWorkspace.OpenConfiguration()
                    )
                }
            }
        }
    }

    private func ping(path: String) {
        request(path: path) { _ in }
    }

    private func request(path: String, completion: @escaping (Bool) -> Void) {
        guard let url = URL(string: engineBase + path) else { return }
        var req = URLRequest(url: url)
        req.timeoutInterval = 1.5
        URLSession.shared.dataTask(with: req) { _, response, error in
            let ok = error == nil
                && (response as? HTTPURLResponse)?.statusCode == 200
            completion(ok)
        }.resume()
    }
}

/// Small async favicon fetcher with an in-memory cache.
final class FaviconLoader {
    static let shared = FaviconLoader()
    private var cache: [String: NSImage] = [:]

    func load(for save: SaveSummary, into imageView: NSImageView) {
        let host = Self.host(of: save.url)
        let urlString = save.faviconUrl.isEmpty
            ? "https://icons.duckduckgo.com/ip3/\(host).ico"
            : save.faviconUrl

        imageView.image = NSImage(systemSymbolName: "globe", accessibilityDescription: nil)
        imageView.contentTintColor = .tertiaryLabelColor
        // Remember which favicon this cell currently wants, so a slow fetch
        // for a reused cell can't overwrite a newer row's icon.
        imageView.identifier = NSUserInterfaceItemIdentifier(urlString)

        if let cached = cache[urlString] {
            imageView.image = cached
            imageView.contentTintColor = nil
            return
        }
        guard let url = URL(string: urlString) else { return }
        URLSession.shared.dataTask(with: url) { [weak self, weak imageView] data, _, _ in
            guard let data, let image = NSImage(data: data) else { return }
            DispatchQueue.main.async {
                self?.cache[urlString] = image
                if imageView?.identifier?.rawValue == urlString {
                    imageView?.image = image
                    imageView?.contentTintColor = nil
                }
            }
        }.resume()
    }

    static func host(of urlString: String) -> String {
        guard let host = URL(string: urlString)?.host else { return urlString }
        return host.hasPrefix("www.") ? String(host.dropFirst(4)) : host
    }
}

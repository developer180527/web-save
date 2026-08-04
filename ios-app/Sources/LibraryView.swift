import SwiftUI

struct LibraryView: View {
    @EnvironmentObject var store: VaultStore
    @State private var showAdd = false

    var body: some View {
        NavigationStack {
            List {
                if store.saves.isEmpty {
                    ContentUnavailableView(
                        "No saves yet",
                        systemImage: "bookmark",
                        description: Text("Add a URL, or share a page to WebSave.")
                    )
                    .listRowSeparator(.hidden)
                }
                ForEach(store.saves, id: \.id) { save in
                    SaveRow(save: save)
                        .contentShape(Rectangle())
                        .onTapGesture { open(save) }
                        .swipeActions(edge: .trailing) {
                            Button(role: .destructive) {
                                store.delete(save)
                            } label: { Label("Delete", systemImage: "trash") }
                        }
                        .swipeActions(edge: .leading) {
                            Button {
                                store.toggleFavorite(save)
                            } label: {
                                Label("Star", systemImage: save.favorite ? "star.slash" : "star")
                            }
                            .tint(.yellow)
                        }
                }
            }
            .listStyle(.plain)
            .navigationTitle("WebSave")
            .searchable(text: $store.query, prompt: "Search titles, URLs, notes, tags")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    if store.checking {
                        ProgressView()
                    } else {
                        Button {
                            Task { await store.sweepHealth() }
                        } label: { Image(systemName: "arrow.clockwise") }
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showAdd = true } label: { Image(systemName: "plus") }
                }
            }
            .refreshable { await store.sweepHealth() }
            .alert(
                "Something went wrong",
                isPresented: .constant(store.errorMessage != nil),
                actions: { Button("OK") { store.errorMessage = nil } },
                message: { Text(store.errorMessage ?? "") }
            )
            .sheet(isPresented: $showAdd) {
                AddSheet { url in store.add(url) }
                    .presentationDetents([.height(200)])
            }
        }
    }

    private func open(_ save: SaveSummary) {
        if let url = URL(string: save.url) { UIApplication.shared.open(url) }
    }
}

/// Minimal add-URL sheet. A sheet (not an alert) so the keyboard and the
/// URL field behave reliably.
struct AddSheet: View {
    let onSave: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var url = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("https://…", text: $url)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)
                    .submitLabel(.done)
                    .onSubmit(save)
            }
            .navigationTitle("Save a URL")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save", action: save)
                        .disabled(url.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
    }

    private func save() {
        onSave(url)
        dismiss()
    }
}

struct SaveRow: View {
    let save: SaveSummary

    private var host: String {
        (URL(string: save.url)?.host ?? save.url).replacingOccurrences(
            of: "^www\\.", with: "", options: .regularExpression)
    }

    var body: some View {
        HStack(spacing: 12) {
            AsyncImage(
                url: URL(
                    string: save.faviconUrl.isEmpty
                        ? "https://icons.duckduckgo.com/ip3/\(host).ico"
                        : save.faviconUrl)
            ) { img in
                img.resizable()
            } placeholder: {
                Image(systemName: "globe").foregroundStyle(.secondary)
            }
            .frame(width: 26, height: 26)
            .clipShape(RoundedRectangle(cornerRadius: 6))

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    if !save.isRead {
                        Circle().fill(.blue).frame(width: 7, height: 7)
                    }
                    Text(save.title.isEmpty ? host : save.title)
                        .font(.body.weight(.medium))
                        .lineLimit(1)
                    if save.favorite {
                        Image(systemName: "star.fill")
                            .font(.caption2).foregroundStyle(.yellow)
                    }
                }
                HStack(spacing: 6) {
                    Text(host).font(.caption).foregroundStyle(.secondary)
                    StatusBadge(status: save.status)
                    if !save.tags.isEmpty {
                        Text(save.tags.map { "#\($0)" }.joined(separator: " "))
                            .font(.caption2).foregroundStyle(.tertiary)
                            .lineLimit(1)
                    }
                }
            }
            Spacer()
        }
        .padding(.vertical, 3)
    }
}

struct StatusBadge: View {
    let status: String

    private var color: Color {
        switch status {
        case "active": return .green
        case "changed": return .orange
        case "redirected": return .blue
        // Amber, not red: a link on a network we're not attached to is not
        // broken, and must not be presented as rot.
        case "unreachable": return .orange
        case "dead": return .red
        default: return .secondary
        }
    }

    var body: some View {
        if status != "unchecked" {
            Text(status.capitalized)
                .font(.caption2.weight(.semibold))
                .foregroundStyle(color)
                .padding(.horizontal, 6).padding(.vertical, 1)
                .background(color.opacity(0.15), in: Capsule())
        }
    }
}

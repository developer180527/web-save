import AppKit

/// The popover content: search field, sectioned list (starred / recent /
/// search results), footer with "Open WebSave" and quit.
final class PanelViewController: NSViewController,
    NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate
{
    enum Row {
        case header(String)
        case save(SaveSummary)
        case message(String)
    }

    weak var popover: NSPopover?

    private let store = VaultStore.shared
    private var rows: [Row] = []

    private let searchField = NSSearchField()
    private let tableView = NSTableView()

    override func loadView() {
        view = NSView(frame: NSRect(x: 0, y: 0, width: 340, height: 460))

        searchField.placeholderString = "Search saves…"
        searchField.delegate = self
        searchField.focusRingType = .none

        tableView.addTableColumn(NSTableColumn(identifier: .init("main")))
        tableView.headerView = nil
        tableView.backgroundColor = .clear
        tableView.intercellSpacing = NSSize(width: 0, height: 2)
        tableView.selectionHighlightStyle = .none
        tableView.dataSource = self
        tableView.delegate = self
        tableView.target = self
        tableView.action = #selector(rowClicked)

        let scroll = NSScrollView()
        scroll.documentView = tableView
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false

        let openButton = NSButton(
            title: "Open WebSave", target: self, action: #selector(openMainApp)
        )
        openButton.bezelStyle = .accessoryBarAction

        let quitButton = NSButton(
            image: NSImage(systemSymbolName: "power", accessibilityDescription: "Quit")!,
            target: NSApp, action: #selector(NSApplication.terminate(_:))
        )
        quitButton.bezelStyle = .accessoryBarAction
        quitButton.toolTip = "Quit WebSave Menubar"

        let footerSpacer = NSView()
        footerSpacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let footer = NSStackView(views: [openButton, footerSpacer, quitButton])
        footer.orientation = .horizontal

        let topSeparator = NSBox()
        topSeparator.boxType = .separator
        let bottomSeparator = NSBox()
        bottomSeparator.boxType = .separator

        let stack = NSStackView(
            views: [searchField, topSeparator, scroll, bottomSeparator, footer]
        )
        stack.orientation = .vertical
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 10, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false

        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: view.topAnchor),
            stack.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: view.trailingAnchor),
        ])
    }

    override func viewWillAppear() {
        super.viewWillAppear()
        // Fresh data every time the panel pops open.
        searchField.stringValue = ""
        reload()
        view.window?.makeFirstResponder(searchField)
    }

    func controlTextDidChange(_ obj: Notification) {
        reload()
    }

    private func reload() {
        let query = searchField.stringValue.trimmingCharacters(in: .whitespaces)
        rows = []

        if let error = store.openError {
            rows.append(.message(error))
        } else if query.isEmpty {
            let starred = store.starred()
            rows.append(.header("Starred"))
            if starred.isEmpty {
                rows.append(.message("No starred saves yet"))
            } else {
                rows.append(contentsOf: starred.map(Row.save))
            }
            let starredIds = Set(starred.map(\.id))
            let recent = store.recent().filter { !starredIds.contains($0.id) }.prefix(8)
            if !recent.isEmpty {
                rows.append(.header("Recent"))
                rows.append(contentsOf: recent.map(Row.save))
            }
        } else {
            let results = store.search(query)
            rows = results.isEmpty
                ? [.message("No matches")]
                : [.header("Results")] + results.map(Row.save)
        }
        tableView.reloadData()
    }

    // MARK: actions

    @objc private func rowClicked() {
        let index = tableView.clickedRow
        guard index >= 0, case .save(let save) = rows[index] else { return }
        store.open(save)
        popover?.performClose(nil)
    }

    @objc private func openMainApp() {
        store.openMainApp()
        popover?.performClose(nil)
    }

    private func starClicked(_ save: SaveSummary) {
        store.toggleStar(save)
        reload()
    }

    // MARK: table

    func numberOfRows(in tableView: NSTableView) -> Int {
        rows.count
    }

    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
        switch rows[row] {
        case .header: return 22
        case .save: return 40
        case .message: return 28
        }
    }

    func tableView(
        _ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int
    ) -> NSView? {
        switch rows[row] {
        case .header(let title):
            return LabelCell(text: title.uppercased(), style: .header)
        case .message(let text):
            return LabelCell(text: text, style: .message)
        case .save(let save):
            let cell = SaveCell()
            cell.configure(with: save) { [weak self] in
                self?.starClicked(save)
            }
            return cell
        }
    }
}

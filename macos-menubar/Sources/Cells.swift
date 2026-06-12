import AppKit

/// Section header / informational row.
final class LabelCell: NSView {
    enum Style {
        case header
        case message
    }

    init(text: String, style: Style) {
        super.init(frame: .zero)
        let label = NSTextField(labelWithString: text)
        switch style {
        case .header:
            label.font = .systemFont(ofSize: 10.5, weight: .semibold)
            label.textColor = .secondaryLabelColor
        case .message:
            label.font = .systemFont(ofSize: 12)
            label.textColor = .secondaryLabelColor
        }
        label.translatesAutoresizingMaskIntoConstraints = false
        addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            label.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -8),
            label.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }
}

/// One save: favicon, title + host, star toggle.
final class SaveCell: NSView {
    private let iconView = NSImageView()
    private let titleLabel = NSTextField(labelWithString: "")
    private let hostLabel = NSTextField(labelWithString: "")
    private let starButton = NSButton()
    private var onStar: (() -> Void)?

    init() {
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 7

        iconView.wantsLayer = true
        iconView.layer?.cornerRadius = 5
        iconView.layer?.masksToBounds = true
        iconView.imageScaling = .scaleProportionallyUpOrDown

        titleLabel.font = .systemFont(ofSize: 12.5, weight: .medium)
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        hostLabel.font = .systemFont(ofSize: 10.5)
        hostLabel.textColor = .secondaryLabelColor
        hostLabel.lineBreakMode = .byTruncatingTail
        hostLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        starButton.isBordered = false
        starButton.target = self
        starButton.action = #selector(starPressed)

        let textStack = NSStackView(views: [titleLabel, hostLabel])
        textStack.orientation = .vertical
        textStack.alignment = .leading
        textStack.spacing = 1
        // Text expands, pushing the star to the trailing edge so all stars
        // line up in one column.
        textStack.setContentHuggingPriority(.defaultLow, for: .horizontal)

        let stack = NSStackView(views: [iconView, textStack, starButton])
        stack.orientation = .horizontal
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 0, left: 8, bottom: 0, right: 8)
        stack.translatesAutoresizingMaskIntoConstraints = false

        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            iconView.widthAnchor.constraint(equalToConstant: 24),
            iconView.heightAnchor.constraint(equalToConstant: 24),
            starButton.widthAnchor.constraint(equalToConstant: 22),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    func configure(with save: SaveSummary, onStar: @escaping () -> Void) {
        self.onStar = onStar
        let host = FaviconLoader.host(of: save.url)
        titleLabel.stringValue = save.title.isEmpty ? host : save.title
        hostLabel.stringValue = host
        toolTip = save.url

        starButton.image = NSImage(
            systemSymbolName: save.favorite ? "star.fill" : "star",
            accessibilityDescription: save.favorite ? "Unstar" : "Star"
        )
        starButton.contentTintColor = save.favorite ? .systemYellow : .tertiaryLabelColor

        FaviconLoader.shared.load(for: save, into: iconView)
    }

    @objc private func starPressed() {
        onStar?()
    }

    // Hover highlight, the AppKit way.
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
            owner: self
        ))
    }

    override func mouseEntered(with event: NSEvent) {
        layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.07).cgColor
    }

    override func mouseExited(with event: NSEvent) {
        layer?.backgroundColor = nil
    }
}

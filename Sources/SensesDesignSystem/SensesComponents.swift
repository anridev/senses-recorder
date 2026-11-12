import SwiftUI

public enum SensesStyle {
    public static let displayStart = Color(red: 0.11, green: 0.21, blue: 0.20)
    public static let displayEnd = Color(red: 0.055, green: 0.09, blue: 0.105)
    public static let ready = Color(red: 0.55, green: 0.93, blue: 0.80)
    public static let record = Color(red: 0.78, green: 0.15, blue: 0.18)
    public static let readout = Font.system(size: 65, weight: .light, design: .rounded).monospacedDigit()
    public static let meterLabel = Font.system(size: 10, weight: .medium, design: .monospaced)
    public static let monitorRadius: CGFloat = 18
    public static let settingsRadius: CGFloat = 14
    public static let pageInset: CGFloat = 24
    public static let sectionSpacing: CGFloat = 22
    public static let maxContentWidth: CGFloat = 760
}

/// A dark value display with a status header and a native action bar.
/// Supply a chart, meter, or EmptyView below the value.
public struct SensesMonitorCard<Detail: View, Actions: View>: View {
    let status: String
    let statusColor: Color
    let metadata: String
    let value: String
    let valueLabel: String
    let detail: Detail
    let actions: Actions

    public init(status: String, statusColor: Color = SensesStyle.ready, metadata: String,
                value: String, valueLabel: String, @ViewBuilder detail: () -> Detail,
                @ViewBuilder actions: () -> Actions) {
        self.status = status
        self.statusColor = statusColor
        self.metadata = metadata
        self.value = value
        self.valueLabel = valueLabel
        self.detail = detail()
        self.actions = actions()
    }

    public var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 19) {
                HStack {
                    HStack(spacing: 7) {
                        Circle().fill(statusColor).frame(width: 7, height: 7).accessibilityHidden(true)
                        Text(status).font(.callout.weight(.medium))
                    }
                    Spacer()
                    Text(metadata).font(.callout).foregroundStyle(.white.opacity(0.72))
                }
                Text(value)
                    .font(SensesStyle.readout)
                    .lineLimit(1).minimumScaleFactor(0.7)
                    .accessibilityLabel(valueLabel).accessibilityValue(value)
                detail
            }
            .foregroundStyle(.white)
            .padding(.horizontal, 25).padding(.top, 22).padding(.bottom, 21)
            .background {
                LinearGradient(colors: [SensesStyle.displayStart, SensesStyle.displayEnd],
                               startPoint: .topLeading, endPoint: .bottomTrailing)
            }
            HStack(spacing: 10) { actions }
                .frame(maxWidth: .infinity, alignment: .leading)
                .controlSize(.large)
                .padding(.horizontal, 19).padding(.vertical, 14)
                .background(.background.secondary)
        }
        .clipShape(RoundedRectangle(cornerRadius: SensesStyle.monitorRadius))
        .overlay { RoundedRectangle(cornerRadius: SensesStyle.monitorRadius).strokeBorder(.primary.opacity(0.1)) }
    }
}

/// Use LabeledContent for each setting and Divider between rows.
public struct SensesSettingsGroup<Content: View>: View {
    let title: LocalizedStringKey
    let content: Content

    public init(_ title: LocalizedStringKey, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.headline)
            VStack(spacing: 0) { content }
                .labelStyle(OptionsLabelStyle())
                .labeledContentStyle(OptionsRowStyle())
                .padding(.horizontal, 15)
                .background(.background.secondary, in: RoundedRectangle(cornerRadius: SensesStyle.settingsRadius))
                .overlay { RoundedRectangle(cornerRadius: SensesStyle.settingsRadius).strokeBorder(.primary.opacity(0.08)) }
        }
    }
}

/// A named item with a platform image, or an SF Symbol when no image is available.
public struct SensesIconLabel: View {
    let name: String
    let icon: Image
    let size: CGFloat

    public init(_ name: String, icon: Image? = nil, size: CGFloat = 22) {
        self.name = name
        self.icon = icon ?? Image(systemName: "app")
        self.size = size
    }

    public var body: some View {
        HStack(spacing: 6) {
            icon.resizable().aspectRatio(contentMode: .fit)
                .frame(width: size, height: size)
                .accessibilityHidden(true)
            Text(name).lineLimit(2)
        }
        .help(name)
    }
}

private struct OptionsLabelStyle: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 10) {
            configuration.icon.foregroundStyle(.secondary).frame(width: 18)
            configuration.title
        }
    }
}

private struct OptionsRowStyle: LabeledContentStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 12) {
            configuration.label.fixedSize()
            Spacer(minLength: 0)
            configuration.content
        }
        .padding(.vertical, 10)
        .frame(minHeight: 45)
    }
}

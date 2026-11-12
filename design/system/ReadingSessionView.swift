import SensesDesignSystem
import SwiftUI

struct ReadingSessionView: View {
    @State private var pagesRead = 0
    @State private var dailyGoal = 20

    var body: some View {
        VStack(alignment: .leading, spacing: SensesStyle.sectionSpacing) {
            SensesMonitorCard(status: pagesRead >= dailyGoal ? "Goal reached" : "Reading",
                              metadata: "Daily goal · \(dailyGoal) pages",
                              value: String(pagesRead), valueLabel: "Pages read") {
                ProgressView(value: Double(min(pagesRead, dailyGoal)), total: Double(dailyGoal))
                    .tint(SensesStyle.ready)
                    .accessibilityLabel("Daily reading progress")
            } actions: {
                Button("Mark Page Read") { pagesRead += 1 }
                    .buttonStyle(.borderedProminent)
                Spacer()
                SensesIconLabel("Books", icon: Image(systemName: "book.closed"), size: 20)
            }

            SensesSettingsGroup("Reading Options") {
                LabeledContent {
                    Stepper("\(dailyGoal) pages", value: $dailyGoal, in: 1...200)
                } label: {
                    Label("Daily Goal", systemImage: "target")
                }
            }
        }
        .frame(maxWidth: SensesStyle.maxContentWidth)
        .padding(SensesStyle.pageInset)
    }
}

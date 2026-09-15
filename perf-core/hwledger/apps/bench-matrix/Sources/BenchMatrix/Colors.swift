import SwiftUI

enum BenchColors {
    static let bg = Color(red: 0.08, green: 0.08, blue: 0.10)
    static let surface = Color(red: 0.12, green: 0.12, blue: 0.15)
    static let border = Color(red: 0.22, green: 0.22, blue: 0.28)
    static let text = Color(red: 0.88, green: 0.88, blue: 0.92)
    static let textDim = Color(red: 0.55, green: 0.55, blue: 0.62)
    static let headerBg = Color(red: 0.10, green: 0.10, blue: 0.13)

    static let gray = Color(red: 0.18, green: 0.18, blue: 0.22)
    static let orange = Color(red: 0.90, green: 0.55, blue: 0.15)
    static let red = Color(red: 0.85, green: 0.22, blue: 0.22)
    static let yellow = Color(red: 0.88, green: 0.72, blue: 0.18)
    static let green = Color(red: 0.22, green: 0.72, blue: 0.35)

    static func cellColor(for status: CellStatus, score: Double?) -> Color {
        switch status {
        case .noData:
            return gray
        case .running:
            return orange
        case .failed:
            return red.opacity(0.6 + (score ?? 0) * 0.4)
        case .partial:
            let t = score.map { ($0 - 0.5) / 0.2 } ?? 0.5
            return yellow.opacity(0.5 + t * 0.5)
        case .passed:
            let t = score.map { ($0 - 0.7) / 0.3 } ?? 0.85
            return green.opacity(0.5 + min(t, 1.0) * 0.5)
        }
    }
}

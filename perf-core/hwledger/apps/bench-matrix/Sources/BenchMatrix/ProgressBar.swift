import SwiftUI

struct ProgressBar: View {
    let complete: Int
    let total: Int
    let fraction: Double

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text("\(complete)/\(total) complete")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundStyle(BenchColors.text)
                Spacer()
                Text("\(Int(fraction * 100))%")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundStyle(BenchColors.textDim)
            }

            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 3)
                        .fill(BenchColors.border)
                    RoundedRectangle(cornerRadius: 3)
                        .fill(barColor)
                        .frame(width: geo.size.width * fraction)
                }
            }
            .frame(height: 6)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
    }

    private var barColor: Color {
        if fraction >= 1.0 { return BenchColors.green }
        if fraction >= 0.5 { return BenchColors.yellow }
        return BenchColors.orange
    }
}

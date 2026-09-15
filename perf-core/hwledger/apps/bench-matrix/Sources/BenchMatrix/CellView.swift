import SwiftUI

struct CellView: View {
    let cell: MatrixCell
    let isHovered: Bool

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 4)
                .fill(BenchColors.cellColor(for: cell.status, score: cell.score))
                .overlay(
                    RoundedRectangle(cornerRadius: 4)
                        .stroke(isHovered ? BenchColors.text.opacity(0.3) : .clear, lineWidth: 1)
                )

            if let score = cell.score {
                Text(String(format: "%.2f", score))
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(.white)
                    .fontWeight(.semibold)
            } else if cell.status == .running {
                ProgressView()
                    .scaleEffect(0.6)
                    .progressViewStyle(.circular)
            } else {
                Text("--")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(BenchColors.textDim)
            }
        }
        .frame(minWidth: 80, minHeight: 40)
        .help(tooltipText)
    }

    private var tooltipText: String {
        var lines: [String] = []
        lines.append("Variant: \(cell.variant)")
        if let d = cell.cellData {
            lines.append("Suite: \(d.suite)")
            lines.append("Task: \(d.taskId)")
            lines.append(String(format: "Score: %.3f", d.judgeScore))
            lines.append(String(format: "Pass@1: %.3f", d.passAt1))
            lines.append(String(format: "Partial: %.3f", d.partialCredit))
            lines.append(String(format: "Wall: %.1fs", d.wallClockS))
            if d.tokensPerSecond > 0 {
                lines.append(String(format: "Tok/s: %.1f", d.tokensPerSecond))
            }
            lines.append(d.ok ? "OK" : "FAILED")
        } else {
            lines.append("No data")
        }
        return lines.joined(separator: "\n")
    }
}

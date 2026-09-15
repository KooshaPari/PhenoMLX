import SwiftUI

struct MatrixView: View {
    @ObservedObject var data: BenchmarkData
    @State private var hoveredCellId: String?

    var body: some View {
        ScrollView([.horizontal, .vertical]) {
            VStack(alignment: .leading, spacing: 0) {
                headerRow
                Divider().background(BenchColors.border)
                ForEach(data.matrixRows) { row in
                    rowView(row)
                    Divider().background(BenchColors.border.opacity(0.5))
                }
            }
        }
        .background(BenchColors.bg)
    }

    private var headerRow: some View {
        HStack(spacing: 0) {
            Text("Task")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(BenchColors.textDim)
                .frame(width: 180, alignment: .leading)
                .padding(.horizontal, 8)

            ForEach(data.variants, id: \.self) { variant in
                Text(variant)
                    .font(.system(.caption2, design: .monospaced).weight(.semibold))
                    .foregroundStyle(BenchColors.text)
                    .frame(width: 88, alignment: .center)
            }
        }
        .padding(.vertical, 6)
        .background(BenchColors.headerBg)
    }

    private func rowView(_ row: MatrixRow) -> some View {
        HStack(spacing: 0) {
            Text(row.taskName)
                .font(.system(.caption2, design: .monospaced))
                .foregroundStyle(BenchColors.text)
                .lineLimit(1)
                .frame(width: 180, alignment: .leading)
                .padding(.horizontal, 8)

            ForEach(row.cells) { cell in
                CellView(cell: cell, isHovered: hoveredCellId == cell.id)
                    .onHover { hovering in
                        hoveredCellId = hovering ? cell.id : nil
                    }
                    .frame(width: 88)
            }
        }
        .padding(.vertical, 2)
    }
}

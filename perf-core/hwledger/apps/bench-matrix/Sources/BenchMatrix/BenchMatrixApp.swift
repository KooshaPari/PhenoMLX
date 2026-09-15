import SwiftUI

@main
struct BenchMatrixApp: App {
    @StateObject private var data = BenchmarkData()
    @StateObject private var monitor: DirectoryMonitor

    private let runsDir: URL

    init() {
        let args = CommandLine.arguments
        let dir: URL
        if args.count > 1 {
            dir = URL(fileURLWithPath: args[1])
        } else {
            let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            dir = cwd.appendingPathComponent(".runs")
        }
        self.runsDir = dir
        self._monitor = StateObject(wrappedValue: DirectoryMonitor(directory: dir))
    }

    var body: some Scene {
        WindowGroup {
            ContentView(data: data, monitor: monitor, runsDir: runsDir)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1100, height: 650)
    }
}

struct ContentView: View {
    @ObservedObject var data: BenchmarkData
    @ObservedObject var monitor: DirectoryMonitor
    let runsDir: URL

    var body: some View {
        VStack(spacing: 0) {
            headerBar
            ProgressBar(
                complete: data.completeCount,
                total: data.totalCount,
                fraction: data.progressFraction
            )
            Divider().background(BenchColors.border)
            MatrixView(data: data)
        }
        .frame(minWidth: 900, minHeight: 500)
        .background(BenchColors.bg)
        .onAppear {
            data.loadAll(from: runsDir)
            monitor.start(
                onAdded: { url in data.load(from: url) },
                onChanged: { url in data.load(from: url) }
            )
        }
        .onDisappear {
            monitor.stop()
        }
    }

    private var headerBar: some View {
        HStack {
            Text("Bench Matrix")
                .font(.system(.headline, design: .monospaced))
                .foregroundStyle(BenchColors.text)
            if !data.model.isEmpty {
                Text("--")
                    .foregroundStyle(BenchColors.textDim)
                Text(data.model)
                    .font(.system(.headline, design: .monospaced))
                    .foregroundStyle(BenchColors.orange)
            }
            Spacer()
            Text("\(data.completeCount)/\(data.totalCount) complete")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(BenchColors.textDim)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(BenchColors.headerBg)
    }
}

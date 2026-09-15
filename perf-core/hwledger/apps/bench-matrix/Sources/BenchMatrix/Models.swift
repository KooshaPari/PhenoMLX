import Foundation

struct BenchmarkCell: Codable, Identifiable {
    let id: String
    let suite: String
    let taskId: String
    let variant: String
    let ok: Bool
    let passAt1: Double
    let partialCredit: Double
    let judgeScore: Double
    let wallClockS: Double
    let tokensPerSecond: Double
    let formatComplianceRate: Double
    let intentPreservationRate: Double
    let hallucinationCount: Int

    enum CodingKeys: String, CodingKey {
        case suite
        case taskId = "task_id"
        case variant
        case ok
        case passAt1 = "pass_at_1"
        case partialCredit = "partial_credit"
        case judgeScore = "judge_score"
        case wallClockS = "wall_clock_s"
        case tokensPerSecond = "tokens_per_second"
        case formatComplianceRate = "format_compliance_rate"
        case intentPreservationRate = "intent_preservation_rate"
        case hallucinationCount = "hallucination_count"
    }

    init(from cell: BenchmarkCell) {
        self.id = "\(cell.taskId)-\(cell.variant)"
        self.suite = cell.suite
        self.taskId = cell.taskId
        self.variant = cell.variant
        self.ok = cell.ok
        self.passAt1 = cell.passAt1
        self.partialCredit = cell.partialCredit
        self.judgeScore = cell.judgeScore
        self.wallClockS = cell.wallClockS
        self.tokensPerSecond = cell.tokensPerSecond
        self.formatComplianceRate = cell.formatComplianceRate
        self.intentPreservationRate = cell.intentPreservationRate
        self.hallucinationCount = cell.hallucinationCount
    }

    init(
        suite: String, taskId: String, variant: String,
        ok: Bool, passAt1: Double, partialCredit: Double,
        judgeScore: Double, wallClockS: Double, tokensPerSecond: Double,
        formatComplianceRate: Double, intentPreservationRate: Double,
        hallucinationCount: Int
    ) {
        self.id = "\(taskId)-\(variant)"
        self.suite = suite
        self.taskId = taskId
        self.variant = variant
        self.ok = ok
        self.passAt1 = passAt1
        self.partialCredit = partialCredit
        self.judgeScore = judgeScore
        self.wallClockS = wallClockS
        self.tokensPerSecond = tokensPerSecond
        self.formatComplianceRate = formatComplianceRate
        self.intentPreservationRate = intentPreservationRate
        self.hallucinationCount = hallucinationCount
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        suite = try c.decode(String.self, forKey: .suite)
        taskId = try c.decode(String.self, forKey: .taskId)
        variant = try c.decode(String.self, forKey: .variant)
        ok = try c.decode(Bool.self, forKey: .ok)
        passAt1 = try c.decode(Double.self, forKey: .passAt1)
        partialCredit = try c.decode(Double.self, forKey: .partialCredit)
        judgeScore = try c.decode(Double.self, forKey: .judgeScore)
        wallClockS = try c.decode(Double.self, forKey: .wallClockS)
        tokensPerSecond = try c.decode(Double.self, forKey: .tokensPerSecond)
        formatComplianceRate = try c.decode(Double.self, forKey: .formatComplianceRate)
        intentPreservationRate = try c.decode(Double.self, forKey: .intentPreservationRate)
        hallucinationCount = try c.decode(Int.self, forKey: .hallucinationCount)
        id = "\(taskId)-\(variant)"
    }
}

struct BenchmarkSummary: Codable {
    let meta: Meta?
    let byVariant: [String: VariantStats]?

    enum CodingKeys: String, CodingKey {
        case meta
        case byVariant = "by_variant"
    }

    struct Meta: Codable {
        let model: String?
        let variants: [String]?
        let nCells: Int?

        enum CodingKeys: String, CodingKey {
            case model
            case variants
            case nCells = "n_cells"
        }
    }

    struct VariantStats: Codable {
        let nCells: Int?
        let passAt1: Double?
        let okCount: Int?

        enum CodingKeys: String, CodingKey {
            case nCells = "n_cells"
            case passAt1 = "pass_at_1"
            case okCount = "ok_count"
        }
    }
}

struct BenchmarkRun: Codable {
    let summary: BenchmarkSummary?
    let cells: [BenchmarkCell]

    var modelName: String {
        summary?.meta?.model ?? "unknown"
    }

    var variants: [String] {
        summary?.meta?.variants ?? Array(Set(cells.map(\.variant))).sorted()
    }
}

enum CellStatus {
    case noData
    case running
    case failed
    case partial
    case passed

    init(score: Double?, isRunning: Bool = false) {
        if isRunning {
            self = .running
        } else if let s = score {
            if s < 0.5 { self = .failed }
            else if s <= 0.7 { self = .partial }
            else { self = .passed }
        } else {
            self = .noData
        }
    }

    var isComplete: Bool {
        switch self {
        case .passed, .partial, .failed: return true
        default: return false
        }
    }
}

struct MatrixRow: Identifiable {
    let id: String
    let taskName: String
    let cells: [MatrixCell]
}

struct MatrixCell: Identifiable {
    let id: String
    let variant: String
    let score: Double?
    let status: CellStatus
    let cellData: BenchmarkCell?
}

class BenchmarkData: ObservableObject {
    @Published var runs: [BenchmarkRun] = []
    @Published var matrixRows: [MatrixRow] = []
    @Published var variants: [String] = []
    @Published var model: String = ""
    @Published var completeCount: Int = 0
    @Published var totalCount: Int = 0

    var progressFraction: Double {
        totalCount > 0 ? Double(completeCount) / Double(totalCount) : 0
    }

    func load(from url: URL) {
        guard let data = try? Data(contentsOf: url),
              let run = try? JSONDecoder().decode(BenchmarkRun.self, from: data) else {
            return
        }

        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.runs.append(run)
            self.model = run.modelName
            self.rebuild()
        }
    }

    func loadAll(from directory: URL) {
        let fm = FileManager.default
        guard let files = try? fm.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: nil
        ) else { return }

        let jsonFiles = files.filter { $0.pathExtension == "json" }
        for file in jsonFiles {
            load(from: file)
        }
    }

    func rebuild() {
        var taskSet = Set<String>()
        var variantSet = Set<String>()
        var cellMap: [String: [String: BenchmarkCell]] = [:]

        for run in runs {
            for cell in run.cells {
                let taskKey = "\(cell.suite)/\(cell.taskId)"
                taskSet.insert(taskKey)
                variantSet.insert(cell.variant)

                if cellMap[taskKey] == nil { cellMap[taskKey] = [:] }
                cellMap[taskKey]![cell.variant] = cell
            }
        }

        variants = variantSet.sorted()

        let rows: [MatrixRow] = taskSet.sorted().map { taskKey in
            let cells: [MatrixCell] = variants.map { variant in
                let cellData = cellMap[taskKey]?[variant]
                let score = cellData?.judgeScore
                let status = CellStatus(score: score)
                return MatrixCell(
                    id: "\(taskKey)-\(variant)",
                    variant: variant,
                    score: score,
                    status: status,
                    cellData: cellData
                )
            }
            return MatrixRow(id: taskKey, taskName: taskKey, cells: cells)
        }

        matrixRows = rows
        totalCount = taskSet.count * variantSet.count
        completeCount = rows.flatMap(\.cells).filter(\.status.isComplete).count
    }
}

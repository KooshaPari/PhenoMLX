// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "BenchMatrix",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(
            name: "BenchMatrix",
            path: "Sources/BenchMatrix"
        )
    ]
)

import Foundation
import Combine

class DirectoryMonitor: NSObject, ObservableObject {
    private let fileManager = FileManager.default
    private var source: DispatchSourceFileSystemObject?
    private let directoryURL: URL
    private var knownFiles: Set<String> = []
    private var onFileAdded: ((URL) -> Void)?
    private var onFileChanged: ((URL) -> Void)?

    init(directory: URL) {
        self.directoryURL = directory
        super.init()
        scanExisting()
    }

    func start(onAdded: @escaping (URL) -> Void, onChanged: @escaping (URL) -> Void) {
        self.onFileAdded = onAdded
        self.onFileChanged = onChanged

        let fd = FileHandle(forReadingAtPath: directoryURL.path)
        guard let handle = fd else {
            print("[BenchMatrix] Cannot watch \(directoryURL.path)")
            return
        }

        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: handle.fileDescriptor,
            eventMask: [.write, .rename, .delete, .attrib],
            queue: .global(qos: .userInitiated)
        )

        source.setEventHandler { [weak self] in
            self?.poll()
        }

        source.setCancelHandler {
            handle.closeFile()
        }

        self.source = source
        source.resume()
    }

    func stop() {
        source?.cancel()
        source = nil
    }

    private func scanExisting() {
        guard let files = try? fileManager.contentsOfDirectory(
            at: directoryURL,
            includingPropertiesForKeys: nil
        ) else { return }

        for file in files where file.pathExtension == "json" {
            knownFiles.insert(file.lastPathComponent)
        }
    }

    private func poll() {
        guard let files = try? fileManager.contentsOfDirectory(
            at: directoryURL,
            includingPropertiesForKeys: [.contentModificationDateKey]
        ) else { return }

        let current = Set(files.filter { $0.pathExtension == "json" }.map(\.lastPathComponent))

        let added = current.subtracting(knownFiles)
        let changed = current.intersection(knownFiles)

        for name in added {
            let url = directoryURL.appendingPathComponent(name)
            DispatchQueue.main.async { [weak self] in
                self?.onFileAdded?(url)
            }
        }

        for name in changed {
            let url = directoryURL.appendingPathComponent(name)
            DispatchQueue.main.async { [weak self] in
                self?.onFileChanged?(url)
            }
        }

        knownFiles = current
    }
}

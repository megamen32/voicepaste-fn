// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "VoicePasteSwiftHelpers",
    platforms: [
        .macOS(.v11)
    ],
    products: [
        .library(name: "ModifierMonitor", targets: ["ModifierMonitor"]),
        .executable(name: "modifier_monitor", targets: ["ModifierMonitorExec"]),
        .library(name: "NativeSTT", targets: ["NativeSTT"]),
        .executable(name: "native_stt", targets: ["NativeSTTExec"]),
    ],
    targets: [
        .target(
            name: "ModifierMonitor",
            path: "Sources/ModifierMonitor"
        ),
        .executableTarget(
            name: "ModifierMonitorExec",
            dependencies: ["ModifierMonitor"],
            path: "Sources/ModifierMonitorExec"
        ),
        .target(
            name: "NativeSTT",
            path: "Sources/NativeSTT"
        ),
        .executableTarget(
            name: "NativeSTTExec",
            dependencies: ["NativeSTT"],
            path: "Sources/NativeSTTExec"
        ),
        .testTarget(
            name: "ModifierMonitorTests",
            dependencies: ["ModifierMonitor"],
            path: "Tests/ModifierMonitorTests"
        ),
        .testTarget(
            name: "NativeSTTTests",
            dependencies: ["NativeSTT"],
            path: "Tests/NativeSTTTests"
        ),
    ]
)

// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "VoicePasteFn",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "voicepaste-fn", targets: ["VoicePasteFn"]),
        .executable(name: "voicepaste-model-probe", targets: ["VoicePasteModelProbe"]),
        .library(name: "VoicePasteLib", targets: ["VoicePasteLib"]),
    ],
    targets: [
        .target(
            name: "VoicePasteLib",
            path: "Sources/VoicePasteLib"
        ),
        .executableTarget(
            name: "VoicePasteFn",
            dependencies: ["VoicePasteLib"],
            path: "Sources/VoicePasteFn"
        ),
        .executableTarget(
            name: "VoicePasteModelProbe",
            dependencies: ["VoicePasteLib"],
            path: "Sources/VoicePasteModelProbe"
        ),
        .testTarget(
            name: "VoicePasteFnTests",
            dependencies: ["VoicePasteLib"],
            path: "Tests/VoicePasteFnTests"
        )
    ]
)

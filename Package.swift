// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "VoicePasteFn",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "voicepaste-fn", targets: ["VoicePasteFn"]),
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
        .testTarget(
            name: "VoicePasteFnTests",
            dependencies: ["VoicePasteLib"],
            path: "Tests/VoicePasteFnTests"
        )
    ]
)
